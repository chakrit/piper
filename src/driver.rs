use crate::checkpointer::Checkpointer;
use crate::dsl::AspectAction;
use crate::llm::LlmClient;
use crate::models::{Aspect, AspectType};
use crate::prompt_templates::PromptTemplates;
use crate::run_snapshot::{
    CallPhase, RunSnapshot, RunStatus, TerminalOutcome, TurnKind, TurnRecord,
};
use crate::state::PipelineState;
use anyhow::{Context, Result};

pub struct Driver<C: LlmClient, K: Checkpointer> {
    client: C,
    prompt_templates: PromptTemplates,
    checkpointer: K,
}

/// Operator's decision at a gate, supplied to `resume`. The engine never reads it
/// from the terminal.
#[derive(Debug, Clone, Copy)]
pub enum GateDecision {
    Continue,
    Abort,
}

/// Whether an accepted action left the run mid-flight or drove it to a terminal.
enum StepOutcome {
    Continued,
    Terminal(TerminalOutcome),
}

/// What opening a snapshot demands before any new model call: either resume normally, or
/// halt on a crash captured in the trailing turn.
enum Recovery {
    Fresh,
    Halt,
}

impl<C: LlmClient, K: Checkpointer> Driver<C, K> {
    pub fn new(client: C, prompt_templates: PromptTemplates, checkpointer: K) -> Self {
        Self {
            client,
            prompt_templates,
            checkpointer,
        }
    }

    /// Drive a snapshot forward from a fresh start or after a crash. A trailing `Calling`
    /// turn surfaces `interrupted_call` and stops; a trailing un-applied `Received` is
    /// applied without repeating the model call, per the crash-recovery spec.
    pub fn advance(&self, mut snapshot: RunSnapshot) -> Result<RunSnapshot> {
        snapshot.validate()?;

        match self.recover_pending(&mut snapshot)? {
            Recovery::Halt => Ok(snapshot),
            Recovery::Fresh => {
                snapshot.status = RunStatus::Running;
                self.drive(snapshot)
            }
        }
    }

    /// Resume a run paused at a gate, applying the operator's decision and checkpointing it.
    /// `Continue` steps past the gate and drives on; `Abort` ends the run without a Result.
    /// Approval is an explicit input, never a terminal read.
    pub fn resume(&self, mut snapshot: RunSnapshot, decision: GateDecision) -> Result<RunSnapshot> {
        anyhow::ensure!(
            matches!(
                snapshot.state.current_aspect().aspect_type,
                AspectType::Gate
            ),
            "resume requires the run to be waiting at a gate"
        );

        match decision {
            GateDecision::Continue => {
                snapshot.state.frame_mut().current_index += 1;
                snapshot.status = RunStatus::Running;
                self.checkpointer.checkpoint(&snapshot)?;
                self.drive(snapshot)
            }
            GateDecision::Abort => self.abort(snapshot),
        }
    }

    /// Record an operator abort on any non-terminal run and checkpoint it. Consumes no
    /// Worker turn or recovery and does not require the run to be waiting at a gate, so the
    /// `abort` command can end a run stopped anywhere — at a gate, or interrupted mid-call.
    pub fn abort(&self, mut snapshot: RunSnapshot) -> Result<RunSnapshot> {
        Self::set_terminal(&mut snapshot, TerminalOutcome::Aborted);
        self.checkpointer.checkpoint(&snapshot)?;
        Ok(snapshot)
    }

    fn recover_pending(&self, snapshot: &mut RunSnapshot) -> Result<Recovery> {
        if snapshot.has_interrupted_call() {
            snapshot.status = RunStatus::InterruptedCall;
            self.checkpointer.checkpoint(snapshot)?;
            return Ok(Recovery::Halt);
        }

        let Some(pending) = snapshot.turns.last() else {
            return Ok(Recovery::Fresh);
        };

        match &pending.phase {
            CallPhase::Calling | CallPhase::Applied { .. } => Ok(Recovery::Fresh),
            CallPhase::Received { raw_output } => {
                let raw = raw_output.clone();
                let model = snapshot.state.current_aspect().model.clone();
                snapshot.status = RunStatus::Running;
                self.apply_response(snapshot, raw, model.as_deref())?;
                match snapshot.status.is_terminal() {
                    true => Ok(Recovery::Halt),
                    false => Ok(Recovery::Fresh),
                }
            }
        }
    }

    fn drive(&self, mut snapshot: RunSnapshot) -> Result<RunSnapshot> {
        loop {
            if snapshot.state.frame().current_index >= snapshot.state.aspects.len() {
                if snapshot.state.is_root() {
                    Self::set_terminal(&mut snapshot, TerminalOutcome::Done { result: Vec::new() });
                    self.checkpointer.checkpoint(&snapshot)?;
                    return Ok(snapshot);
                }
                Self::ensure_no_unjoined_forks(&snapshot.state, "return")?;
                snapshot.state.return_current_fork()?;
                continue;
            }

            let aspect = snapshot.state.current_aspect().clone();
            match aspect.aspect_type {
                AspectType::Gate => {
                    Self::log_aspect_started(&snapshot.state, "Gate");
                    println!("│ Aspect    {} · gate", aspect.id);
                    snapshot.status = RunStatus::WaitingAtGate;
                    self.checkpointer.checkpoint(&snapshot)?;
                    return Ok(snapshot);
                }
                AspectType::Worker => {
                    self.worker_turn(&mut snapshot, &aspect)?;
                    if snapshot.status.is_terminal() {
                        return Ok(snapshot);
                    }
                }
            }
        }
    }

    /// One Worker call: consume a turn, checkpoint `Calling`, call the model, checkpoint the
    /// exact raw output as `Received`, then parse and apply it.
    fn worker_turn(&self, snapshot: &mut RunSnapshot, aspect: &Aspect) -> Result<()> {
        snapshot.state.consume_turn()?;
        Self::log_worker_started(
            &snapshot.state,
            aspect,
            self.client.resolve_model(aspect.model.as_deref()),
        );

        let prompt = self.render_worker_prompt(&snapshot.state, aspect)?;
        snapshot.turns.push(TurnRecord {
            kind: TurnKind::Worker,
            prompt: prompt.clone(),
            phase: CallPhase::Calling,
        });
        self.checkpointer.checkpoint(snapshot)?;

        println!("│ LLM       requesting action");
        let raw = self.client.call(&prompt, aspect.model.as_deref())?;
        Self::mark_received(snapshot, raw.clone());
        self.checkpointer.checkpoint(snapshot)?;

        self.apply_response(snapshot, raw, aspect.model.as_deref())
    }

    /// Parse a stored raw response and apply the accepted transition, recovering a malformed
    /// response first. Marks the turn `Applied`, updates the run status, and checkpoints.
    fn apply_response(
        &self,
        snapshot: &mut RunSnapshot,
        raw: String,
        model: Option<&str>,
    ) -> Result<()> {
        let (raw, action) = self.parse_or_recover(snapshot, raw, model)?;

        Self::mark_applied(snapshot, raw, action.clone());
        match Self::apply_action(&mut snapshot.state, action)? {
            StepOutcome::Continued => {}
            StepOutcome::Terminal(outcome) => Self::set_terminal(snapshot, outcome),
        }
        self.checkpointer.checkpoint(snapshot)?;
        Ok(())
    }

    /// Turn a raw response into an accepted action. A malformed response spends a recovery,
    /// records a `FormatterRecovery` turn through its own `Calling`/`Received` phases, and
    /// leaves that recovery turn as the one to be applied.
    fn parse_or_recover(
        &self,
        snapshot: &mut RunSnapshot,
        raw: String,
        model: Option<&str>,
    ) -> Result<(String, AspectAction)> {
        let error = match AspectAction::parse(&raw) {
            Ok(action) => {
                Self::log_action("Action", &action);
                return Ok((raw, action));
            }
            Err(error) => error,
        };

        snapshot.state.consume_recovery()?;
        println!(
            "│ Recovery  {}/{} · {}",
            snapshot.state.recoveries_taken, snapshot.state.max_recoveries, error
        );
        println!("│ Rejected  {}", one_line(&raw));

        let template = self.prompt_templates.recover(&crate::actions::AppContext)?;
        let recover_prompt = format!("{template}\n\nBad Response:\n{raw}");
        snapshot.turns.push(TurnRecord {
            kind: TurnKind::FormatterRecovery,
            prompt: recover_prompt.clone(),
            phase: CallPhase::Calling,
        });
        self.checkpointer.checkpoint(snapshot)?;

        println!("│ LLM       requesting formatter recovery");
        let recovered = self.client.call(&recover_prompt, model)?;
        Self::mark_received(snapshot, recovered.clone());
        self.checkpointer.checkpoint(snapshot)?;

        let action = AspectAction::parse(&recovered).with_context(|| {
            format!(
                "Recovery response was not valid Piper DSL: {}",
                one_line(&recovered)
            )
        })?;
        Self::log_action("Recovered", &action);
        Ok((recovered, action))
    }

    fn apply_action(state: &mut PipelineState, action: AspectAction) -> Result<StepOutcome> {
        match action {
            AspectAction::Set(value) => {
                let key = state.contribution_key();
                state.frame_mut().corpus.insert(key, value);
                println!(
                    "╰─ Saved contribution · {} corpus entries",
                    state.frame().corpus.len()
                );
                Ok(StepOutcome::Continued)
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
                Ok(StepOutcome::Continued)
            }
            AspectAction::Fork { id, prompt } => {
                let fork_id = id.clone();
                state.push_fork(id, prompt)?;
                println!(
                    "╰─ Entered fork {fork_id} · depth {}",
                    state.frames.len() - 1
                );
                Ok(StepOutcome::Continued)
            }
            AspectAction::Jump(id) => {
                Self::ensure_no_unjoined_forks(state, "JUMP")?;
                state.jump_to_id(&id)?;
                println!("╰─ Jumped to Aspect {id}");
                Ok(StepOutcome::Continued)
            }
            AspectAction::Stop(reason) => {
                Self::ensure_no_unjoined_forks(state, "STOP")?;
                println!("╰─ Stop accepted");
                Ok(StepOutcome::Terminal(TerminalOutcome::Stopped { reason }))
            }
            AspectAction::Done(result) => {
                Self::ensure_no_unjoined_forks(state, "DONE")?;
                println!("╰─ Completion accepted");
                Ok(StepOutcome::Terminal(TerminalOutcome::Done {
                    result: vec![result],
                }))
            }
        }
    }

    fn render_worker_prompt(&self, state: &PipelineState, aspect: &Aspect) -> Result<String> {
        let template = self.prompt_templates.worker(&crate::actions::AppContext)?;
        let context = Self::format_aspect_context(state);

        Ok(template
            .replace("{{aspect_name}}", &aspect.name)
            .replace("{{aspect_prompt}}", &aspect.prompt)
            .replace("{{aspect_id}}", &aspect.id)
            .replace("{{request}}", &state.request)
            .replace("{{fork_prompt}}", state.frame().fork_prompt().unwrap_or(""))
            .replace("{{context}}", &context))
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

    fn set_terminal(snapshot: &mut RunSnapshot, outcome: TerminalOutcome) {
        snapshot.status = match &outcome {
            TerminalOutcome::Done { .. } => RunStatus::Done,
            TerminalOutcome::Stopped { .. } => RunStatus::Stopped,
            TerminalOutcome::Aborted => RunStatus::Aborted,
            TerminalOutcome::Failed { .. } => RunStatus::Failed,
        };
        snapshot.terminal = Some(outcome);
    }

    fn mark_received(snapshot: &mut RunSnapshot, raw: String) {
        let turn = snapshot
            .turns
            .last_mut()
            .expect("a call turn was recorded before its output");
        turn.phase = CallPhase::Received { raw_output: raw };
    }

    fn mark_applied(snapshot: &mut RunSnapshot, raw: String, action: AspectAction) {
        let turn = snapshot
            .turns
            .last_mut()
            .expect("a call turn was recorded before it is applied");
        turn.phase = CallPhase::Applied {
            raw_output: raw,
            action,
        };
    }

    fn log_aspect_started(state: &PipelineState, kind: &str) {
        println!(
            "\n╭─ {kind} {}/{} · {}",
            state.frame().current_index + 1,
            state.aspects.len(),
            state.current_aspect().name
        );
    }

    fn log_worker_started(state: &PipelineState, aspect: &Aspect, model: &str) {
        Self::log_aspect_started(state, "Worker");
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
            state.turns_taken, state.max_turns, state.recoveries_taken, state.max_recoveries
        );
        println!(
            "│ State     {} corpus entries · {} pending forks",
            state.frame().corpus.len(),
            state.frame().completed_forks.len()
        );
        println!("│ Model     {model}");
    }

    fn log_action(label: &str, action: &AspectAction) {
        match action {
            AspectAction::Set(value) => println!("│ {label:<10}SET · {value}"),
            AspectAction::Continue => println!("│ {label:<10}CONTINUE"),
            AspectAction::Fork { id, prompt } => println!("│ {label:<10}FORK {id} · {prompt}"),
            AspectAction::Jump(id) => println!("│ {label:<10}JUMP · {id}"),
            AspectAction::Stop(reason) => println!("│ {label:<10}STOP · {reason}"),
            AspectAction::Done(result) => println!("│ {label:<10}DONE · {result}"),
        }
    }

    fn ensure_no_unjoined_forks(state: &PipelineState, action: &str) -> Result<()> {
        if !state.frame().completed_forks.is_empty() {
            anyhow::bail!("{action} cannot discard unjoined fork work; use CONTINUE first");
        }

        Ok(())
    }
}

pub fn log_run_started(state: &PipelineState) {
    println!("╭─ Piper run · {}", state.pipeline_id.as_str());
    println!("│ Request   {}", one_line(&state.request));
    println!("│ Aspects   {}", state.aspects.len());
    println!(
        "│ Budgets   {} turns · {} recoveries",
        state.max_turns, state.max_recoveries
    );
    println!("╰─ Starting");
}

pub fn log_completed(state: &PipelineState, output_count: usize) {
    let output_label = if output_count == 1 {
        "output"
    } else {
        "outputs"
    };
    println!("\n✓ Completed · {output_count} {output_label}");
    log_budget_used(state);
}

pub fn log_budget_used(state: &PipelineState) {
    println!(
        "  Budget used · {}/{} turns · {}/{} recoveries",
        state.turns_taken, state.max_turns, state.recoveries_taken, state.max_recoveries
    );
}

pub fn one_line(value: &str) -> String {
    value.replace('\r', "\\r").replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpointer::NoopCheckpointer;
    use crate::identifier::PipelineId;
    use crate::models::{Aspect, AspectType};
    use crate::run_id::RunId;
    use crate::run_snapshot::{
        CURRENT_SCHEMA_VERSION, CallPhase, RunStatus, TerminalOutcome, TurnKind, TurnRecord,
    };
    use std::path::PathBuf;
    use std::sync::Mutex;

    fn pipeline_id() -> PipelineId {
        PipelineId::new("test-pipeline").expect("test pipeline ID should be valid")
    }

    fn snapshot_of(state: PipelineState) -> RunSnapshot {
        RunSnapshot {
            schema_version: CURRENT_SCHEMA_VERSION,
            run_id: RunId::generate(),
            project_root: PathBuf::from("/project"),
            config_path: PathBuf::from("/project/piper.toml"),
            pipeline_path: PathBuf::from("/project/pipeline.toml"),
            prompt_path: PathBuf::from("/project/prompts"),
            state,
            turns: Vec::new(),
            status: RunStatus::Ready,
            terminal: None,
        }
    }

    /// A checkpoint reduced to what the spec makes observable: the run status and the phase
    /// of its latest recorded call. Tests assert the exact sequence of these.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Phase {
        None,
        Calling(TurnKind),
        Received(TurnKind),
        Applied(TurnKind),
    }

    fn mark(snapshot: &RunSnapshot) -> (RunStatus, Phase) {
        let phase = match snapshot.turns.last() {
            None => Phase::None,
            Some(turn) => match turn.phase {
                CallPhase::Calling => Phase::Calling(turn.kind),
                CallPhase::Received { .. } => Phase::Received(turn.kind),
                CallPhase::Applied { .. } => Phase::Applied(turn.kind),
            },
        };
        (snapshot.status, phase)
    }

    #[derive(Default)]
    struct RecordingCheckpointer {
        marks: Mutex<Vec<(RunStatus, Phase)>>,
    }

    impl Checkpointer for RecordingCheckpointer {
        fn checkpoint(&self, snapshot: &RunSnapshot) -> Result<()> {
            self.marks
                .lock()
                .expect("checkpoint marks lock should not be poisoned")
                .push(mark(snapshot));
            Ok(())
        }
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

    /// Fails the test if the model is ever called. Proves resume applies a stored response
    /// without repeating the external call.
    struct PanickingLlmClient;

    impl LlmClient for PanickingLlmClient {
        fn call(&self, _prompt: &str, _model: Option<&str>) -> Result<String> {
            panic!("the model must not be called when a stored response is being applied");
        }
    }

    /// Drives a gateless pipeline to its terminal status, mirroring how the attended
    /// wrapper maps terminals to a Result vector.
    fn advance_to_results<C: LlmClient, K: Checkpointer>(
        driver: &Driver<C, K>,
        state: PipelineState,
    ) -> Result<Vec<String>> {
        let final_snapshot = driver.advance(snapshot_of(state))?;
        match final_snapshot.terminal {
            Some(TerminalOutcome::Done { result }) => Ok(result),
            Some(TerminalOutcome::Stopped { .. }) | Some(TerminalOutcome::Aborted) => {
                Ok(Vec::new())
            }
            Some(TerminalOutcome::Failed { error }) => Err(anyhow::anyhow!(error)),
            None => match final_snapshot.status {
                RunStatus::WaitingAtGate => panic!("gateless pipeline paused at a gate"),
                other => panic!("run ended without a terminal outcome: {}", other.label()),
            },
        }
    }

    fn worker(id: &str) -> Aspect {
        Aspect {
            id: id.to_string(),
            name: id.to_string(),
            aspect_type: AspectType::Worker,
            prompt: "Prompt".to_string(),
            model: None,
        }
    }

    fn gate(id: &str) -> Aspect {
        Aspect {
            id: id.to_string(),
            name: id.to_string(),
            aspect_type: AspectType::Gate,
            prompt: String::new(),
            model: None,
        }
    }

    fn templates_dir(name: &str) -> PromptTemplates {
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).expect("template directory should be created");
        std::fs::write(dir.join("aspect-worker.md"), "Prompt: {{aspect_prompt}}")
            .expect("worker template should be written");
        std::fs::write(dir.join("aspect-recover.md"), "Recovery template")
            .expect("recovery template should be written");
        PromptTemplates::directory(dir)
    }

    #[test]
    fn checkpoints_each_call_phase_the_gate_decision_and_the_terminal() {
        let client = MockLlmClient {
            responses: Mutex::new(vec!["CONTINUE".to_string(), "DONE result".to_string()]),
        };
        let driver = Driver::new(
            client,
            templates_dir("piper_ckpt_sequence"),
            RecordingCheckpointer::default(),
        );
        let state = PipelineState::new(
            pipeline_id(),
            "Request".to_string(),
            vec![worker("w1"), gate("g"), worker("w2")],
            std::num::NonZeroUsize::new(4).expect("turn limit is non-zero"),
            0,
        );

        let paused = driver
            .advance(snapshot_of(state))
            .expect("advance should pause at the gate");
        assert_eq!(paused.status, RunStatus::WaitingAtGate);

        let finished = driver
            .resume(paused, GateDecision::Continue)
            .expect("resume should complete the run");
        assert_eq!(finished.status, RunStatus::Done);
        assert_eq!(
            finished.terminal,
            Some(TerminalOutcome::Done {
                result: vec!["result".to_string()],
            })
        );

        let marks = driver
            .checkpointer
            .marks
            .lock()
            .expect("marks lock should not be poisoned");
        assert_eq!(
            marks.as_slice(),
            [
                (RunStatus::Running, Phase::Calling(TurnKind::Worker)),
                (RunStatus::Running, Phase::Received(TurnKind::Worker)),
                (RunStatus::Running, Phase::Applied(TurnKind::Worker)),
                (RunStatus::WaitingAtGate, Phase::Applied(TurnKind::Worker)),
                (RunStatus::Running, Phase::Applied(TurnKind::Worker)),
                (RunStatus::Running, Phase::Calling(TurnKind::Worker)),
                (RunStatus::Running, Phase::Received(TurnKind::Worker)),
                (RunStatus::Done, Phase::Applied(TurnKind::Worker)),
            ]
        );
    }

    #[test]
    fn a_trailing_calling_turn_is_an_interrupted_call() {
        let mut snapshot = snapshot_of(PipelineState::new(
            pipeline_id(),
            "Request".to_string(),
            vec![worker("w1")],
            std::num::NonZeroUsize::new(4).expect("turn limit is non-zero"),
            0,
        ));
        snapshot.status = RunStatus::Running;
        snapshot
            .state
            .consume_turn()
            .expect("turn should be consumed");
        snapshot.turns.push(TurnRecord {
            kind: TurnKind::Worker,
            prompt: "Prompt".to_string(),
            phase: CallPhase::Calling,
        });
        assert!(snapshot.has_interrupted_call());

        let driver = Driver::new(
            PanickingLlmClient,
            templates_dir("piper_ckpt_interrupted"),
            RecordingCheckpointer::default(),
        );

        let resumed = driver
            .advance(snapshot)
            .expect("opening an interrupted snapshot should not error");
        assert_eq!(resumed.status, RunStatus::InterruptedCall);

        let marks = driver
            .checkpointer
            .marks
            .lock()
            .expect("marks lock should not be poisoned");
        assert_eq!(
            marks.as_slice(),
            [(RunStatus::InterruptedCall, Phase::Calling(TurnKind::Worker))]
        );
    }

    #[test]
    fn resume_applies_a_stored_response_without_calling_the_model() {
        let mut snapshot = snapshot_of(PipelineState::new(
            pipeline_id(),
            "Request".to_string(),
            vec![worker("w1")],
            std::num::NonZeroUsize::new(4).expect("turn limit is non-zero"),
            0,
        ));
        snapshot.status = RunStatus::Running;
        snapshot
            .state
            .consume_turn()
            .expect("turn should be consumed");
        snapshot.turns.push(TurnRecord {
            kind: TurnKind::Worker,
            prompt: "Prompt".to_string(),
            phase: CallPhase::Received {
                raw_output: "DONE resumed".to_string(),
            },
        });

        let driver = Driver::new(
            PanickingLlmClient,
            templates_dir("piper_ckpt_reuse"),
            RecordingCheckpointer::default(),
        );

        let finished = driver
            .advance(snapshot)
            .expect("a stored response should apply without the model");
        assert_eq!(finished.status, RunStatus::Done);
        assert_eq!(
            finished.terminal,
            Some(TerminalOutcome::Done {
                result: vec!["resumed".to_string()],
            })
        );

        let marks = driver
            .checkpointer
            .marks
            .lock()
            .expect("marks lock should not be poisoned");
        assert_eq!(
            marks.as_slice(),
            [(RunStatus::Done, Phase::Applied(TurnKind::Worker))]
        );
    }

    #[test]
    fn test_run_with_dsl() {
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

        let driver = Driver::new(client, templates_dir("piper_tests_dsl"), NoopCheckpointer);
        let state = PipelineState::new(
            pipeline_id(),
            "Request".to_string(),
            vec![worker("a1"), worker("a2")],
            std::num::NonZeroUsize::new(10).expect("turn limit is non-zero"),
            3,
        );

        let results = advance_to_results(&driver, state).expect("run should succeed");

        assert_eq!(results, vec!["Parent Final"]);
    }

    fn gate_then_worker_state() -> PipelineState {
        PipelineState::new(
            pipeline_id(),
            "Approve the release".to_string(),
            vec![gate("approval"), {
                let mut ship = worker("ship");
                ship.name = "Ship".to_string();
                ship.prompt = "Ship it".to_string();
                ship
            }],
            std::num::NonZeroUsize::new(2).expect("turn limit should be non-zero"),
            0,
        )
    }

    #[test]
    fn test_advance_pauses_at_gate_and_resume_controls_the_run() {
        let client = MockLlmClient {
            responses: Mutex::new(vec!["DONE Shipped".to_string()]),
        };
        let driver = Driver::new(client, templates_dir("piper_tests_gate"), NoopCheckpointer);

        let paused = driver
            .advance(snapshot_of(gate_then_worker_state()))
            .expect("advance should pause at the gate");
        assert_eq!(paused.status, RunStatus::WaitingAtGate);
        assert_eq!(paused.state.frame().current_index, 0);
        assert_eq!(paused.state.current_aspect().id, "approval");

        let approved = driver
            .resume(paused, GateDecision::Continue)
            .expect("resume should advance past the gate");
        assert_eq!(approved.status, RunStatus::Done);
        assert_eq!(
            approved.terminal,
            Some(TerminalOutcome::Done {
                result: vec!["Shipped".to_string()],
            })
        );

        let paused_again = driver
            .advance(snapshot_of(gate_then_worker_state()))
            .expect("advance should pause at the gate again");
        let aborted = driver
            .resume(paused_again, GateDecision::Abort)
            .expect("abort should end the run");
        assert_eq!(aborted.status, RunStatus::Aborted);
        assert_eq!(aborted.terminal, Some(TerminalOutcome::Aborted));
    }

    #[test]
    fn test_fork_runs_current_aspect_and_returns_knowledge_to_parent() {
        let dir = std::env::temp_dir().join("piper_tests_fork_context");
        std::fs::create_dir_all(&dir).expect("template directory should be created");
        std::fs::write(
            dir.join("aspect-worker.md"),
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
        let driver = Driver::new(client, PromptTemplates::directory(dir), NoopCheckpointer);
        let state = PipelineState::new(
            pipeline_id(),
            "Write a short story".to_string(),
            vec![
                {
                    let mut branch = worker("branch");
                    branch.prompt = "Create branches".to_string();
                    branch
                },
                {
                    let mut story = worker("story");
                    story.prompt = "Compose from the joined Corpus".to_string();
                    story
                },
            ],
            std::num::NonZeroUsize::new(7).expect("turn limit is non-zero"),
            3,
        );

        let results = advance_to_results(&driver, state).expect("run should succeed");
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
        let jump_response = "JUMP a1".to_string();
        let client = MockLlmClient {
            responses: Mutex::new(vec![jump_response.clone(), jump_response]),
        };
        let driver = Driver::new(
            client,
            templates_dir("piper_tests_turn_limit"),
            NoopCheckpointer,
        );
        let state = PipelineState::new(
            pipeline_id(),
            "Initial".to_string(),
            vec![worker("a1")],
            std::num::NonZeroUsize::new(2).expect("turn limit is non-zero"),
            3,
        );

        let error = advance_to_results(&driver, state).expect_err("loop should hit the turn limit");
        assert!(error.to_string().contains("max_turns limit of 2"));
    }

    #[test]
    fn test_final_aspect_can_fork_and_join() {
        let dir = std::env::temp_dir().join("piper_tests_final_fork");
        std::fs::create_dir_all(&dir).expect("template directory should be created");
        std::fs::write(dir.join("aspect-worker.md"), "{{context}}")
            .expect("worker template should be written");

        let client = MockLlmClient {
            responses: Mutex::new(vec![
                "FORK check Verify the final calculation".to_string(),
                "SET VERIFIED 42".to_string(),
                "JOIN".to_string(),
                "CONTINUE".to_string(),
            ]),
        };
        let driver = Driver::new(client, PromptTemplates::directory(dir), NoopCheckpointer);
        let state = PipelineState::new(
            pipeline_id(),
            "Verify the answer".to_string(),
            vec![{
                let mut verify = worker("verify");
                verify.prompt = "Fork one independent check".to_string();
                verify
            }],
            std::num::NonZeroUsize::new(4).expect("turn limit is non-zero"),
            1,
        );

        let results =
            advance_to_results(&driver, state).expect("final Aspect fork should join cleanly");

        assert!(results.is_empty());
    }

    #[test]
    fn test_run_recovers_a_malformed_worker_response() {
        let client = MockLlmClient {
            responses: Mutex::new(vec![
                "not a DSL action".to_string(),
                "DONE Recovered result".to_string(),
            ]),
        };
        let driver = Driver::new(
            client,
            templates_dir("piper_tests_recovery"),
            NoopCheckpointer,
        );
        let state = PipelineState::new(
            pipeline_id(),
            "Initial".to_string(),
            vec![worker("a1")],
            std::num::NonZeroUsize::new(3).expect("turn limit is non-zero"),
            1,
        );

        let results = advance_to_results(&driver, state).expect("recovery should succeed");
        assert_eq!(results, vec!["Recovered result"]);
    }

    #[test]
    fn recovery_calls_checkpoint_through_their_own_phases() {
        let client = MockLlmClient {
            responses: Mutex::new(vec![
                "not a DSL action".to_string(),
                "DONE Recovered".to_string(),
            ]),
        };
        let driver = Driver::new(
            client,
            templates_dir("piper_tests_recovery_marks"),
            RecordingCheckpointer::default(),
        );
        let state = PipelineState::new(
            pipeline_id(),
            "Initial".to_string(),
            vec![worker("a1")],
            std::num::NonZeroUsize::new(3).expect("turn limit is non-zero"),
            1,
        );

        driver
            .advance(snapshot_of(state))
            .expect("recovery should succeed");

        let marks = driver
            .checkpointer
            .marks
            .lock()
            .expect("marks lock should not be poisoned");
        assert_eq!(
            marks.as_slice(),
            [
                (RunStatus::Running, Phase::Calling(TurnKind::Worker)),
                (RunStatus::Running, Phase::Received(TurnKind::Worker)),
                (
                    RunStatus::Running,
                    Phase::Calling(TurnKind::FormatterRecovery)
                ),
                (
                    RunStatus::Running,
                    Phase::Received(TurnKind::FormatterRecovery)
                ),
                (RunStatus::Done, Phase::Applied(TurnKind::FormatterRecovery)),
            ]
        );
    }

    #[test]
    fn test_run_set_replaces_current_worker_contribution() {
        let dir = std::env::temp_dir().join("piper_tests_knowledge");
        std::fs::create_dir_all(&dir).expect("template directory should be created");
        std::fs::write(dir.join("aspect-worker.md"), "Context:\n{{context}}")
            .expect("worker template should be written");

        let client = RecordingMockLlmClient {
            responses: Mutex::new(vec![
                "SET Preserve compatibility".to_string(),
                "SET Preserve the stable API".to_string(),
                "DONE Final result".to_string(),
            ]),
            prompts: Mutex::new(Vec::new()),
        };
        let driver = Driver::new(client, PromptTemplates::directory(dir), NoopCheckpointer);
        let state = PipelineState::new(
            pipeline_id(),
            "Initial".to_string(),
            vec![worker("a1")],
            std::num::NonZeroUsize::new(3).expect("turn limit is non-zero"),
            3,
        );

        let results = advance_to_results(&driver, state).expect("run should succeed");
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
        let dir = std::env::temp_dir().join("piper_tests_update_knowledge");
        std::fs::create_dir_all(&dir).expect("template directory should be created");
        std::fs::write(
            dir.join("aspect-worker.md"),
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
        let driver = Driver::new(client, PromptTemplates::directory(dir), NoopCheckpointer);
        let state = PipelineState::new(
            pipeline_id(),
            "Keep this request intact".to_string(),
            vec![worker("a1"), worker("a2")],
            std::num::NonZeroUsize::new(3).expect("turn limit is non-zero"),
            3,
        );

        let results = advance_to_results(&driver, state).expect("run should succeed");
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
        let client = MockLlmClient {
            responses: Mutex::new(vec!["STOP Missing requirements".to_string()]),
        };
        let driver = Driver::new(client, templates_dir("piper_tests_stop"), NoopCheckpointer);
        let state = PipelineState::new(
            pipeline_id(),
            "Initial".to_string(),
            vec![worker("a1")],
            std::num::NonZeroUsize::new(2).expect("turn limit is non-zero"),
            3,
        );

        let results = advance_to_results(&driver, state).expect("stop should be clean");
        assert!(results.is_empty());
    }

    #[test]
    fn test_done_inside_fork_terminates_whole_run() {
        let client = MockLlmClient {
            responses: Mutex::new(vec![
                "FORK f1 Explore the alternative".to_string(),
                "DONE Whole run complete".to_string(),
            ]),
        };
        let driver = Driver::new(
            client,
            templates_dir("piper_tests_fork_done"),
            NoopCheckpointer,
        );
        let state = PipelineState::new(
            pipeline_id(),
            "Initial".to_string(),
            vec![worker("a1")],
            std::num::NonZeroUsize::new(3).expect("turn limit is non-zero"),
            0,
        );

        let results =
            advance_to_results(&driver, state).expect("child DONE should terminate the whole run");
        assert_eq!(results, vec!["Whole run complete"]);
    }

    #[test]
    fn test_done_rejects_unjoined_fork_work() {
        let client = MockLlmClient {
            responses: Mutex::new(vec![
                "FORK f1 Analyze the alternative".to_string(),
                "CONTINUE".to_string(),
                "DONE Parent complete".to_string(),
            ]),
        };
        let driver = Driver::new(
            client,
            templates_dir("piper_tests_unjoined_fork"),
            NoopCheckpointer,
        );
        let state = PipelineState::new(
            pipeline_id(),
            "Initial".to_string(),
            vec![worker("a1"), worker("a2")],
            std::num::NonZeroUsize::new(3).expect("turn limit is non-zero"),
            3,
        );

        let error = advance_to_results(&driver, state)
            .expect_err("DONE with unjoined fork work should fail");
        assert!(error.to_string().contains("use CONTINUE first"));
    }
}
