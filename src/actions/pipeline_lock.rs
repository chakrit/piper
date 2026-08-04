use super::AppContext;
use super::run_storage::EnsurePipelineLayout;
use super::state_storage::open_or_create_private_file;
use crate::identifier::PipelineId;
use anyhow::{Result, bail};
use std::fs::{File, TryLockError};
use std::path::PathBuf;

/// A held exclusive operating-system advisory lock on a pipeline's `operation.lock`.
/// Releasing is automatic: dropping the guard closes the file, and the OS drops the lock
/// with it. A mutating command keeps this guard alive across its whole operation so no
/// second mutation on the same pipeline can proceed concurrently.
pub struct PipelineLock {
    // Held only for its RAII drop: closing this handle releases the OS lock. It is never
    // read, so the dead-code lint is silenced on this field alone rather than the struct.
    #[allow(dead_code)]
    file: File,
}

/// Acquire a pipeline's exclusive `operation.lock` without waiting. Ensures the pipeline
/// directory exists, opens-or-creates the lock file at mode `0600`, and takes a
/// non-blocking exclusive lock. When the lock is already held by any owner it fails
/// immediately rather than blocking, so a concurrent mutation is refused at once.
pub struct AcquirePipelineLock {
    pub project_state_dir: PathBuf,
    pub pipeline_id: PipelineId,
}

impl AcquirePipelineLock {
    pub fn run(&self, ctx: &AppContext) -> Result<PipelineLock> {
        EnsurePipelineLayout {
            project_state_dir: self.project_state_dir.clone(),
            pipeline_id: self.pipeline_id.clone(),
        }
        .run(ctx)?;

        let path = self
            .project_state_dir
            .join("pipelines")
            .join(self.pipeline_id.as_str())
            .join("operation.lock");
        let file = open_or_create_private_file(&path)?;

        match file.try_lock() {
            Ok(()) => Ok(PipelineLock { file }),
            Err(TryLockError::WouldBlock) => bail!(
                "Pipeline {:?} is already being mutated by another operation",
                self.pipeline_id.as_str()
            ),
            Err(TryLockError::Error(error)) => {
                Err(error).map_err(|source| anyhow::anyhow!("Could not lock {path:?}: {source}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;

    fn pipeline_id(value: &str) -> PipelineId {
        PipelineId::new(value).expect("pipeline id should be valid")
    }

    fn state_dir() -> (tempfile::TempDir, PathBuf) {
        let temp = tempdir().expect("temp directory should be created");
        let dir = temp
            .path()
            .canonicalize()
            .expect("temp path should canonicalize");
        (temp, dir)
    }

    fn lock_path(state_dir: &Path, id: &PipelineId) -> PathBuf {
        state_dir
            .join("pipelines")
            .join(id.as_str())
            .join("operation.lock")
    }

    fn acquire(state_dir: &Path, id: &PipelineId) -> Result<PipelineLock> {
        AcquirePipelineLock {
            project_state_dir: state_dir.to_path_buf(),
            pipeline_id: id.clone(),
        }
        .run(&AppContext)
    }

    #[test]
    fn acquiring_creates_the_lock_file() {
        let (_temp, dir) = state_dir();
        let id = pipeline_id("build");

        let _guard = acquire(&dir, &id).expect("first acquisition should succeed");

        assert!(
            lock_path(&dir, &id).exists(),
            "operation.lock should be created"
        );
    }

    #[test]
    fn second_acquisition_fails_immediately_while_first_is_held() {
        let (_temp, dir) = state_dir();
        let id = pipeline_id("build");

        let held = acquire(&dir, &id).expect("first acquisition should succeed");
        let contended = acquire(&dir, &id);

        assert!(
            contended.is_err(),
            "a second acquisition must be refused while the first guard is alive"
        );
        drop(held);
    }

    #[test]
    fn acquisition_succeeds_after_the_guard_is_dropped() {
        let (_temp, dir) = state_dir();
        let id = pipeline_id("build");

        let first = acquire(&dir, &id).expect("first acquisition should succeed");
        drop(first);

        acquire(&dir, &id).expect("acquisition should succeed after release");
    }

    #[test]
    fn distinct_pipeline_ids_do_not_contend() {
        let (_temp, dir) = state_dir();
        let build = pipeline_id("build");
        let deploy = pipeline_id("deploy");

        let _build = acquire(&dir, &build).expect("build lock should succeed");
        let _deploy =
            acquire(&dir, &deploy).expect("a different pipeline's lock should not contend");
    }

    #[cfg(unix)]
    #[test]
    fn lock_file_is_private() {
        use std::os::unix::fs::PermissionsExt;

        let (_temp, dir) = state_dir();
        let id = pipeline_id("build");
        let _guard = acquire(&dir, &id).expect("acquisition should succeed");

        let mode = std::fs::metadata(lock_path(&dir, &id))
            .expect("lock metadata should be readable")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}
