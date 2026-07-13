use super::AppContext;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fmt::Write;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct ProjectPaths {
    pub root: PathBuf,
    pub config: PathBuf,
    pub state_dir: PathBuf,
}

pub struct ResolveProject {
    pub config: Option<PathBuf>,
    pub cwd: PathBuf,
}

impl ResolveProject {
    pub fn run(&self, _ctx: &AppContext) -> Result<ProjectPaths> {
        resolve_with(self.config.as_deref(), &self.cwd, |name| {
            std::env::var_os(name)
        })
    }
}

fn resolve_with(
    config: Option<&Path>,
    cwd: &Path,
    lookup: impl Fn(&str) -> Option<OsString>,
) -> Result<ProjectPaths> {
    let config = match config {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => cwd.join(path),
        None => cwd
            .ancestors()
            .map(|ancestor| ancestor.join("piper.toml"))
            .find(|candidate| candidate.is_file())
            .context("Could not find piper.toml in the current directory or its ancestors")?,
    };
    let config = config
        .canonicalize()
        .with_context(|| format!("Could not resolve configuration path {config:?}"))?;
    let root = config
        .parent()
        .expect("a canonical configuration path has a parent")
        .to_path_buf();
    let state_home = lookup("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(Ok)
        .unwrap_or_else(|| {
            let home = PathBuf::from(
                lookup("HOME").context("HOME is required when XDG_STATE_HOME is unavailable")?,
            );
            if !home.is_absolute() {
                anyhow::bail!("HOME must be an absolute path")
            }
            Ok(home.join(".local/state"))
        })?;
    let state_dir = state_home.join("piper/projects").join(project_hash(&root));

    Ok(ProjectPaths {
        root,
        config,
        state_dir,
    })
}

fn project_hash(root: &Path) -> String {
    let digest = Sha256::digest(root.as_os_str().as_encoded_bytes());
    digest.iter().fold(
        String::with_capacity(digest.len() * 2),
        |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a String cannot fail");
            output
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::tempdir;

    fn environment(values: &[(&str, &Path)]) -> HashMap<String, OsString> {
        values
            .iter()
            .map(|(name, value)| ((*name).to_string(), value.as_os_str().to_os_string()))
            .collect()
    }

    #[test]
    fn discovers_nearest_ancestor_config() {
        let temp = tempdir().expect("temp directory should be created");
        let project = temp.path().join("project");
        let nested = project.join("a/b");
        fs::create_dir_all(&nested).expect("nested directory should be created");
        fs::write(project.join("piper.toml"), "").expect("project config should be written");
        fs::write(temp.path().join("piper.toml"), "").expect("outer config should be written");
        let state = temp.path().join("state");
        let env = environment(&[("XDG_STATE_HOME", &state)]);

        let paths = resolve_with(None, &nested, |name| env.get(name).cloned())
            .expect("project should resolve");

        assert_eq!(
            paths.root,
            project
                .canonicalize()
                .expect("project path should canonicalize")
        );
        assert_eq!(
            paths.config,
            project
                .join("piper.toml")
                .canonicalize()
                .expect("config path should canonicalize")
        );
    }

    #[test]
    fn explicit_config_defines_project_root() {
        let temp = tempdir().expect("temp directory should be created");
        let project = temp.path().join("project");
        fs::create_dir_all(&project).expect("project directory should be created");
        let config = project.join("shared.toml");
        fs::write(&config, "").expect("config should be written");
        let state = temp.path().join("state");
        let env = environment(&[("XDG_STATE_HOME", &state)]);

        let paths = resolve_with(Some(&config), temp.path(), |name| env.get(name).cloned())
            .expect("project should resolve");

        assert_eq!(
            paths.root,
            project
                .canonicalize()
                .expect("project path should canonicalize")
        );
        assert_eq!(
            paths.config,
            config
                .canonicalize()
                .expect("config path should canonicalize")
        );
    }

    #[test]
    fn uses_absolute_xdg_state_home() {
        let temp = tempdir().expect("temp directory should be created");
        let config = temp.path().join("piper.toml");
        fs::write(&config, "").expect("config should be written");
        let state = temp.path().join("xdg-state");
        let env = environment(&[("XDG_STATE_HOME", &state)]);

        let paths = resolve_with(Some(&config), temp.path(), |name| env.get(name).cloned())
            .expect("project should resolve");

        assert!(paths.state_dir.starts_with(state.join("piper/projects")));
    }

    #[test]
    fn relative_or_empty_xdg_state_home_uses_home_fallback() {
        let temp = tempdir().expect("temp directory should be created");
        let config = temp.path().join("piper.toml");
        fs::write(&config, "").expect("config should be written");
        let home = temp.path().join("home");

        for xdg in [Path::new("relative"), Path::new("")] {
            let env = environment(&[("XDG_STATE_HOME", xdg), ("HOME", &home)]);
            let paths = resolve_with(Some(&config), temp.path(), |name| env.get(name).cloned())
                .expect("project should resolve");

            assert!(
                paths
                    .state_dir
                    .starts_with(home.join(".local/state/piper/projects"))
            );
        }
    }

    #[test]
    fn missing_home_prevents_xdg_fallback() {
        let temp = tempdir().expect("temp directory should be created");
        let config = temp.path().join("piper.toml");
        fs::write(&config, "").expect("config should be written");

        let error = resolve_with(Some(&config), temp.path(), |_| None)
            .expect_err("missing HOME should fail");

        assert!(error.to_string().contains("HOME"));
    }

    #[test]
    fn hashes_canonical_project_root_with_sha256() {
        assert_eq!(
            project_hash(Path::new("/tmp/piper-project")),
            "c39cde9a745c4868a855fa77ec420fd4a5ed3f0c146b51a70f706ee25696cc8b"
        );
    }
}
