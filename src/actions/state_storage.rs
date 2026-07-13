use super::AppContext;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const PROJECT_SCHEMA_VERSION: u32 = 1;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectMetadata {
    schema_version: u32,
    canonical_root: PathBuf,
    configuration_path: PathBuf,
}

pub struct EnsureProject {
    pub directory: PathBuf,
    pub canonical_root: PathBuf,
    pub configuration_path: PathBuf,
}

impl EnsureProject {
    pub fn run(&self, _ctx: &AppContext) -> Result<()> {
        create_state_directories(&self.directory)?;
        let path = self.directory.join("project.json");
        let expected = ProjectMetadata {
            schema_version: PROJECT_SCHEMA_VERSION,
            canonical_root: self.canonical_root.clone(),
            configuration_path: self.configuration_path.clone(),
        };

        match fs::read(&path) {
            Ok(content) => validate_project_metadata(&path, &content, &expected),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let content = serde_json::to_vec_pretty(&expected)?;
                if atomic_create(&path, &content)? {
                    Ok(())
                } else {
                    let content = fs::read(&path)
                        .with_context(|| format!("Could not read concurrently created {path:?}"))?;
                    validate_project_metadata(&path, &content, &expected)
                }
            }
            Err(error) => Err(error).with_context(|| format!("Could not read {path:?}")),
        }
    }
}

fn validate_project_metadata(
    path: &Path,
    content: &[u8],
    expected: &ProjectMetadata,
) -> Result<()> {
    let stored: ProjectMetadata = serde_json::from_slice(content)
        .with_context(|| format!("Failed to parse project metadata {path:?}"))?;
    anyhow::ensure!(
        stored.schema_version == PROJECT_SCHEMA_VERSION,
        "Unsupported project metadata schema version {}",
        stored.schema_version
    );
    anyhow::ensure!(
        &stored == expected,
        "Stored project identity does not match the selected project"
    );
    Ok(())
}

fn atomic_create(path: &Path, content: &[u8]) -> Result<bool> {
    let parent = path
        .parent()
        .with_context(|| format!("State path {path:?} has no parent directory"))?;
    let temporary_path = write_temporary(path, content)?;
    match fs::hard_link(&temporary_path, path) {
        Ok(()) => {
            fs::remove_file(&temporary_path).with_context(|| {
                format!("Could not remove temporary state file {temporary_path:?}")
            })?;
            sync_directory(parent)?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(temporary_path);
            Ok(false)
        }
        Err(error) => {
            let _ = fs::remove_file(temporary_path);
            Err(error).with_context(|| format!("Could not create state file {path:?}"))
        }
    }
}

fn write_temporary(path: &Path, content: &[u8]) -> Result<PathBuf> {
    let parent = path
        .parent()
        .with_context(|| format!("State path {path:?} has no parent directory"))?;
    let file_name = path
        .file_name()
        .with_context(|| format!("State path {path:?} has no file name"))?;
    let mut temporary = None;
    for _ in 0..100 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            sequence
        ));
        match open_private_file(&candidate) {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Could not create temporary state file {candidate:?}")
                });
            }
        }
    }

    let (temporary_path, mut file) = temporary.context("Could not allocate a state temp file")?;
    let result = (|| -> Result<()> {
        file.write_all(content)
            .with_context(|| format!("Could not write temporary state file {temporary_path:?}"))?;
        file.sync_all()
            .with_context(|| format!("Could not sync temporary state file {temporary_path:?}"))?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    Ok(temporary_path)
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .with_context(|| format!("Could not open state directory {path:?}"))?
        .sync_all()
        .with_context(|| format!("Could not sync state directory {path:?}"))
}

fn create_state_directories(project_directory: &Path) -> Result<()> {
    fs::create_dir_all(project_directory)
        .with_context(|| format!("Could not create state directory {project_directory:?}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let projects = project_directory
            .parent()
            .context("Project state directory must be below projects")?;
        let piper = projects
            .parent()
            .context("Projects state directory must be below piper")?;
        for directory in [piper, projects, project_directory] {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
                .with_context(|| format!("Could not secure state directory {directory:?}"))?;
        }
    }

    Ok(())
}

fn open_private_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn action(base: &Path) -> EnsureProject {
        EnsureProject {
            directory: base.join("piper/projects/hash"),
            canonical_root: base.join("project"),
            configuration_path: base.join("project/piper.toml"),
        }
    }

    #[test]
    fn creates_and_validates_project_metadata() {
        let temp = tempdir().expect("temp directory should be created");
        let action = action(temp.path());

        action.run(&AppContext).expect("metadata should be created");
        action
            .run(&AppContext)
            .expect("matching metadata should validate");

        let content =
            fs::read(action.directory.join("project.json")).expect("metadata should be readable");
        let metadata: ProjectMetadata =
            serde_json::from_slice(&content).expect("metadata should be valid");
        assert_eq!(metadata.schema_version, PROJECT_SCHEMA_VERSION);
        assert_eq!(metadata.canonical_root, action.canonical_root);
        assert_eq!(metadata.configuration_path, action.configuration_path);
    }

    #[test]
    fn rejects_mismatched_identity_without_rewriting_it() {
        let temp = tempdir().expect("temp directory should be created");
        let action = action(temp.path());
        action.run(&AppContext).expect("metadata should be created");
        let path = action.directory.join("project.json");
        let before = fs::read(&path).expect("metadata should be readable");
        let mismatched = EnsureProject {
            canonical_root: temp.path().join("other"),
            ..action
        };

        let error = mismatched
            .run(&AppContext)
            .expect_err("identity mismatch should fail");

        assert!(error.to_string().contains("identity"));
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn rejects_corrupt_metadata_without_rewriting_it() {
        let temp = tempdir().expect("temp directory should be created");
        let action = action(temp.path());
        create_state_directories(&action.directory).expect("directories should be created");
        let path = action.directory.join("project.json");
        fs::write(&path, b"not json").expect("corrupt metadata should be written");

        let error = action
            .run(&AppContext)
            .expect_err("corrupt metadata should fail");

        assert!(error.to_string().contains("parse project metadata"));
        assert_eq!(fs::read(path).unwrap(), b"not json");
    }

    #[test]
    fn rejects_unknown_project_schema_without_rewriting_it() {
        let temp = tempdir().expect("temp directory should be created");
        let action = action(temp.path());
        create_state_directories(&action.directory).expect("directories should be created");
        let path = action.directory.join("project.json");
        let content = serde_json::to_vec(&ProjectMetadata {
            schema_version: 2,
            canonical_root: action.canonical_root.clone(),
            configuration_path: action.configuration_path.clone(),
        })
        .expect("future metadata should serialize");
        fs::write(&path, &content).expect("future metadata should be written");

        let error = action
            .run(&AppContext)
            .expect_err("unknown schema should fail");

        assert!(error.to_string().contains("schema version 2"));
        assert_eq!(fs::read(path).unwrap(), content);
    }

    #[test]
    fn atomic_create_never_replaces_an_existing_file() {
        let temp = tempdir().expect("temp directory should be created");
        let path = temp.path().join("state.json");

        assert!(atomic_create(&path, b"first").expect("first create should succeed"));
        assert!(!atomic_create(&path, b"second").expect("second create should be observed"));

        assert_eq!(fs::read(&path).unwrap(), b"first");
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn creates_private_directories_and_files() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().expect("temp directory should be created");
        let action = action(temp.path());
        action.run(&AppContext).expect("metadata should be created");

        for directory in [
            temp.path().join("piper"),
            temp.path().join("piper/projects"),
            action.directory.clone(),
        ] {
            assert_eq!(
                fs::metadata(directory).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        assert_eq!(
            fs::metadata(action.directory.join("project.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
