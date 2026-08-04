use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// A durable-lifecycle sandbox: an isolated project, an isolated XDG state home, and a
/// deterministic shell Worker that logs every call so tests can prove the model was not
/// re-invoked for an already-applied turn.
struct Sandbox {
    _home: tempfile::TempDir,
    home: PathBuf,
    project: PathBuf,
    state: PathBuf,
    llm_log: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let home = tempfile::tempdir().expect("temp home should be created");
        let root = home.path().to_path_buf();
        let project = root.join("project");
        fs::create_dir(&project).expect("project directory should be created");

        let llm = write_worker(&root);
        fs::write(
            project.join("piper.toml"),
            format!(
                "[llm]\ninvocation = \"{} {{{{prompt}}}}\"\ndefault_model = \"sonnet\"\n",
                llm.display()
            ),
        )
        .expect("project config should be written");

        Self {
            home: root.clone(),
            project,
            state: root.join("state"),
            llm_log: root.join("llm.log"),
            _home: home,
        }
    }

    fn piper(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_piper"));
        command
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", "/usr/bin:/bin")
            .env("TERM", "dumb")
            .env("XDG_STATE_HOME", &self.state)
            .env("PIPER_TEST_LLM_LOG", &self.llm_log)
            .current_dir(&self.project)
            .stdin(Stdio::null());
        command
    }

    fn pipeline(&self, name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/pipelines")
            .join(name)
    }

    fn runs_dir(&self, pipeline: &str) -> PathBuf {
        let projects = self.state.join("piper/projects");
        let hash_dir = fs::read_dir(&projects)
            .expect("projects directory should exist after a run")
            .next()
            .expect("one project hash directory should exist")
            .expect("project hash entry should read")
            .path();
        hash_dir.join("pipelines").join(pipeline).join("runs")
    }

    fn only_run_id(&self, pipeline: &str) -> String {
        let runs = self.runs_dir(pipeline);
        let entry = fs::read_dir(&runs)
            .expect("runs directory should exist")
            .filter_map(Result::ok)
            .find(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("json"))
            .expect("one run snapshot should exist");
        entry
            .path()
            .file_stem()
            .expect("snapshot has a file stem")
            .to_string_lossy()
            .into_owned()
    }

    fn snapshot(&self, pipeline: &str, run_id: &str) -> serde_json::Value {
        let path = self.runs_dir(pipeline).join(format!("{run_id}.json"));
        serde_json::from_slice(&fs::read(&path).expect("snapshot should be readable"))
            .expect("snapshot should be valid JSON")
    }

    fn active(&self, pipeline: &str) -> String {
        let path = self.runs_dir(pipeline).parent().unwrap().join("active");
        fs::read_to_string(path)
            .expect("active pointer should be readable")
            .trim()
            .to_string()
    }
}

fn write_worker(dir: &Path) -> PathBuf {
    let script = dir.join("piper-lifecycle-llm");
    fs::write(
        &script,
        r#"#!/usr/bin/env bash
set -euo pipefail
prompt="${1:-}"
if [[ -n "${PIPER_TEST_LLM_LOG:-}" ]]; then
  printf '%s\n===CALL===\n' "$prompt" >> "$PIPER_TEST_LLM_LOG"
fi
if [[ "$prompt" == *"Current Aspect**: Prepare Release"* ]]; then
  printf 'CONTINUE\n'
elif [[ "$prompt" == *"Current Aspect**: Midpoint"* ]]; then
  printf 'CONTINUE\n'
elif [[ "$prompt" == *"Current Aspect**: Ship Release"* ]]; then
  printf 'DONE shipped\n'
else
  printf 'STOP unexpected prompt\n'
fi
"#,
    )
    .expect("worker script should be written");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script)
            .expect("worker metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("worker should be executable");
    }

    script
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected success\nstderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn start_reaches_gate_and_writes_inspectable_active_run() {
    let env = Sandbox::new();

    let output = env
        .piper()
        .args(["--file", "piper.toml", "start"])
        .arg(env.pipeline("gated.toml"))
        .arg("Ship the release")
        .output()
        .expect("piper start should run");
    assert_success(&output);

    let run_id = env.only_run_id("gated");
    assert_eq!(env.snapshot("gated", &run_id)["status"], "waiting_at_gate");
    assert_eq!(env.active("gated"), run_id);
}

#[test]
fn continue_approves_the_gate_and_reaches_done() {
    let env = Sandbox::new();
    let start = env
        .piper()
        .args(["--file", "piper.toml", "start"])
        .arg(env.pipeline("gated.toml"))
        .arg("Ship the release")
        .output()
        .expect("piper start should run");
    assert_success(&start);
    let run_id = env.only_run_id("gated");

    let output = env
        .piper()
        .args(["--file", "piper.toml", "continue", "gated"])
        .output()
        .expect("piper continue should run");
    assert_success(&output);

    assert_eq!(env.snapshot("gated", &run_id)["status"], "done");
    assert!(stdout_of(&output).contains("shipped"));
}

#[test]
fn two_gates_require_two_continues() {
    let env = Sandbox::new();
    let start = env
        .piper()
        .args(["--file", "piper.toml", "start"])
        .arg(env.pipeline("double-gate.toml"))
        .arg("Ship through two gates")
        .output()
        .expect("piper start should run");
    assert_success(&start);
    let run_id = env.only_run_id("double-gate");

    let first = env
        .piper()
        .args(["--file", "piper.toml", "continue", "double-gate"])
        .output()
        .expect("first continue should run");
    assert_success(&first);
    assert_eq!(
        env.snapshot("double-gate", &run_id)["status"],
        "waiting_at_gate",
        "one continue must stop at the second gate, not finish the run"
    );

    let second = env
        .piper()
        .args(["--file", "piper.toml", "continue", "double-gate"])
        .output()
        .expect("second continue should run");
    assert_success(&second);
    assert_eq!(env.snapshot("double-gate", &run_id)["status"], "done");
}

#[test]
fn start_refuses_to_replace_a_non_terminal_active_run() {
    let env = Sandbox::new();
    let first = env
        .piper()
        .args(["--file", "piper.toml", "start"])
        .arg(env.pipeline("gated.toml"))
        .arg("First release")
        .output()
        .expect("first start should run");
    assert_success(&first);
    let run_id = env.only_run_id("gated");

    let second = env
        .piper()
        .args(["--file", "piper.toml", "start"])
        .arg(env.pipeline("gated.toml"))
        .arg("Second release")
        .output()
        .expect("second start should run");

    assert!(
        !second.status.success(),
        "starting over a non-terminal active run must be refused"
    );
    assert_eq!(
        env.only_run_id("gated"),
        run_id,
        "the refused start must not mint a second run"
    );
    assert_eq!(env.snapshot("gated", &run_id)["status"], "waiting_at_gate");
}

#[test]
fn status_reports_state_without_mutating_it() {
    let env = Sandbox::new();
    let start = env
        .piper()
        .args(["--file", "piper.toml", "start"])
        .arg(env.pipeline("gated.toml"))
        .arg("Inspect me")
        .output()
        .expect("piper start should run");
    assert_success(&start);
    let run_id = env.only_run_id("gated");

    let path = env.runs_dir("gated").join(format!("{run_id}.json"));
    let before = fs::read(&path).expect("snapshot should read before status");

    let output = env
        .piper()
        .args(["--file", "piper.toml", "status", "gated"])
        .output()
        .expect("piper status should run");
    assert_success(&output);
    assert!(stdout_of(&output).contains("waiting_at_gate"));

    let after = fs::read(&path).expect("snapshot should read after status");
    assert_eq!(before, after, "status must not rewrite the snapshot");
}

#[test]
fn abort_makes_the_active_run_terminal() {
    let env = Sandbox::new();
    let start = env
        .piper()
        .args(["--file", "piper.toml", "start"])
        .arg(env.pipeline("gated.toml"))
        .arg("Abort me")
        .output()
        .expect("piper start should run");
    assert_success(&start);
    let run_id = env.only_run_id("gated");

    let output = env
        .piper()
        .args(["--file", "piper.toml", "abort", "gated"])
        .output()
        .expect("piper abort should run");
    assert_success(&output);

    assert_eq!(env.snapshot("gated", &run_id)["status"], "aborted");
}

#[test]
fn run_selection_accepts_a_unique_ulid_prefix() {
    let env = Sandbox::new();
    let start = env
        .piper()
        .args(["--file", "piper.toml", "start"])
        .arg(env.pipeline("gated.toml"))
        .arg("Select by prefix")
        .output()
        .expect("piper start should run");
    assert_success(&start);
    let run_id = env.only_run_id("gated");
    let prefix = &run_id[..8];

    let output = env
        .piper()
        .args(["--file", "piper.toml", "continue", "--run", prefix])
        .output()
        .expect("piper continue --run should run");
    assert_success(&output);

    assert_eq!(env.snapshot("gated", &run_id)["status"], "done");
}

#[test]
fn resuming_does_not_reinvoke_the_model_for_an_applied_turn() {
    let env = Sandbox::new();
    let start = env
        .piper()
        .args(["--file", "piper.toml", "start"])
        .arg(env.pipeline("gated.toml"))
        .arg("Do not repeat the prepare turn")
        .output()
        .expect("piper start should run");
    assert_success(&start);

    let after_start = fs::read_to_string(&env.llm_log).expect("llm log should exist after start");
    assert_eq!(
        after_start
            .matches("Current Aspect**: Prepare Release")
            .count(),
        1,
        "start must invoke the prepare worker exactly once"
    );

    let cont = env
        .piper()
        .args(["--file", "piper.toml", "continue", "gated"])
        .output()
        .expect("piper continue should run");
    assert_success(&cont);

    let after_continue = fs::read_to_string(&env.llm_log).expect("llm log should exist");
    assert_eq!(
        after_continue
            .matches("Current Aspect**: Prepare Release")
            .count(),
        1,
        "continue must not re-invoke the already-applied prepare turn"
    );
    assert_eq!(
        after_continue
            .matches("Current Aspect**: Ship Release")
            .count(),
        1,
        "continue must invoke the ship worker exactly once"
    );
}

#[test]
fn continue_without_a_selection_is_an_error_when_non_interactive() {
    let env = Sandbox::new();
    let start = env
        .piper()
        .args(["--file", "piper.toml", "start"])
        .arg(env.pipeline("gated.toml"))
        .arg("Need a selection")
        .output()
        .expect("piper start should run");
    assert_success(&start);

    let output = env
        .piper()
        .args(["--file", "piper.toml", "continue"])
        .output()
        .expect("piper continue should run");

    assert!(
        !output.status.success(),
        "a non-interactive continue with no selection must error, not hang or guess"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("pipeline id") || stderr.contains("--run"),
        "the error should tell the operator how to select a run: {stderr}"
    );
}
