use crate::actions::AppContext;
use crate::actions::run_storage::SaveRunSnapshot;
use crate::run_snapshot::RunSnapshot;
use anyhow::Result;
use std::path::PathBuf;

/// Durable sink for run snapshots. The driver hands it a snapshot at every checkpoint
/// boundary; an implementation decides where that snapshot lives. Injecting it keeps the
/// drive logic independent of disk, so tests can observe the checkpoint sequence directly.
pub trait Checkpointer {
    fn checkpoint(&self, snapshot: &RunSnapshot) -> Result<()>;
}

/// Persists each checkpoint to the project state directory through the one owning
/// unit-of-work, `SaveRunSnapshot`. Every mutating command drives the engine through this
/// sink, so a crash between checkpoints leaves the last durable snapshot authoritative.
pub struct DiskCheckpointer {
    pub project_state_dir: PathBuf,
}

impl Checkpointer for DiskCheckpointer {
    fn checkpoint(&self, snapshot: &RunSnapshot) -> Result<()> {
        SaveRunSnapshot {
            project_state_dir: self.project_state_dir.clone(),
            snapshot,
        }
        .run(&AppContext)
    }
}

/// Drops every checkpoint. Tests that exercise the drive path without asserting on the
/// checkpoint sequence use it in place of the disk-backed sink.
#[cfg(test)]
pub struct NoopCheckpointer;

#[cfg(test)]
impl Checkpointer for NoopCheckpointer {
    fn checkpoint(&self, _snapshot: &RunSnapshot) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::run_storage::LoadRunSnapshot;
    use crate::identifier::PipelineId;
    use crate::models::{Aspect, AspectType};
    use crate::run_id::RunId;
    use crate::run_snapshot::{CURRENT_SCHEMA_VERSION, RunStatus};
    use crate::state::PipelineState;
    use std::num::NonZeroUsize;

    #[test]
    fn disk_checkpointer_persists_a_loadable_snapshot() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let state_dir = temp
            .path()
            .canonicalize()
            .expect("temp path should canonicalize");

        let pipeline_id = PipelineId::new("test-pipeline").expect("pipeline id should be valid");
        let project_root = PathBuf::from("/project");
        let config_path = PathBuf::from("/project/piper.toml");
        let run_id = RunId::generate();
        let snapshot = RunSnapshot {
            schema_version: CURRENT_SCHEMA_VERSION,
            run_id,
            project_root: project_root.clone(),
            config_path: config_path.clone(),
            pipeline_path: PathBuf::from("/project/pipeline.toml"),
            prompt_path: PathBuf::from("/project/prompts"),
            state: PipelineState::new(
                pipeline_id.clone(),
                "Request".to_string(),
                vec![Aspect {
                    id: "a1".to_string(),
                    name: "Aspect 1".to_string(),
                    aspect_type: AspectType::Worker,
                    prompt: "Prompt".to_string(),
                    model: None,
                }],
                NonZeroUsize::new(4).expect("turn limit should be non-zero"),
                1,
            ),
            turns: Vec::new(),
            status: RunStatus::Running,
            terminal: None,
        };

        DiskCheckpointer {
            project_state_dir: state_dir.clone(),
        }
        .checkpoint(&snapshot)
        .expect("disk checkpoint should persist");

        let loaded = LoadRunSnapshot {
            project_state_dir: state_dir,
            pipeline_id,
            run_id,
            project_root,
            config_path,
        }
        .run(&AppContext)
        .expect("persisted snapshot should load");
        assert_eq!(loaded.run_id, run_id);
    }
}
