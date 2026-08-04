use crate::dsl::AspectAction;
use crate::identifier::PipelineId;
use crate::run_id::RunId;
use crate::state::PipelineState;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Monotonic schema tag for on-disk run snapshots. Stored as a plain integer so a future
/// version an older binary does not recognize is representable and rejected by
/// `validate`, rather than failing opaquely inside serde.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemaVersion(pub u32);

/// The only snapshot schema this binary writes and trusts.
pub const CURRENT_SCHEMA_VERSION: SchemaVersion = SchemaVersion(1);

/// Observable lifecycle state of a durable run. The final four variants are terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Ready,
    Running,
    WaitingAtGate,
    InterruptedCall,
    Done,
    Stopped,
    Aborted,
    Failed,
}

impl RunStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Done | Self::Stopped | Self::Aborted | Self::Failed
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Running => "running",
            Self::WaitingAtGate => "waiting_at_gate",
            Self::InterruptedCall => "interrupted_call",
            Self::Done => "done",
            Self::Stopped => "stopped",
            Self::Aborted => "aborted",
            Self::Failed => "failed",
        }
    }
}

/// The outcome recorded once a run reaches a terminal status. Tied to `RunStatus`: a
/// non-terminal run carries none, and each terminal status pairs with exactly one variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalOutcome {
    Done { result: Vec<String> },
    Stopped { reason: String },
    Aborted,
    Failed { error: String },
}

/// Whether a recorded turn is an ordinary Worker call or a formatter-recovery retry after
/// a malformed Worker response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnKind {
    Worker,
    FormatterRecovery,
}

/// How far a single model call has progressed. Checkpoints move a turn forward through
/// these phases so a crash mid-call is recoverable without repeating the external call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallPhase {
    /// Persisted before the model call; no output exists yet.
    Calling,
    /// The exact raw output is stored but not yet parsed or applied.
    Received { raw_output: String },
    /// The stored raw output was parsed into an accepted transition and applied.
    Applied {
        raw_output: String,
        action: AspectAction,
    },
}

/// One model call within a run: the rendered prompt sent, whether it was a Worker or
/// recovery call, and how far it progressed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRecord {
    pub kind: TurnKind,
    pub prompt: String,
    pub phase: CallPhase,
}

/// A schema-versioned, self-contained snapshot of a durable run. It carries everything
/// needed to resume without reconstructing execution from the current source tree.
#[derive(Debug, Serialize, Deserialize)]
pub struct RunSnapshot {
    pub schema_version: SchemaVersion,
    pub run_id: RunId,
    pub project_root: PathBuf,
    pub config_path: PathBuf,
    pub pipeline_path: PathBuf,
    pub prompt_path: PathBuf,
    pub state: PipelineState,
    pub turns: Vec<TurnRecord>,
    pub status: RunStatus,
    pub terminal: Option<TerminalOutcome>,
}

impl RunSnapshot {
    /// The pipeline this run belongs to. Held authoritatively by the embedded state; the
    /// snapshot does not duplicate it.
    pub fn pipeline_id(&self) -> &PipelineId {
        &self.state.pipeline_id
    }

    /// A run whose latest recorded call was persisted as `calling` with no stored response
    /// crashed mid-call. Resuming such a snapshot must surface `interrupted_call` and never
    /// silently repeat the possibly-completed external call.
    pub fn has_interrupted_call(&self) -> bool {
        matches!(
            self.turns.last().map(|turn| &turn.phase),
            Some(CallPhase::Calling)
        )
    }

    /// Check internal consistency the spec demands. Full path and pointer-identity checks
    /// belong to the load slice, which compares the snapshot against the directory it came
    /// from; those are not performed here.
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.schema_version == CURRENT_SCHEMA_VERSION,
            "unknown run snapshot schema version {}",
            self.schema_version.0
        );

        let agrees = match (self.status, &self.terminal) {
            (RunStatus::Done, Some(TerminalOutcome::Done { .. })) => true,
            (RunStatus::Stopped, Some(TerminalOutcome::Stopped { .. })) => true,
            (RunStatus::Aborted, Some(TerminalOutcome::Aborted)) => true,
            (RunStatus::Failed, Some(TerminalOutcome::Failed { .. })) => true,
            (status, None) => !status.is_terminal(),
            _ => false,
        };
        anyhow::ensure!(
            agrees,
            "run status '{}' disagrees with its terminal outcome",
            self.status.label()
        );

        self.state.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Aspect, AspectType};
    use std::num::NonZeroUsize;

    fn pipeline_id() -> PipelineId {
        PipelineId::new("test-pipeline").expect("test pipeline ID should be valid")
    }

    fn sample_state() -> PipelineState {
        let mut state = PipelineState::new(
            pipeline_id(),
            "Request".to_string(),
            vec![Aspect {
                id: "research".to_string(),
                name: "Research".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Find evidence".to_string(),
                model: None,
            }],
            NonZeroUsize::new(8).expect("turn limit should be non-zero"),
            2,
        );
        state
            .push_fork("market".to_string(), "Study the market".to_string())
            .expect("fork should start");
        state
    }

    fn snapshot(status: RunStatus, terminal: Option<TerminalOutcome>) -> RunSnapshot {
        RunSnapshot {
            schema_version: CURRENT_SCHEMA_VERSION,
            run_id: RunId::generate(),
            project_root: PathBuf::from("/project"),
            config_path: PathBuf::from("/project/piper.toml"),
            pipeline_path: PathBuf::from("/project/pipeline.toml"),
            prompt_path: PathBuf::from("/project/prompts"),
            state: sample_state(),
            turns: Vec::new(),
            status,
            terminal,
        }
    }

    #[test]
    fn is_terminal_marks_exactly_the_final_four() {
        let terminal = [
            RunStatus::Done,
            RunStatus::Stopped,
            RunStatus::Aborted,
            RunStatus::Failed,
        ];
        let live = [
            RunStatus::Ready,
            RunStatus::Running,
            RunStatus::WaitingAtGate,
            RunStatus::InterruptedCall,
        ];

        assert!(terminal.iter().all(|status| status.is_terminal()));
        assert!(live.iter().all(|status| !status.is_terminal()));
    }

    #[test]
    fn label_matches_spec_wire_strings() {
        let cases = [
            (RunStatus::Ready, "ready"),
            (RunStatus::Running, "running"),
            (RunStatus::WaitingAtGate, "waiting_at_gate"),
            (RunStatus::InterruptedCall, "interrupted_call"),
            (RunStatus::Done, "done"),
            (RunStatus::Stopped, "stopped"),
            (RunStatus::Aborted, "aborted"),
            (RunStatus::Failed, "failed"),
        ];

        for (status, expected) in cases {
            assert_eq!(status.label(), expected);
        }
    }

    #[test]
    fn validate_accepts_agreeing_status_and_outcome() {
        let done = snapshot(
            RunStatus::Done,
            Some(TerminalOutcome::Done {
                result: vec!["Final".to_string()],
            }),
        );
        done.validate().expect("done with a result should validate");

        let running = snapshot(RunStatus::Running, None);
        running
            .validate()
            .expect("a non-terminal run with no outcome should validate");
    }

    #[test]
    fn validate_rejects_terminal_status_without_outcome() {
        let orphan = snapshot(RunStatus::Done, None);

        let error = orphan
            .validate()
            .expect_err("a done run with no outcome must be rejected");
        assert!(error.to_string().contains("terminal outcome"));
    }

    #[test]
    fn validate_rejects_nonterminal_status_with_outcome() {
        let mismatch = snapshot(
            RunStatus::Running,
            Some(TerminalOutcome::Stopped {
                reason: "impossible".to_string(),
            }),
        );

        assert!(
            mismatch.validate().is_err(),
            "a running run cannot carry a terminal outcome"
        );
    }

    #[test]
    fn validate_rejects_mismatched_terminal_variant() {
        let mismatch = snapshot(
            RunStatus::Done,
            Some(TerminalOutcome::Stopped {
                reason: "wrong variant".to_string(),
            }),
        );

        assert!(
            mismatch.validate().is_err(),
            "a done run must carry a done outcome, not a stopped one"
        );
    }

    #[test]
    fn validate_rejects_unknown_schema_version() {
        let mut snapshot = snapshot(RunStatus::Running, None);
        snapshot.schema_version = SchemaVersion(CURRENT_SCHEMA_VERSION.0 + 1);

        let error = snapshot
            .validate()
            .expect_err("an unknown schema version must be rejected");
        assert!(error.to_string().contains("schema version"));
    }

    #[test]
    fn round_trips_through_json_preserving_turns_and_nested_frames() {
        let mut original = snapshot(
            RunStatus::Done,
            Some(TerminalOutcome::Done {
                result: vec!["Final".to_string()],
            }),
        );
        original.turns = vec![
            TurnRecord {
                kind: TurnKind::Worker,
                prompt: "Worker prompt".to_string(),
                phase: CallPhase::Applied {
                    raw_output: "SET Evidence gathered".to_string(),
                    action: AspectAction::Set("Evidence gathered".to_string()),
                },
            },
            TurnRecord {
                kind: TurnKind::FormatterRecovery,
                prompt: "Recovery prompt".to_string(),
                phase: CallPhase::Calling,
            },
        ];

        let json = serde_json::to_string(&original).expect("snapshot should serialize");
        let restored: RunSnapshot =
            serde_json::from_str(&json).expect("snapshot should deserialize");

        restored
            .validate()
            .expect("a round-tripped snapshot should still validate");
        assert_eq!(restored.turns, original.turns);
        assert_eq!(restored.state.frames.len(), 2);
        assert_eq!(restored.state.frame().fork_id(), Some("market"));
    }
}
