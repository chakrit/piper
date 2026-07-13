use crate::dsl::AspectAction;
use crate::llm::LlmClient;
use crate::models::AspectType;
use crate::prompt_templates::PromptTemplates;
use crate::state::PipelineState;
use anyhow::{Context, Result};
use dialoguer::Select;

pub struct Driver<C: LlmClient> {
    client: C,
    prompt_templates: PromptTemplates,
}

#[derive(Debug)]
pub enum DriveStatus {
    WaitingAtGate,
    Done(Vec<String>),
    Stopped(String),
}

pub struct DriveOutcome {
    pub state: PipelineState,
    pub status: DriveStatus,
}

impl<C: LlmClient> Driver<C> {
    pub fn new(client: C, prompt_templates: PromptTemplates) -> Self {
        Self {
            client,
            prompt_templates,
        }
    }

    fn format_aspect_context(state: &PipelineState) -> String {
        let context = std::iter::once(format!("CONTEXT {}", state.context_path()));
        let mut knowledge: Vec<_> = state.frame().corpus.iter().collect();
        knowledge.sort_unstable_by_key(|(key, _)| *key);
        let knowledge = knowledge
            .into_iter()
            .map(|(_, value)| format!("KNOWLEDGE {value}"));
        let jump_points = state
            .aspects
            .iter()
            .map(|aspect| format!("ASPECT {}", aspect.id));
        let fork_history = state
            .frame()
            .completed_forks
            .iter()
            .map(|fork| format!("FORK {} {}", fork.id, fork.prompt));
        let mut fork_knowledge: Vec<_> = state
            .frame()
            .completed_forks
            .iter()
            .flat_map(|fork| {
                fork.corpus
                    .iter()
                    .map(|(key, value)| (fork.id.as_str(), key.as_str(), value.as_str()))
            })
            .collect();
        fork_knowledge.sort_unstable_by_key(|(fork_id, key, _)| (*fork_id, *key));
        let fork_knowledge = fork_knowledge
            .into_iter()
            .map(|(fork_id, _, value)| format!("FORK_KNOWLEDGE {fork_id} {value}"));

        context
            .chain(knowledge)
            .chain(jump_points)
            .chain(fork_history)
            .chain(fork_knowledge)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn handle_gate(&self, state: &PipelineState) -> Result<bool> {
        println!("│ Request   {}", Self::one_line(&state.request));
        println!("╰─ Waiting for approval");

        let options = vec!["Continue", "Abort"];
        let selection = Select::new()
            .with_prompt("Choose an action")
            .items(&options)
            .interact()?;

        match selection {
            0 => Ok(true),
            _ => Ok(false),
        }
    }

    pub fn run(&self, mut state: PipelineState) -> Result<Vec<String>> {
        Self::log_run_started(&state);

        loop {
            let outcome = self.drive(state)?;
            state = outcome.state;
            match outcome.status {
                DriveStatus::WaitingAtGate => {
                    let gate_id = state.current_aspect().id.clone();
                    if !self.handle_gate(&state)? {
                        anyhow::bail!("Run aborted by user at gate {gate_id}");
                    }
                    state.frame_mut().current_index += 1;
                    println!("✓ Gate approved · continuing after {gate_id}");
                }
                DriveStatus::Done(results) => {
                    Self::log_completed(&state, results.len());
                    return Ok(results);
                }
                DriveStatus::Stopped(reason) => {
                    println!("\n■ Stopped · {}", Self::one_line(&reason));
                    Self::log_budget_used(&state);
                    return Ok(Vec::new());
                }
            }
        }
    }

    pub fn drive(&self, mut state: PipelineState) -> Result<DriveOutcome> {
        state.validate()?;

        loop {
            if state.frame().current_index >= state.aspects.len() {
                if state.is_root() {
                    return Ok(DriveOutcome {
                        state,
                        status: DriveStatus::Done(Vec::new()),
                    });
                }
                Self::ensure_no_unjoined_forks(&state, "return")?;
                state.return_current_fork()?;
                continue;
            }
            let aspect = state.current_aspect().clone();

            match aspect.aspect_type {
                AspectType::Gate => {
                    Self::log_aspect_started(&state, "Gate");
                    println!("│ Aspect    {} · gate", aspect.id);

                    return Ok(DriveOutcome {
                        state,
                        status: DriveStatus::WaitingAtGate,
                    });
                }
                AspectType::Worker => {
                    state.consume_turn()?;
                    Self::log_aspect_started(&state, "Worker");
                    println!("│ Aspect    {} · worker", aspect.id);
                    println!(
                        "│ Context   {} · {}",
                        state.context_path(),
                        state
                            .frame()
                            .fork_id()
                            .map(|id| format!("fork {id}"))
                            .unwrap_or_else(|| "root".to_string())
                    );
                    println!(
                        "│ Budget    turn {}/{} · recovery {}/{}",
                        state.turns_taken,
                        state.max_turns,
                        state.recoveries_taken,
                        state.max_recoveries
                    );
                    println!(
                        "│ State     {} corpus entries · {} pending forks",
                        state.frame().corpus.len(),
                        state.frame().completed_forks.len()
                    );
                    println!(
                        "│ Model     {}",
                        self.client.resolve_model(aspect.model.as_deref())
                    );

                    let template = self.prompt_templates.worker(&crate::actions::AppContext)?;

                    let context = Self::format_aspect_context(&state);

                    let prompt = template
                        .replace("{{aspect_name}}", &aspect.name)
                        .replace("{{aspect_prompt}}", &aspect.prompt)
                        .replace("{{aspect_id}}", &aspect.id)
                        .replace("{{request}}", &state.request)
                        .replace("{{fork_prompt}}", state.frame().fork_prompt().unwrap_or(""))
                        .replace("{{context}}", &context);

                    println!("│ LLM       requesting action");
                    let raw_response = self.client.call(&prompt, aspect.model.as_deref())?;

                    let action = match AspectAction::parse(&raw_response) {
                        Ok(action) => {
                            Self::log_action("Action", &action);
                            action
                        }
                        Err(error) => {
                            state.consume_recovery()?;
                            println!(
                                "│ Recovery  {}/{} · {}",
                                state.recoveries_taken, state.max_recoveries, error
                            );
                            println!("│ Rejected  {}", Self::one_line(&raw_response));
                            let recover_template =
                                self.prompt_templates.recover(&crate::actions::AppContext)?;

                            let recover_prompt =
                                format!("{}\n\nBad Response:\n{}", recover_template, raw_response);
                            println!("│ LLM       requesting formatter recovery");
                            let recovered_raw =
                                self.client.call(&recover_prompt, aspect.model.as_deref())?;
                            let recovered =
                                AspectAction::parse(&recovered_raw).with_context(|| {
                                    format!(
                                        "Recovery response was not valid Piper DSL: {}",
                                        Self::one_line(&recovered_raw)
                                    )
                                })?;
                            Self::log_action("Recovered", &recovered);
                            recovered
                        }
                    };

                    let mut terminal = None;
                    match action {
                        AspectAction::Set(value) => {
                            let key = state.contribution_key();
                            state.frame_mut().corpus.insert(key, value);
                            println!(
                                "╰─ Saved contribution · {} corpus entries",
                                state.frame().corpus.len()
                            );
                        }
                        AspectAction::Continue => {
                            let joined_forks = state.frame().completed_forks.len();
                            let returning_fork = state.frame().fork_id().map(str::to_string);
                            state.join_completed_forks();
                            if state.is_root() {
                                state.frame_mut().current_index += 1;
                                println!(
                                    "╰─ Advanced · joined {joined_forks} forks · {} corpus entries",
                                    state.frame().corpus.len()
                                );
                            } else {
                                state.return_current_fork()?;
                                println!(
                                    "╰─ Returned fork {} · joined {joined_forks} descendant forks",
                                    returning_fork.expect("non-root frame has a fork ID")
                                );
                            }
                        }
                        AspectAction::Fork { id, prompt } => {
                            let fork_id = id.clone();
                            state.push_fork(id, prompt)?;
                            println!(
                                "╰─ Entered fork {fork_id} · depth {}",
                                state.frames.len() - 1
                            );
                        }
                        AspectAction::Jump(id) => {
                            Self::ensure_no_unjoined_forks(&state, "JUMP")?;
                            state.jump_to_id(&id)?;
                            println!("╰─ Jumped to Aspect {id}");
                        }
                        AspectAction::Stop(reason) => {
                            Self::ensure_no_unjoined_forks(&state, "STOP")?;
                            println!("╰─ Stop accepted");
                            terminal = Some(DriveStatus::Stopped(reason));
                        }
                        AspectAction::Done(result) => {
                            Self::ensure_no_unjoined_forks(&state, "DONE")?;
                            if state.is_root() {
                                println!("╰─ Completion accepted");
                                terminal = Some(DriveStatus::Done(vec![result]));
                            } else {
                                let fork_id = state
                                    .frame()
                                    .fork_id()
                                    .expect("non-root frame has a fork ID")
                                    .to_string();
                                state.return_current_fork()?;
                                println!("╰─ Returned completed fork {fork_id}");
                            }
                        }
                    }
                    if let Some(status) = terminal {
                        return Ok(DriveOutcome { state, status });
                    }
                }
            }
        }
    }

    fn log_run_started(state: &PipelineState) {
        println!("╭─ Piper run · {}", state.pipeline_id.as_str());
        println!("│ Request   {}", Self::one_line(&state.request));
        println!("│ Aspects   {}", state.aspects.len());
        println!(
            "│ Budgets   {} turns · {} recoveries",
            state.max_turns, state.max_recoveries
        );
        println!("╰─ Starting");
    }

    fn log_aspect_started(state: &PipelineState, kind: &str) {
        println!(
            "\n╭─ {kind} {}/{} · {}",
            state.frame().current_index + 1,
            state.aspects.len(),
            state.current_aspect().name
        );
    }

    fn log_action(label: &str, action: &AspectAction) {
        match action {
            AspectAction::Set(value) => println!("│ {label:<10}SET · {value}"),
            AspectAction::Continue => println!("│ {label:<10}CONTINUE"),
            AspectAction::Fork { id, prompt } => {
                println!("│ {label:<10}FORK {id} · {prompt}");
            }
            AspectAction::Jump(id) => println!("│ {label:<10}JUMP · {id}"),
            AspectAction::Stop(reason) => println!("│ {label:<10}STOP · {reason}"),
            AspectAction::Done(result) => println!("│ {label:<10}DONE · {result}"),
        }
    }

    fn log_completed(state: &PipelineState, output_count: usize) {
        let output_label = if output_count == 1 {
            "output"
        } else {
            "outputs"
        };
        println!("\n✓ Completed · {output_count} {output_label}");
        Self::log_budget_used(state);
    }

    fn log_budget_used(state: &PipelineState) {
        println!(
            "  Budget used · {}/{} turns · {}/{} recoveries",
            state.turns_taken, state.max_turns, state.recoveries_taken, state.max_recoveries
        );
    }

    fn one_line(value: &str) -> String {
        value.replace('\r', "\\r").replace('\n', "\\n")
    }

    fn ensure_no_unjoined_forks(state: &PipelineState, action: &str) -> Result<()> {
        if !state.frame().completed_forks.is_empty() {
            anyhow::bail!("{action} cannot discard unjoined fork work; use CONTINUE first");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identifier::PipelineId;
    use crate::models::{Aspect, AspectType};
    use std::sync::Mutex;

    fn pipeline_id() -> PipelineId {
        PipelineId::new("test-pipeline").expect("test pipeline ID should be valid")
    }

    struct MockLlmClient {
        responses: Mutex<Vec<String>>,
    }

    impl LlmClient for MockLlmClient {
        fn call(&self, _prompt: &str, _model: Option<&str>) -> Result<String> {
            let mut resps = self.responses.lock().unwrap();
            if resps.is_empty() {
                anyhow::bail!("No mock responses left");
            }
            Ok(resps.remove(0))
        }
    }

    struct RecordingMockLlmClient {
        responses: Mutex<Vec<String>>,
        prompts: Mutex<Vec<String>>,
    }

    impl LlmClient for RecordingMockLlmClient {
        fn call(&self, prompt: &str, _model: Option<&str>) -> Result<String> {
            self.prompts
                .lock()
                .expect("prompts lock should not be poisoned")
                .push(prompt.to_string());
            let mut responses = self
                .responses
                .lock()
                .expect("responses lock should not be poisoned");
            if responses.is_empty() {
                anyhow::bail!("No mock responses left");
            }
            Ok(responses.remove(0))
        }
    }

    #[test]
    fn test_run_with_dsl() {
        let temp_dir = std::env::temp_dir().join("piper_tests_dsl");
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("aspect-worker.md"), "Mock template").unwrap();

        let parent_fork = "FORK f1 Analyze the alternative".to_string();
        let fork_knowledge = "SET The behavior needs a failing test.".to_string();
        let fork_result = "CONTINUE".to_string();
        let parent_merge = "CONTINUE".to_string();
        let parent_result = "DONE Parent Final".to_string();

        let client = MockLlmClient {
            responses: Mutex::new(vec![
                parent_fork,
                fork_knowledge,
                fork_result,
                parent_merge,
                parent_result,
            ]),
        };

        let driver = Driver::new(client, PromptTemplates::directory(temp_dir));
        let state = PipelineState::new(
            pipeline_id(),
            "Request".to_string(),
            vec![
                Aspect {
                    id: "a1".to_string(),
                    name: "Distill".to_string(),
                    aspect_type: AspectType::Worker,
                    prompt: "Prompt".to_string(),
                    model: None,
                },
                Aspect {
                    id: "a2".to_string(),
                    name: "Implement".to_string(),
                    aspect_type: AspectType::Worker,
                    prompt: "Prompt".to_string(),
                    model: None,
                },
            ],
            std::num::NonZeroUsize::new(10).expect("turn limit is non-zero"),
            3,
        );

        let results = driver.run(state).expect("run should succeed");

        assert_eq!(results, vec!["Parent Final"]);
    }

    #[test]
    fn test_drive_pauses_at_gate_without_reading_terminal_input() {
        let temp_dir = tempfile::tempdir().expect("test prompt directory should exist");
        let client = MockLlmClient {
            responses: Mutex::new(Vec::new()),
        };
        let driver = Driver::new(
            client,
            PromptTemplates::directory(temp_dir.path().to_path_buf()),
        );
        let state = PipelineState::new(
            pipeline_id(),
            "Approve the release".to_string(),
            vec![Aspect {
                id: "approval".to_string(),
                name: "Release Approval".to_string(),
                aspect_type: AspectType::Gate,
                prompt: String::new(),
                model: None,
            }],
            std::num::NonZeroUsize::new(1).expect("turn limit should be non-zero"),
            0,
        );

        let outcome = driver.drive(state).expect("drive should pause");

        assert!(matches!(outcome.status, DriveStatus::WaitingAtGate));
        assert_eq!(outcome.state.frame().current_index, 0);
        assert_eq!(outcome.state.current_aspect().id, "approval");
    }

    #[test]
    fn test_fork_runs_current_aspect_and_returns_knowledge_to_parent() {
        let temp_dir = tempfile::tempdir().expect("test prompt directory should exist");
        std::fs::write(
            temp_dir.path().join("aspect-worker.md"),
            "Aspect: {{aspect_id}}\nFork: {{fork_prompt}}\nContext:\n{{context}}",
        )
        .expect("worker template should be written");

        let client = RecordingMockLlmClient {
            responses: Mutex::new(vec![
                "SET CAST draft".to_string(),
                "FORK hero Create a fantasy hero".to_string(),
                "SET CHARACTER Lyra".to_string(),
                "CONTINUE".to_string(),
                "SET CAST Lyra".to_string(),
                "CONTINUE".to_string(),
                "DONE Lyra crossed the moonlit bridge.".to_string(),
            ]),
            prompts: Mutex::new(Vec::new()),
        };
        let driver = Driver::new(
            client,
            PromptTemplates::directory(temp_dir.path().to_path_buf()),
        );
        let state = PipelineState::new(
            pipeline_id(),
            "Write a short story".to_string(),
            vec![
                Aspect {
                    id: "branch".to_string(),
                    name: "Branch".to_string(),
                    aspect_type: AspectType::Worker,
                    prompt: "Create branches".to_string(),
                    model: None,
                },
                Aspect {
                    id: "story".to_string(),
                    name: "Story".to_string(),
                    aspect_type: AspectType::Worker,
                    prompt: "Compose from the joined Corpus".to_string(),
                    model: None,
                },
            ],
            std::num::NonZeroUsize::new(7).expect("turn limit is non-zero"),
            3,
        );

        let results = driver.run(state).expect("run should succeed");
        assert_eq!(results, vec!["Lyra crossed the moonlit bridge."]);

        let prompts = driver
            .client
            .prompts
            .lock()
            .expect("prompts lock should not be poisoned");
        assert!(prompts[0].contains("CONTEXT branch"));
        assert!(prompts[1].contains("KNOWLEDGE CAST draft"));
        assert!(prompts[2].contains("Aspect: branch\nFork: Create a fantasy hero"));
        assert!(prompts[2].contains("CONTEXT branch/hero"));
        assert!(prompts[3].contains("KNOWLEDGE CHARACTER Lyra"));
        assert!(prompts[4].contains("Aspect: branch\nFork: \n"));
        assert!(!prompts[4].contains("KNOWLEDGE CHARACTER Lyra"));
        assert!(prompts[4].contains("FORK_KNOWLEDGE hero CHARACTER Lyra"));
        assert!(prompts[5].contains("KNOWLEDGE CAST Lyra"));
        assert!(!prompts[5].contains("KNOWLEDGE CAST draft"));
        assert!(prompts[5].contains("FORK_KNOWLEDGE hero CHARACTER Lyra"));
        assert!(prompts[6].contains("Aspect: story\nFork: \n"));
        assert!(prompts[6].contains("KNOWLEDGE CHARACTER Lyra"));
        assert!(prompts[6].contains("KNOWLEDGE CAST Lyra"));
        assert!(!prompts[6].contains("KNOWLEDGE CAST draft"));
    }

    #[test]
    fn test_run_stops_when_turn_limit_is_reached() {
        let temp_dir = std::env::temp_dir().join("piper_tests_turn_limit");
        std::fs::create_dir_all(&temp_dir).expect("test prompt directory should exist");
        std::fs::write(temp_dir.join("aspect-worker.md"), "Mock template")
            .expect("test prompt should be written");

        let jump_response = "JUMP a1".to_string();

        let client = MockLlmClient {
            responses: Mutex::new(vec![jump_response.clone(), jump_response]),
        };
        let driver = Driver::new(client, PromptTemplates::directory(temp_dir));
        let state = PipelineState::new(
            pipeline_id(),
            "Initial".to_string(),
            vec![Aspect {
                id: "a1".to_string(),
                name: "Aspect 1".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Prompt".to_string(),
                model: None,
            }],
            std::num::NonZeroUsize::new(2).expect("turn limit is non-zero"),
            3,
        );

        let error = driver
            .run(state)
            .expect_err("loop should hit the turn limit");
        assert!(error.to_string().contains("max_turns limit of 2"));
    }

    #[test]
    fn test_final_aspect_can_fork_and_join() {
        let temp_dir = tempfile::tempdir().expect("test prompt directory should exist");
        std::fs::write(temp_dir.path().join("aspect-worker.md"), "{{context}}")
            .expect("worker template should be written");

        let client = MockLlmClient {
            responses: Mutex::new(vec![
                "FORK check Verify the final calculation".to_string(),
                "SET VERIFIED 42".to_string(),
                "JOIN".to_string(),
                "CONTINUE".to_string(),
            ]),
        };
        let driver = Driver::new(
            client,
            PromptTemplates::directory(temp_dir.path().to_path_buf()),
        );
        let state = PipelineState::new(
            pipeline_id(),
            "Verify the answer".to_string(),
            vec![Aspect {
                id: "verify".to_string(),
                name: "Verify".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Fork one independent check".to_string(),
                model: None,
            }],
            std::num::NonZeroUsize::new(4).expect("turn limit is non-zero"),
            1,
        );

        let results = driver
            .run(state)
            .expect("final Aspect fork should join cleanly");

        assert!(results.is_empty());
    }

    #[test]
    fn test_run_recovers_a_malformed_worker_response() {
        let temp_dir = std::env::temp_dir().join("piper_tests_recovery");
        std::fs::create_dir_all(&temp_dir).expect("test prompt directory should exist");
        std::fs::write(temp_dir.join("aspect-worker.md"), "Mock template")
            .expect("worker template should be written");
        std::fs::write(temp_dir.join("aspect-recover.md"), "Recovery template")
            .expect("recovery template should be written");

        let recovered_response = "DONE Recovered result".to_string();

        let client = MockLlmClient {
            responses: Mutex::new(vec!["not a DSL action".to_string(), recovered_response]),
        };
        let driver = Driver::new(client, PromptTemplates::directory(temp_dir));
        let state = PipelineState::new(
            pipeline_id(),
            "Initial".to_string(),
            vec![Aspect {
                id: "a1".to_string(),
                name: "Aspect 1".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Prompt".to_string(),
                model: None,
            }],
            std::num::NonZeroUsize::new(3).expect("turn limit is non-zero"),
            1,
        );

        let results = driver.run(state).expect("recovery should succeed");
        assert_eq!(results, vec!["Recovered result"]);
    }

    #[test]
    fn test_run_set_replaces_current_worker_contribution() {
        let temp_dir = std::env::temp_dir().join("piper_tests_knowledge");
        std::fs::create_dir_all(&temp_dir).expect("test prompt directory should exist");
        std::fs::write(temp_dir.join("aspect-worker.md"), "Context:\n{{context}}")
            .expect("worker template should be written");

        let client = RecordingMockLlmClient {
            responses: Mutex::new(vec![
                "SET Preserve compatibility".to_string(),
                "SET Preserve the stable API".to_string(),
                "DONE Final result".to_string(),
            ]),
            prompts: Mutex::new(Vec::new()),
        };
        let driver = Driver::new(client, PromptTemplates::directory(temp_dir));
        let state = PipelineState::new(
            pipeline_id(),
            "Initial".to_string(),
            vec![Aspect {
                id: "a1".to_string(),
                name: "Aspect 1".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Prompt".to_string(),
                model: None,
            }],
            std::num::NonZeroUsize::new(3).expect("turn limit is non-zero"),
            3,
        );

        let results = driver.run(state).expect("run should succeed");
        assert_eq!(results, vec!["Final result"]);

        let prompts = driver
            .client
            .prompts
            .lock()
            .expect("prompts lock should not be poisoned");
        assert_eq!(
            prompts.as_slice(),
            [
                "Context:\nCONTEXT a1\nASPECT a1",
                "Context:\nCONTEXT a1\nKNOWLEDGE Preserve compatibility\nASPECT a1",
                "Context:\nCONTEXT a1\nKNOWLEDGE Preserve the stable API\nASPECT a1",
            ]
        );
    }

    #[test]
    fn test_run_updates_corpus_without_changing_request() {
        let temp_dir = std::env::temp_dir().join("piper_tests_update_knowledge");
        std::fs::create_dir_all(&temp_dir).expect("test prompt directory should exist");
        std::fs::write(
            temp_dir.join("aspect-worker.md"),
            "Request: {{request}}\nContext:\n{{context}}",
        )
        .expect("worker template should be written");

        let client = RecordingMockLlmClient {
            responses: Mutex::new(vec![
                "SET Preserve compatibility".to_string(),
                "SET Preserve the stable API".to_string(),
                "DONE Final result".to_string(),
            ]),
            prompts: Mutex::new(Vec::new()),
        };
        let driver = Driver::new(client, PromptTemplates::directory(temp_dir));
        let state = PipelineState::new(
            pipeline_id(),
            "Keep this request intact".to_string(),
            vec![
                Aspect {
                    id: "a1".to_string(),
                    name: "Aspect 1".to_string(),
                    aspect_type: AspectType::Worker,
                    prompt: "Prompt".to_string(),
                    model: None,
                },
                Aspect {
                    id: "a2".to_string(),
                    name: "Aspect 2".to_string(),
                    aspect_type: AspectType::Worker,
                    prompt: "Prompt".to_string(),
                    model: None,
                },
            ],
            std::num::NonZeroUsize::new(3).expect("turn limit is non-zero"),
            3,
        );

        let results = driver.run(state).expect("run should succeed");
        assert_eq!(results, vec!["Final result"]);

        let prompts = driver
            .client
            .prompts
            .lock()
            .expect("prompts lock should not be poisoned");
        assert_eq!(
            prompts.as_slice(),
            [
                "Request: Keep this request intact\nContext:\nCONTEXT a1\nASPECT a1\nASPECT a2",
                "Request: Keep this request intact\nContext:\nCONTEXT a1\nKNOWLEDGE Preserve compatibility\nASPECT a1\nASPECT a2",
                "Request: Keep this request intact\nContext:\nCONTEXT a1\nKNOWLEDGE Preserve the stable API\nASPECT a1\nASPECT a2",
            ]
        );
    }

    #[test]
    fn test_run_stop_returns_no_final_result() {
        let temp_dir = std::env::temp_dir().join("piper_tests_stop");
        std::fs::create_dir_all(&temp_dir).expect("test prompt directory should exist");
        std::fs::write(temp_dir.join("aspect-worker.md"), "Mock template")
            .expect("worker template should be written");

        let client = MockLlmClient {
            responses: Mutex::new(vec!["STOP Missing requirements".to_string()]),
        };
        let driver = Driver::new(client, PromptTemplates::directory(temp_dir));
        let state = PipelineState::new(
            pipeline_id(),
            "Initial".to_string(),
            vec![Aspect {
                id: "a1".to_string(),
                name: "Aspect 1".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Prompt".to_string(),
                model: None,
            }],
            std::num::NonZeroUsize::new(2).expect("turn limit is non-zero"),
            3,
        );

        let results = driver.run(state).expect("stop should be clean");
        assert!(results.is_empty());
    }

    #[test]
    fn test_done_rejects_unjoined_fork_work() {
        let temp_dir = std::env::temp_dir().join("piper_tests_unjoined_fork");
        std::fs::create_dir_all(&temp_dir).expect("test prompt directory should exist");
        std::fs::write(temp_dir.join("aspect-worker.md"), "Mock template")
            .expect("worker template should be written");

        let client = MockLlmClient {
            responses: Mutex::new(vec![
                "FORK f1 Analyze the alternative".to_string(),
                "DONE Child complete".to_string(),
                "DONE Parent complete".to_string(),
            ]),
        };
        let driver = Driver::new(client, PromptTemplates::directory(temp_dir));
        let state = PipelineState::new(
            pipeline_id(),
            "Initial".to_string(),
            vec![
                Aspect {
                    id: "a1".to_string(),
                    name: "Aspect 1".to_string(),
                    aspect_type: AspectType::Worker,
                    prompt: "Prompt".to_string(),
                    model: None,
                },
                Aspect {
                    id: "a2".to_string(),
                    name: "Aspect 2".to_string(),
                    aspect_type: AspectType::Worker,
                    prompt: "Prompt".to_string(),
                    model: None,
                },
            ],
            std::num::NonZeroUsize::new(3).expect("turn limit is non-zero"),
            3,
        );

        let error = driver
            .run(state)
            .expect_err("DONE with unjoined fork work should fail");
        assert!(error.to_string().contains("use CONTINUE first"));
    }
}
