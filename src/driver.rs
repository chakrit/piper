use crate::dsl::AspectAction;
use crate::llm::LlmClient;
use crate::models::AspectType;
use crate::state::{CompletedFork, PipelineState, SessionContext};
use anyhow::{Context, Result};
use dialoguer::Select;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct Driver<C: LlmClient> {
    client: C,
    prompt_dir: PathBuf,
}

struct RunOutcome {
    results: Vec<String>,
    corpus: HashMap<String, String>,
    stopped: bool,
}

impl<C: LlmClient> Driver<C> {
    pub fn new(client: C, prompt_dir: PathBuf) -> Self {
        Self { client, prompt_dir }
    }

    fn load_prompt(&self, filename: &str) -> Result<String> {
        fs::read_to_string(self.prompt_dir.join(filename))
            .with_context(|| format!("Failed to read prompt template {}", filename))
    }

    fn format_aspect_context(state: &PipelineState) -> String {
        let mut knowledge: Vec<_> = state.corpus.iter().collect();
        knowledge.sort_unstable_by_key(|(key, _)| *key);
        let knowledge = knowledge
            .into_iter()
            .map(|(_, value)| format!("KNOWLEDGE {value}"));
        let jump_points = state
            .aspects
            .iter()
            .map(|aspect| format!("ASPECT {}", aspect.id));
        let fork_history = state
            .completed_forks
            .iter()
            .map(|fork| format!("FORK {} {}", fork.id, fork.prompt));

        knowledge
            .chain(jump_points)
            .chain(fork_history)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn handle_gate(&self, state: &mut PipelineState) -> Result<bool> {
        println!("\n--- 🛑 STOP-GATE: {} ---", state.current_aspect().name);
        println!("\nREQUEST:\n\n{}\n", state.request);
        println!("--------------------------------------------------");

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

    pub fn run(&self, ctx: &mut SessionContext, state: PipelineState) -> Result<Vec<String>> {
        let outcome = self.run_inner(ctx, state)?;
        Ok(outcome.results)
    }

    fn run_inner(&self, ctx: &mut SessionContext, mut state: PipelineState) -> Result<RunOutcome> {
        let mut all_results = Vec::new();

        while state.current_index < state.aspects.len() {
            let aspect = state.current_aspect().clone();
            let fork_info = state
                .fork_id
                .as_ref()
                .map(|id| format!("[Fork: {}]", id))
                .unwrap_or_default();
            println!(
                "\n{} >>> Processing: {} ({} - {})",
                fork_info,
                aspect.name,
                aspect.aspect_type.label(),
                aspect.id
            );

            match aspect.aspect_type {
                AspectType::Gate => {
                    if !self.handle_gate(&mut state)? {
                        anyhow::bail!("Run aborted by user at gate {}", aspect.id);
                    }
                    state.current_index += 1;
                }
                AspectType::Worker => {
                    ctx.consume_turn(state.max_turns)?;
                    let template = self.load_prompt("aspect-worker.md")?;

                    let context = Self::format_aspect_context(&state);

                    let prompt = template
                        .replace("{{aspect_name}}", &aspect.name)
                        .replace("{{aspect_prompt}}", &aspect.prompt)
                        .replace("{{aspect_id}}", &aspect.id)
                        .replace("{{request}}", &state.request)
                        .replace("{{goal}}", &state.goal)
                        .replace(
                            "{{fork_prompt}}",
                            state.fork_prompt.as_deref().unwrap_or(""),
                        )
                        .replace("{{context}}", &context);

                    let raw_response = self.client.call(&prompt, aspect.model.as_deref())?;

                    let action = match AspectAction::parse(&raw_response) {
                        Ok(action) => action,
                        Err(_) => {
                            println!("[Driver] DSL parsing failed. Attempting recovery...");
                            let recover_template = self.load_prompt("aspect-recover.md")?;

                            let recover_prompt =
                                format!("{}\n\nBad Response:\n{}", recover_template, raw_response);
                            let recovered_raw =
                                self.client.call(&recover_prompt, aspect.model.as_deref())?;
                            AspectAction::parse(&recovered_raw)?
                        }
                    };

                    match action {
                        AspectAction::Set(value) => {
                            let key = state.contribution_key();
                            state.corpus.insert(key, value);
                        }
                        AspectAction::Continue => {
                            for fork in state.completed_forks.drain(..) {
                                state.corpus.extend(fork.corpus);
                            }
                            state.current_index += 1;
                        }
                        AspectAction::Fork { id, prompt } => {
                            if state.completed_forks.iter().any(|fork| fork.id == id) {
                                anyhow::bail!("fork ID '{}' already exists", id);
                            }
                            let next_index = state.current_index + 1;
                            if next_index >= state.aspects.len() {
                                anyhow::bail!(
                                    "cannot fork from the final Aspect '{}'; no child start Aspect exists",
                                    aspect.id
                                );
                            }
                            let mut branch_path = state.branch_path.clone();
                            branch_path.push(id.clone());
                            let fork_state = PipelineState {
                                fork_id: Some(id.clone()),
                                fork_prompt: Some(prompt.clone()),
                                branch_path,
                                completed_forks: Vec::new(),
                                request: state.request.clone(),
                                goal: state.goal.clone(),
                                current_index: next_index,
                                aspects: state.aspects.clone(),
                                corpus: state.corpus.clone(),
                                max_turns: state.max_turns,
                            };
                            let fork_outcome = self.run_inner(ctx, fork_state)?;
                            if fork_outcome.stopped {
                                return Ok(RunOutcome {
                                    results: all_results,
                                    corpus: state.corpus,
                                    stopped: true,
                                });
                            }
                            state.completed_forks.push(CompletedFork {
                                id,
                                prompt,
                                corpus: fork_outcome.corpus,
                            });
                        }
                        AspectAction::Jump(id) => {
                            Self::ensure_no_unjoined_forks(&state, "JUMP")?;
                            state.jump_to_id(&id)?;
                        }
                        AspectAction::Stop(reason) => {
                            Self::ensure_no_unjoined_forks(&state, "STOP")?;
                            println!("[Driver] LLM stopped the run: {reason}");
                            return Ok(RunOutcome {
                                results: all_results,
                                corpus: state.corpus,
                                stopped: true,
                            });
                        }
                        AspectAction::Done(result) => {
                            Self::ensure_no_unjoined_forks(&state, "DONE")?;
                            println!("[Driver] LLM signaled completion.");
                            if state.fork_id.is_none() {
                                all_results.push(result);
                            }
                            return Ok(RunOutcome {
                                results: all_results,
                                corpus: state.corpus,
                                stopped: false,
                            });
                        }
                    }
                }
            }
        }

        Ok(RunOutcome {
            results: all_results,
            corpus: state.corpus,
            stopped: false,
        })
    }

    fn ensure_no_unjoined_forks(state: &PipelineState, action: &str) -> Result<()> {
        if !state.completed_forks.is_empty() {
            anyhow::bail!("{action} cannot discard unjoined fork work; use CONTINUE first");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Aspect, AspectType};
    use std::sync::Mutex;

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
        let fork_result = "DONE Fork 1 Final".to_string();
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

        let driver = Driver::new(client, temp_dir);
        let mut ctx = SessionContext::new();
        let state = PipelineState {
            fork_id: None,
            fork_prompt: None,
            branch_path: Vec::new(),
            completed_forks: Vec::new(),
            request: "Request".to_string(),
            goal: "Goal".to_string(),
            current_index: 0,
            aspects: vec![
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
            corpus: std::collections::HashMap::new(),
            max_turns: std::num::NonZeroUsize::new(10).expect("turn limit is non-zero"),
        };

        let results = driver.run(&mut ctx, state).unwrap();

        assert_eq!(results, vec!["Parent Final"]);
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
        let driver = Driver::new(client, temp_dir);
        let mut ctx = SessionContext::new();
        let state = PipelineState::new(
            "Initial".to_string(),
            "Goal".to_string(),
            vec![Aspect {
                id: "a1".to_string(),
                name: "Aspect 1".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Prompt".to_string(),
                model: None,
            }],
            std::num::NonZeroUsize::new(2).expect("turn limit is non-zero"),
        );

        let error = driver
            .run(&mut ctx, state)
            .expect_err("loop should hit the turn limit");
        assert!(error.to_string().contains("max_turns limit of 2"));
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
        let driver = Driver::new(client, temp_dir);
        let mut ctx = SessionContext::new();
        let state = PipelineState::new(
            "Initial".to_string(),
            "Goal".to_string(),
            vec![Aspect {
                id: "a1".to_string(),
                name: "Aspect 1".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Prompt".to_string(),
                model: None,
            }],
            std::num::NonZeroUsize::new(3).expect("turn limit is non-zero"),
        );

        let results = driver
            .run(&mut ctx, state)
            .expect("recovery should succeed");
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
        let driver = Driver::new(client, temp_dir);
        let mut ctx = SessionContext::new();
        let state = PipelineState::new(
            "Initial".to_string(),
            "Goal".to_string(),
            vec![Aspect {
                id: "a1".to_string(),
                name: "Aspect 1".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Prompt".to_string(),
                model: None,
            }],
            std::num::NonZeroUsize::new(3).expect("turn limit is non-zero"),
        );

        let results = driver.run(&mut ctx, state).expect("run should succeed");
        assert_eq!(results, vec!["Final result"]);

        let prompts = driver
            .client
            .prompts
            .lock()
            .expect("prompts lock should not be poisoned");
        assert_eq!(
            prompts.as_slice(),
            [
                "Context:\nASPECT a1",
                "Context:\nKNOWLEDGE Preserve compatibility\nASPECT a1",
                "Context:\nKNOWLEDGE Preserve the stable API\nASPECT a1",
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
        let driver = Driver::new(client, temp_dir);
        let mut ctx = SessionContext::new();
        let state = PipelineState::new(
            "Keep this request intact".to_string(),
            "Goal".to_string(),
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
        );

        let results = driver.run(&mut ctx, state).expect("run should succeed");
        assert_eq!(results, vec!["Final result"]);

        let prompts = driver
            .client
            .prompts
            .lock()
            .expect("prompts lock should not be poisoned");
        assert_eq!(
            prompts.as_slice(),
            [
                "Request: Keep this request intact\nContext:\nASPECT a1\nASPECT a2",
                "Request: Keep this request intact\nContext:\nKNOWLEDGE Preserve compatibility\nASPECT a1\nASPECT a2",
                "Request: Keep this request intact\nContext:\nKNOWLEDGE Preserve the stable API\nASPECT a1\nASPECT a2",
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
        let driver = Driver::new(client, temp_dir);
        let mut ctx = SessionContext::new();
        let state = PipelineState::new(
            "Initial".to_string(),
            "Goal".to_string(),
            vec![Aspect {
                id: "a1".to_string(),
                name: "Aspect 1".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Prompt".to_string(),
                model: None,
            }],
            std::num::NonZeroUsize::new(2).expect("turn limit is non-zero"),
        );

        let results = driver.run(&mut ctx, state).expect("stop should be clean");
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
        let driver = Driver::new(client, temp_dir);
        let mut ctx = SessionContext::new();
        let state = PipelineState::new(
            "Initial".to_string(),
            "Goal".to_string(),
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
        );

        let error = driver
            .run(&mut ctx, state)
            .expect_err("DONE with unjoined fork work should fail");
        assert!(error.to_string().contains("use CONTINUE first"));
    }
}
