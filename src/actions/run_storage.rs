use super::AppContext;
use super::state_storage::{atomic_replace, ensure_private_directory};
use crate::identifier::PipelineId;
use crate::run_id::RunId;
use crate::run_snapshot::{CURRENT_SCHEMA_VERSION, RunSnapshot};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

fn pipeline_dir(state_dir: &Path, pipeline_id: &PipelineId) -> PathBuf {
    state_dir.join("pipelines").join(pipeline_id.as_str())
}

fn runs_dir(state_dir: &Path, pipeline_id: &PipelineId) -> PathBuf {
    pipeline_dir(state_dir, pipeline_id).join("runs")
}

fn active_path(state_dir: &Path, pipeline_id: &PipelineId) -> PathBuf {
    pipeline_dir(state_dir, pipeline_id).join("active")
}

fn snapshot_path(state_dir: &Path, pipeline_id: &PipelineId, run_id: &RunId) -> PathBuf {
    runs_dir(state_dir, pipeline_id).join(format!("{run_id}.json"))
}

/// Create `pipelines/<pipeline-id>/runs/` under the project state directory, securing each
/// created level to mode `0700`. The single owner of the per-pipeline directory tree; the
/// snapshot and pointer writers call it before their own write.
pub struct EnsurePipelineLayout {
    pub project_state_dir: PathBuf,
    pub pipeline_id: PipelineId,
}

impl EnsurePipelineLayout {
    pub fn run(&self, _ctx: &AppContext) -> Result<()> {
        let pipelines = self.project_state_dir.join("pipelines");
        let pipeline = pipelines.join(self.pipeline_id.as_str());
        let runs = pipeline.join("runs");
        for directory in [&pipelines, &pipeline, &runs] {
            ensure_private_directory(directory)?;
        }
        Ok(())
    }
}

/// Persist a run snapshot to `pipelines/<id>/runs/<run-ulid>.json` as an atomic
/// replacement. Owns the snapshot file write.
pub struct SaveRunSnapshot<'a> {
    pub project_state_dir: PathBuf,
    pub snapshot: &'a RunSnapshot,
}

impl SaveRunSnapshot<'_> {
    pub fn run(&self, ctx: &AppContext) -> Result<()> {
        let pipeline_id = self.snapshot.pipeline_id().clone();
        EnsurePipelineLayout {
            project_state_dir: self.project_state_dir.clone(),
            pipeline_id: pipeline_id.clone(),
        }
        .run(ctx)?;

        let path = snapshot_path(&self.project_state_dir, &pipeline_id, &self.snapshot.run_id);
        let content =
            serde_json::to_vec_pretty(self.snapshot).context("Could not serialize run snapshot")?;
        atomic_replace(&path, &content)
    }
}

/// Point a pipeline's `active` file at one full run ULID as an atomic replacement. Owns the
/// active-pointer write.
pub struct WriteActivePointer {
    pub project_state_dir: PathBuf,
    pub pipeline_id: PipelineId,
    pub run_id: RunId,
}

impl WriteActivePointer {
    pub fn run(&self, ctx: &AppContext) -> Result<()> {
        EnsurePipelineLayout {
            project_state_dir: self.project_state_dir.clone(),
            pipeline_id: self.pipeline_id.clone(),
        }
        .run(ctx)?;

        let path = active_path(&self.project_state_dir, &self.pipeline_id);
        atomic_replace(&path, self.run_id.to_string().as_bytes())
    }
}

/// Why reading a pipeline's `active` pointer did not yield a run identity. `Missing` is a
/// distinct outcome, not a failure: `start` treats it as "free to begin", while selection
/// by pipeline id treats it as "no run to act on". The two callers need that split, so it
/// lives in the type rather than in a stringly-typed error message.
#[derive(Debug, thiserror::Error)]
pub enum ActivePointerError {
    #[error("pipeline {pipeline:?} has no active run")]
    Missing { pipeline: String },
    #[error("could not read active pointer {path:?}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("active pointer {path:?} is not a canonical ULID: {message}")]
    Malformed { path: PathBuf, message: String },
}

/// Read a pipeline's `active` pointer and parse it back into a run identity.
pub struct ReadActivePointer {
    pub project_state_dir: PathBuf,
    pub pipeline_id: PipelineId,
}

impl ReadActivePointer {
    pub fn run(&self, _ctx: &AppContext) -> Result<RunId, ActivePointerError> {
        let path = active_path(&self.project_state_dir, &self.pipeline_id);
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(ActivePointerError::Missing {
                    pipeline: self.pipeline_id.as_str().to_string(),
                });
            }
            Err(source) => return Err(ActivePointerError::Read { path, source }),
        };
        RunId::parse(content.trim()).map_err(|error| ActivePointerError::Malformed {
            path,
            message: error.to_string(),
        })
    }
}

/// Enumerate every `(pipeline id, run ULID)` under a project's state directory by reading
/// the on-disk layout, without opening any snapshot. Directory names that are not valid
/// pipeline ids and files that are not `<canonical-ulid>.json` are ignored: the layout is a
/// trust boundary, so only well-formed identities are admitted. Owns the run enumeration.
pub struct ListProjectRuns {
    pub project_state_dir: PathBuf,
}

impl ListProjectRuns {
    pub fn run(&self, _ctx: &AppContext) -> Result<Vec<(PipelineId, RunId)>> {
        let pipelines_dir = self.project_state_dir.join("pipelines");
        let entries = match fs::read_dir(&pipelines_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Could not read pipelines directory {pipelines_dir:?}")
                });
            }
        };

        let mut found = Vec::new();
        for entry in entries {
            let entry = entry.context("Could not read a pipelines directory entry")?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Ok(pipeline_id) = PipelineId::new(name) else {
                continue;
            };
            Self::collect_runs(&entry.path().join("runs"), &pipeline_id, &mut found)?;
        }
        Ok(found)
    }

    fn collect_runs(
        runs_dir: &Path,
        pipeline_id: &PipelineId,
        found: &mut Vec<(PipelineId, RunId)>,
    ) -> Result<()> {
        let entries = match fs::read_dir(runs_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Could not read runs directory {runs_dir:?}"));
            }
        };

        for entry in entries {
            let path = entry
                .context("Could not read a runs directory entry")?
                .path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if let Ok(run_id) = RunId::parse(stem) {
                found.push((pipeline_id.clone(), run_id));
            }
        }
        Ok(())
    }
}

/// Every way loading a run snapshot can be rejected. Each variant is a fail-closed refusal:
/// the caller neither rewrites, recovers, nor partially trusts the stored state.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotLoadError {
    #[error("could not read run snapshot {path:?}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("run snapshot {path:?} is not valid JSON: {source}")]
    Malformed {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("run snapshot uses unsupported schema version {version}")]
    UnsupportedSchema { version: u32 },
    #[error("run snapshot pipeline id {id:?} is not a valid identifier")]
    InvalidPipelineId { id: String },
    #[error("run snapshot pipeline id {found:?} does not match requested {expected:?}")]
    PipelineIdMismatch { expected: String, found: String },
    #[error("run snapshot run id {found} does not match requested {expected}")]
    RunIdMismatch { expected: RunId, found: RunId },
    #[error("run snapshot project root {found:?} does not match current {expected:?}")]
    ProjectRootMismatch { expected: PathBuf, found: PathBuf },
    #[error("run snapshot config path {found:?} does not match current {expected:?}")]
    ConfigPathMismatch { expected: PathBuf, found: PathBuf },
    #[error("run snapshot is internally inconsistent: {message}")]
    Inconsistent { message: String },
}

/// Load and fail-closed-validate a run snapshot from `pipelines/<id>/runs/<run-ulid>.json`.
/// The decoded pipeline and run identities are re-checked against the directory they came
/// from rather than trusted, and the stored project/config paths must match the currently
/// resolved project.
pub struct LoadRunSnapshot {
    pub project_state_dir: PathBuf,
    pub pipeline_id: PipelineId,
    pub run_id: RunId,
    pub project_root: PathBuf,
    pub config_path: PathBuf,
}

impl LoadRunSnapshot {
    pub fn run(&self, _ctx: &AppContext) -> Result<RunSnapshot, SnapshotLoadError> {
        let path = snapshot_path(&self.project_state_dir, &self.pipeline_id, &self.run_id);
        let content = fs::read(&path).map_err(|source| SnapshotLoadError::Read {
            path: path.clone(),
            source,
        })?;
        let snapshot: RunSnapshot =
            serde_json::from_slice(&content).map_err(|source| SnapshotLoadError::Malformed {
                path: path.clone(),
                source,
            })?;

        self.verify(snapshot)
    }

    fn verify(&self, snapshot: RunSnapshot) -> Result<RunSnapshot, SnapshotLoadError> {
        if snapshot.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(SnapshotLoadError::UnsupportedSchema {
                version: snapshot.schema_version.0,
            });
        }

        let pipeline_id = snapshot.pipeline_id();
        crate::identifier::validate(pipeline_id.as_str(), "pipeline.id").map_err(|_| {
            SnapshotLoadError::InvalidPipelineId {
                id: pipeline_id.as_str().to_string(),
            }
        })?;
        if pipeline_id != &self.pipeline_id {
            return Err(SnapshotLoadError::PipelineIdMismatch {
                expected: self.pipeline_id.as_str().to_string(),
                found: pipeline_id.as_str().to_string(),
            });
        }
        if snapshot.run_id != self.run_id {
            return Err(SnapshotLoadError::RunIdMismatch {
                expected: self.run_id,
                found: snapshot.run_id,
            });
        }
        if snapshot.project_root != self.project_root {
            return Err(SnapshotLoadError::ProjectRootMismatch {
                expected: self.project_root.clone(),
                found: snapshot.project_root.clone(),
            });
        }
        if snapshot.config_path != self.config_path {
            return Err(SnapshotLoadError::ConfigPathMismatch {
                expected: self.config_path.clone(),
                found: snapshot.config_path.clone(),
            });
        }

        snapshot
            .validate()
            .map_err(|error| SnapshotLoadError::Inconsistent {
                message: error.to_string(),
            })?;
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::AspectAction;
    use crate::models::{Aspect, AspectType};
    use crate::run_snapshot::{
        CURRENT_SCHEMA_VERSION, CallPhase, RunStatus, SchemaVersion, TerminalOutcome, TurnKind,
        TurnRecord,
    };
    use crate::state::PipelineState;
    use std::num::NonZeroUsize;
    use tempfile::tempdir;

    const PROJECT_ROOT: &str = "/project";
    const CONFIG_PATH: &str = "/project/piper.toml";

    fn pipeline_id() -> PipelineId {
        PipelineId::new("test-pipeline").expect("pipeline id should be valid")
    }

    fn nested_state() -> PipelineState {
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
            .expect("outer fork should start");
        state
            .push_fork("buyers".to_string(), "Study buyers".to_string())
            .expect("nested fork should start");
        state
    }

    fn snapshot(run_id: RunId) -> RunSnapshot {
        RunSnapshot {
            schema_version: CURRENT_SCHEMA_VERSION,
            run_id,
            project_root: PathBuf::from(PROJECT_ROOT),
            config_path: PathBuf::from(CONFIG_PATH),
            pipeline_path: PathBuf::from("/project/pipeline.toml"),
            prompt_path: PathBuf::from("/project/prompts"),
            state: nested_state(),
            turns: vec![
                TurnRecord {
                    kind: TurnKind::Worker,
                    prompt: "Worker prompt".to_string(),
                    phase: CallPhase::Applied {
                        raw_output: "SET Evidence".to_string(),
                        action: AspectAction::Set("Evidence".to_string()),
                    },
                },
                TurnRecord {
                    kind: TurnKind::FormatterRecovery,
                    prompt: "Recovery prompt".to_string(),
                    phase: CallPhase::Calling,
                },
            ],
            status: RunStatus::Running,
            terminal: None,
        }
    }

    fn state_dir() -> (tempfile::TempDir, PathBuf) {
        let temp = tempdir().expect("temp directory should be created");
        let dir = temp
            .path()
            .canonicalize()
            .expect("temp path should canonicalize");
        (temp, dir)
    }

    fn loader(state_dir: &Path, run_id: RunId) -> LoadRunSnapshot {
        LoadRunSnapshot {
            project_state_dir: state_dir.to_path_buf(),
            pipeline_id: pipeline_id(),
            run_id,
            project_root: PathBuf::from(PROJECT_ROOT),
            config_path: PathBuf::from(CONFIG_PATH),
        }
    }

    fn save(state_dir: &Path, snapshot: &RunSnapshot) {
        SaveRunSnapshot {
            project_state_dir: state_dir.to_path_buf(),
            snapshot,
        }
        .run(&AppContext)
        .expect("snapshot should save");
    }

    #[test]
    fn save_then_load_round_trips_nested_frames_and_turns() {
        let (_temp, dir) = state_dir();
        let run_id = RunId::generate();
        let original = snapshot(run_id);
        save(&dir, &original);

        let loaded = loader(&dir, run_id)
            .run(&AppContext)
            .expect("saved snapshot should load");

        assert_eq!(loaded.run_id, run_id);
        assert_eq!(loaded.turns, original.turns);
        assert_eq!(loaded.state.frames.len(), 3);
        assert_eq!(loaded.state.frame().fork_id(), Some("buyers"));
    }

    #[test]
    fn write_then_read_round_trips_active_pointer() {
        let (_temp, dir) = state_dir();
        let run_id = RunId::generate();
        WriteActivePointer {
            project_state_dir: dir.clone(),
            pipeline_id: pipeline_id(),
            run_id,
        }
        .run(&AppContext)
        .expect("active pointer should write");

        let read = ReadActivePointer {
            project_state_dir: dir.clone(),
            pipeline_id: pipeline_id(),
        }
        .run(&AppContext)
        .expect("active pointer should read");

        assert_eq!(read, run_id);
    }

    #[test]
    fn atomic_replace_overwrites_snapshot_and_pointer() {
        let (_temp, dir) = state_dir();
        let first = RunId::generate();
        let second = RunId::generate();

        save(&dir, &snapshot(first));
        let mut updated = snapshot(first);
        updated.status = RunStatus::Done;
        updated.terminal = Some(TerminalOutcome::Done {
            result: vec!["Final".to_string()],
        });
        save(&dir, &updated);

        let loaded = loader(&dir, first)
            .run(&AppContext)
            .expect("overwritten snapshot should load");
        assert_eq!(loaded.status, RunStatus::Done);

        WriteActivePointer {
            project_state_dir: dir.clone(),
            pipeline_id: pipeline_id(),
            run_id: first,
        }
        .run(&AppContext)
        .expect("first pointer should write");
        WriteActivePointer {
            project_state_dir: dir.clone(),
            pipeline_id: pipeline_id(),
            run_id: second,
        }
        .run(&AppContext)
        .expect("second pointer should overwrite");

        let read = ReadActivePointer {
            project_state_dir: dir.clone(),
            pipeline_id: pipeline_id(),
        }
        .run(&AppContext)
        .expect("pointer should read");
        assert_eq!(read, second);
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_files_are_private_and_directories_are_secured() {
        use std::os::unix::fs::PermissionsExt;

        let (_temp, dir) = state_dir();
        let run_id = RunId::generate();
        save(&dir, &snapshot(run_id));

        let mode = |path: &Path| {
            fs::metadata(path)
                .expect("path metadata should be readable")
                .permissions()
                .mode()
                & 0o777
        };

        assert_eq!(mode(&snapshot_path(&dir, &pipeline_id(), &run_id)), 0o600);
        for directory in [
            dir.join("pipelines"),
            pipeline_dir(&dir, &pipeline_id()),
            runs_dir(&dir, &pipeline_id()),
        ] {
            assert_eq!(mode(&directory), 0o700);
        }
    }

    fn write_raw(state_dir: &Path, run_id: &RunId, bytes: &[u8]) {
        EnsurePipelineLayout {
            project_state_dir: state_dir.to_path_buf(),
            pipeline_id: pipeline_id(),
        }
        .run(&AppContext)
        .expect("layout should exist");
        fs::write(snapshot_path(state_dir, &pipeline_id(), run_id), bytes)
            .expect("raw snapshot should be written");
    }

    #[test]
    fn load_rejects_malformed_json() {
        let (_temp, dir) = state_dir();
        let run_id = RunId::generate();
        write_raw(&dir, &run_id, b"not json");

        let error = loader(&dir, run_id)
            .run(&AppContext)
            .expect_err("malformed json must be rejected");
        assert!(matches!(error, SnapshotLoadError::Malformed { .. }));
    }

    #[test]
    fn load_rejects_truncated_json() {
        let (_temp, dir) = state_dir();
        let run_id = RunId::generate();
        let full = serde_json::to_vec(&snapshot(run_id)).expect("snapshot should serialize");
        write_raw(&dir, &run_id, &full[..full.len() / 2]);

        let error = loader(&dir, run_id)
            .run(&AppContext)
            .expect_err("truncated json must be rejected");
        assert!(matches!(error, SnapshotLoadError::Malformed { .. }));
    }

    #[test]
    fn load_rejects_unknown_schema_version() {
        let (_temp, dir) = state_dir();
        let run_id = RunId::generate();
        let mut future = snapshot(run_id);
        future.schema_version = SchemaVersion(CURRENT_SCHEMA_VERSION.0 + 1);
        write_raw(
            &dir,
            &run_id,
            &serde_json::to_vec(&future).expect("future snapshot should serialize"),
        );

        let error = loader(&dir, run_id)
            .run(&AppContext)
            .expect_err("unknown schema must be rejected");
        assert!(matches!(error, SnapshotLoadError::UnsupportedSchema { .. }));
    }

    #[test]
    fn load_rejects_run_id_that_disagrees_with_filename() {
        let (_temp, dir) = state_dir();
        let stored = RunId::generate();
        let filename = RunId::generate();
        write_raw(
            &dir,
            &filename,
            &serde_json::to_vec(&snapshot(stored)).expect("snapshot should serialize"),
        );

        let error = loader(&dir, filename)
            .run(&AppContext)
            .expect_err("run-id/filename mismatch must be rejected");
        assert!(matches!(error, SnapshotLoadError::RunIdMismatch { .. }));
    }

    #[test]
    fn load_rejects_pipeline_id_mismatch() {
        let (_temp, dir) = state_dir();
        let run_id = RunId::generate();
        let mut foreign = snapshot(run_id);
        foreign.state.pipeline_id =
            PipelineId::new("other-pipeline").expect("other id should be valid");
        write_raw(
            &dir,
            &run_id,
            &serde_json::to_vec(&foreign).expect("foreign snapshot should serialize"),
        );

        let error = loader(&dir, run_id)
            .run(&AppContext)
            .expect_err("pipeline-id mismatch must be rejected");
        assert!(matches!(
            error,
            SnapshotLoadError::PipelineIdMismatch { .. }
        ));
    }

    #[test]
    fn load_rejects_project_root_mismatch() {
        let (_temp, dir) = state_dir();
        let run_id = RunId::generate();
        save(&dir, &snapshot(run_id));

        let mut loader = loader(&dir, run_id);
        loader.project_root = PathBuf::from("/somewhere/else");

        let error = loader
            .run(&AppContext)
            .expect_err("project-root mismatch must be rejected");
        assert!(matches!(
            error,
            SnapshotLoadError::ProjectRootMismatch { .. }
        ));
    }

    #[test]
    fn load_rejects_config_path_mismatch() {
        let (_temp, dir) = state_dir();
        let run_id = RunId::generate();
        save(&dir, &snapshot(run_id));

        let mut loader = loader(&dir, run_id);
        loader.config_path = PathBuf::from("/somewhere/else.toml");

        let error = loader
            .run(&AppContext)
            .expect_err("config-path mismatch must be rejected");
        assert!(matches!(
            error,
            SnapshotLoadError::ConfigPathMismatch { .. }
        ));
    }
}
