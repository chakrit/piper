use std::fs;
use std::path::Path;
use std::process::Command;

fn write_test_llm(dir: &Path) -> std::path::PathBuf {
    let script = dir.join("piper-test-llm");
    fs::write(
        &script,
        r#"#!/usr/bin/env bash
set -euo pipefail
prompt="${1:-}"

if [[ "$prompt" == *"Fork Directive**: child branch"* ]]; then
  printf 'DONE Child complete\n'
elif [[ "$prompt" == *"Current Aspect**: Collect Knowledge"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE Alpha requirement"* ]]; then
    printf 'CONTINUE\n'
  else
    printf 'SET Alpha requirement\n'
  fi
elif [[ "$prompt" == *"Current Aspect**: Branch Work"* ]]; then
  if [[ "$prompt" == *"FORK f1 child branch"* ]]; then
    printf 'CONTINUE\n'
  else
    printf 'FORK f1 child branch\n'
  fi
elif [[ "$prompt" == *"Current Aspect**: Finish"* ]]; then
  printf 'DONE Black-box success\n'
else
  printf 'STOP unexpected prompt\n'
fi
"#,
    )
    .expect("test LLM script should be written");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script)
            .expect("test LLM script metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("test LLM script should be executable");
    }

    script
}

#[test]
fn cli_runs_pipeline_incrementally() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let llm = write_test_llm(temp_dir.path());
    let output = Command::new(env!("CARGO_BIN_EXE_piper"))
        .args([
            "--file",
            "tests/pipelines/black-box.toml",
            "run",
            "black-box",
            "--goal",
            "Exercise the CLI",
            "--request",
            "Run the black-box pipeline",
        ])
        .env(
            "PIPER_TEST_LLM_INVOCATION",
            format!("{} {{{{prompt}}}}", llm.display()),
        )
        .env("PIPER_DEFAULT_MODEL", "cerebras/gemma-4-31b")
        .output()
        .expect("piper CLI should run");

    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Processing: Collect Knowledge"));
    assert!(stdout.contains("Processing: Branch Work"));
    assert!(stdout.contains("[Fork: f1]"));
    assert!(stdout.contains("Black-box success"));
}
