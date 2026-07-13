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

if [[ "$prompt" == *"Bad Response:"* && "$prompt" == *"bad-first"* ]]; then
  printf 'CONTINUE\n'
elif [[ "$prompt" == *"Bad Response:"* && "$prompt" == *"bad-second"* ]]; then
  printf 'DONE recovery budget was not enforced\n'
elif [[ "$prompt" == *"Current Aspect**: First Recovery"* ]]; then
  printf 'bad-first\n'
elif [[ "$prompt" == *"Current Aspect**: Second Recovery"* ]]; then
  printf 'bad-second\n'
elif [[ "$prompt" == *"Current Aspect**: Seed Knowledge"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE TOKEN quartz-17"* ]]; then
    printf 'CONTINUE\n'
  else
    printf 'SET TOKEN quartz-17\n'
  fi
elif [[ "$prompt" == *"Current Aspect**: Relay Knowledge"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE TOKEN quartz-17"* ]]; then
    printf 'DONE RELAY quartz-17\n'
  else
    printf 'STOP token missing from Corpus\n'
  fi
elif [[ "$prompt" == *"Current Aspect**: Replace Knowledge"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE VALUE 17"* ]]; then
    printf 'CONTINUE\n'
  elif [[ "$prompt" == *"KNOWLEDGE VALUE wrong"* ]]; then
    printf 'SET VALUE 17\n'
  else
    printf 'SET VALUE wrong\n'
  fi
elif [[ "$prompt" == *"Current Aspect**: Verify Replacement"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE VALUE 17"* && "$prompt" != *"KNOWLEDGE VALUE wrong"* ]]; then
    printf 'DONE VALUE 17\n'
  else
    printf 'STOP replacement was not preserved\n'
  fi
elif [[ "$prompt" == *"Current Aspect**: Parallel Arithmetic"* && "$prompt" == *"Fork Directive**: calculate seven"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE TERM_A 7"* ]]; then printf 'JOIN\n'; else printf 'SET TERM_A 7\n'; fi
elif [[ "$prompt" == *"Current Aspect**: Parallel Arithmetic"* && "$prompt" == *"Fork Directive**: calculate eleven"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE TERM_A 7"* ]]; then printf 'STOP sibling term leaked\n'; elif [[ "$prompt" == *"KNOWLEDGE TERM_B 11"* ]]; then printf 'CONTINUE\n'; else printf 'SET TERM_B 11\n'; fi
elif [[ "$prompt" == *"Current Aspect**: Parallel Arithmetic"* && "$prompt" == *"Fork Directive**: calculate twenty-four"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE TERM_A 7"* || "$prompt" == *"KNOWLEDGE TERM_B 11"* ]]; then printf 'STOP sibling terms leaked\n'; elif [[ "$prompt" == *"KNOWLEDGE TERM_C 24"* ]]; then printf 'JOIN\n'; else printf 'SET TERM_C 24\n'; fi
elif [[ "$prompt" == *"Current Aspect**: Parallel Arithmetic"* ]]; then
  if [[ "$prompt" == *"FORK term-a calculate seven"* && "$prompt" == *"FORK term-b calculate eleven"* && "$prompt" == *"FORK term-c calculate twenty-four"* ]]; then
    if [[ "$prompt" == *"FORK_KNOWLEDGE term-a TERM_A 7"* && "$prompt" == *"FORK_KNOWLEDGE term-b TERM_B 11"* && "$prompt" == *"FORK_KNOWLEDGE term-c TERM_C 24"* ]]; then
      if [[ "$prompt" == *"KNOWLEDGE TOTAL 42"* ]]; then printf 'JOIN\n'; else printf 'SET TOTAL 42\n'; fi
    else
      printf 'STOP fork knowledge missing from parent context\n'
    fi
  elif [[ "$prompt" != *"FORK term-a calculate seven"* ]]; then
    printf 'FORK term-a calculate seven\n'
  elif [[ "$prompt" != *"FORK term-b calculate eleven"* ]]; then
    printf 'FORK term-b calculate eleven\n'
  else
    printf 'FORK term-c calculate twenty-four\n'
  fi
elif [[ "$prompt" == *"Current Aspect**: Verify Arithmetic"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE TERM_A 7"* && "$prompt" == *"KNOWLEDGE TERM_B 11"* && "$prompt" == *"KNOWLEDGE TERM_C 24"* && "$prompt" == *"KNOWLEDGE TOTAL 42"* ]]; then
    printf 'DONE TOTAL 42\n'
  else
    printf 'STOP arithmetic terms missing from Corpus\n'
  fi
elif [[ "$prompt" == *"Current Aspect**: Fantasy Characters"* && "$prompt" == *"Fork Directive**: create a moon ranger"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE CHARACTER Lyra"* ]]; then printf 'JOIN\n'; else printf 'SET CHARACTER Lyra\n'; fi
elif [[ "$prompt" == *"Current Aspect**: Fantasy Characters"* && "$prompt" == *"Fork Directive**: create a storm mage"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE CHARACTER Lyra"* ]]; then printf 'STOP sibling character leaked\n'; elif [[ "$prompt" == *"KNOWLEDGE CHARACTER Orin"* ]]; then printf 'CONTINUE\n'; else printf 'SET CHARACTER Orin\n'; fi
elif [[ "$prompt" == *"Current Aspect**: Fantasy Characters"* && "$prompt" == *"Fork Directive**: create a clockwork rogue"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE CHARACTER Lyra"* || "$prompt" == *"KNOWLEDGE CHARACTER Orin"* ]]; then printf 'STOP sibling characters leaked\n'; elif [[ "$prompt" == *"KNOWLEDGE CHARACTER Vex"* ]]; then printf 'JOIN\n'; else printf 'SET CHARACTER Vex\n'; fi
elif [[ "$prompt" == *"Current Aspect**: Fantasy Characters"* ]]; then
  if [[ "$prompt" == *"FORK hero create a moon ranger"* && "$prompt" == *"FORK mage create a storm mage"* && "$prompt" == *"FORK rogue create a clockwork rogue"* ]]; then
    if [[ "$prompt" == *"FORK_KNOWLEDGE hero CHARACTER Lyra"* && "$prompt" == *"FORK_KNOWLEDGE mage CHARACTER Orin"* && "$prompt" == *"FORK_KNOWLEDGE rogue CHARACTER Vex"* ]]; then
      if [[ "$prompt" == *"KNOWLEDGE CAST Lyra Orin Vex"* ]]; then printf 'CONTINUE\n'; else printf 'SET CAST Lyra Orin Vex\n'; fi
    else
      printf 'STOP character knowledge missing from parent context\n'
    fi
  elif [[ "$prompt" != *"FORK hero create a moon ranger"* ]]; then
    printf 'FORK hero create a moon ranger\n'
  elif [[ "$prompt" != *"FORK mage create a storm mage"* ]]; then
    printf 'FORK mage create a storm mage\n'
  else
    printf 'FORK rogue create a clockwork rogue\n'
  fi
elif [[ "$prompt" == *"Current Aspect**: Write Short Story"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE CHARACTER Lyra"* && "$prompt" == *"KNOWLEDGE CHARACTER Orin"* && "$prompt" == *"KNOWLEDGE CHARACTER Vex"* && "$prompt" == *"KNOWLEDGE CAST Lyra Orin Vex"* ]]; then
    printf 'DONE Lyra, Orin, and Vex sealed the rift.\n'
  else
    printf 'STOP characters missing from Corpus\n'
  fi
elif [[ "$prompt" == *"Fork Directive**: child branch"* ]]; then
  if [[ "$prompt" == *"KNOWLEDGE Child branch complete"* ]]; then printf 'JOIN\n'; else printf 'SET Child branch complete\n'; fi
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

fn run_pipeline(pipeline: &str, request: &str) -> std::process::Output {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let llm = write_test_llm(temp_dir.path());

    Command::new(env!("CARGO_BIN_EXE_piper"))
        .args(["--file", "piper.toml", "run", pipeline, request])
        .env(
            "PIPER_LLM_INVOCATION",
            format!("{} {{{{prompt}}}}", llm.display()),
        )
        .env("PIPER_DEFAULT_MODEL", "cerebras/gemma-4-31b")
        .output()
        .expect("piper CLI should run")
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn cli_runs_pipeline_incrementally() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let llm = write_test_llm(temp_dir.path());
    let output = Command::new(env!("CARGO_BIN_EXE_piper"))
        .args([
            "--file",
            "piper.toml",
            "run",
            "tests/pipelines/black-box.toml",
            "Run the black-box pipeline",
        ])
        .env(
            "PIPER_LLM_INVOCATION",
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

#[test]
fn cli_relays_knowledge_between_aspects() {
    let output = run_pipeline(
        "tests/pipelines/knowledge-relay.toml",
        "Relay an independently derived token",
    );
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("RELAY quartz-17"));
}

#[test]
fn cli_replaces_knowledge_before_relaying_it() {
    let output = run_pipeline(
        "tests/pipelines/knowledge-replacement.toml",
        "Refine and relay one numeric value",
    );
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("VALUE 17"));
    assert!(!stdout.contains("VALUE wrong"));
}

#[test]
fn cli_joins_parallel_arithmetic_knowledge() {
    let output = run_pipeline(
        "tests/pipelines/parallel-arithmetic.toml",
        "Calculate a total from independent terms",
    );
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("TOTAL 42"));
}

#[test]
fn cli_joins_characters_into_a_story() {
    let output = run_pipeline(
        "tests/pipelines/fantasy-characters.toml",
        "Write a story from independently generated characters",
    );
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    for character in ["Lyra", "Orin", "Vex"] {
        assert!(stdout.contains(character), "story omitted {character}");
    }
}

#[test]
fn cli_limits_recovery_calls_across_aspects() {
    let output = run_pipeline(
        "tests/pipelines/recovery-budget.toml",
        "Exercise the shared recovery budget",
    );

    assert!(!output.status.success(), "second recovery should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("max_recoveries limit of 1"), "{stderr}");
}
