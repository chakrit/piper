# Piper

Piper is a standalone Rust CLI for running context-engineering pipelines. It turns a
large, general instruction into a sequence of narrow Aspect Workers, gives each Worker
only the context it needs, and drives the control flow itself.

## An anti-harness for LLM work

Most agent harnesses ask one capable model to understand the job, maintain a plan, choose
tools, manage parallel work, remember accumulated facts, and decide when it is finished.
That spends model context on orchestration and leaves the execution policy to another
prompt.

Piper takes the opposite approach. The orchestration is a deterministic Rust state
machine; the model sees one focused Aspect and returns one line of a tiny DSL. Piper
mechanically records knowledge, advances the pipeline, routes control, runs isolated
forks, pauses at human gates, enforces a shared turn budget, and recognizes explicit
completion.

This is an **anti-harness**: less general prompting, less model-managed machinery, and
smaller context by construction. A smaller or less-capable model can concentrate on a
narrow question instead of simulating an entire autonomous agent. Each Aspect can have
its own prompt and model, so effort can be placed where it matters rather than paid for on
every turn.

The core transition is:

```text
immutable Request + current Aspect + accumulated Corpus
    -> one DSL command
    -> deterministic state transition
```

Piper does not make the underlying analysis deterministic—the Worker is still an LLM.
It makes the execution loop, state transitions, isolation, routing, and limits explicit
and code-driven.

## How it works

- The **Request** is the immutable input and desired outcome being considered.
- An **Aspect** is one focused lens or question.
- A **Worker** applies that Aspect and emits exactly one command.
- `SET` adds or replaces that Worker's durable contribution to the shared **Corpus**.
- Later Workers receive the accumulated Corpus as `KNOWLEDGE` directives.
- A **fork** explores from a copy of the current Corpus and returns its findings to its
  parent. `CONTINUE` and `JOIN` are exact aliases: at the root they join children and
  advance; inside a fork they join descendants and return to the parent.
- `DONE` returns a final result, while `STOP` ends a blocked or abandoned run cleanly.

Worker output is deliberately small:

```text
SET Preserve public API compatibility.
CONTINUE
JOIN
JUMP security-review
FORK alternative-1 Explore migration risk.
STOP Missing required credentials.
DONE The request is ready to implement.
```

Every response must be exactly one non-empty command line. Piper parses and applies it;
the Worker never edits pipeline state directly. See
[`docs/spec/aop-engine.md`](docs/spec/aop-engine.md) for the exact grammar and execution
semantics.

## Quick start

Prerequisites:

- a current stable Rust toolchain;
- the LLM command configured in `piper.toml` (the committed development defaults invoke
  OpenCode through values loaded from `.env`).

Build and inspect the CLI:

```bash
cargo build
cargo run -- --help
```

Run a pipeline:

```bash
cargo run -- --file piper.toml run tests/pipelines/smoke.toml \
  "Verify the Piper driver by confirming the Worker protocol."
```

`--file` selects the shared runtime configuration and defaults to `piper.toml`. The
positional path after `run` selects a pipeline file. `--prompts` can select a different
Worker-template directory and defaults to `prompts`.

Runtime configuration supports `${VAR}` interpolation. Piper loads `.env`, then
`.env.local`, then process environment values take precedence. The committed `.env`
contains non-production clone-and-run defaults; use the gitignored `.env.local` for
personal overrides.

## Writing a pipeline

A pipeline is an ordered list of inline or file-backed Aspects:

```toml
[pipeline]
max_turns = 30
max_recoveries = 3

[[pipeline.aspects]]
id = "requirements"
name = "Requirements"
prompt = "Record the constraints later Workers must preserve."

[[pipeline.aspects]]
id = "risk-review"
name = "Risk review"
prompt = "Identify risks using the accumulated knowledge."
model = "optional-model-override"

[[pipeline.aspects]]
id = "approval"
name = "Human approval"
type = "gate"
prompt = ""

[[pipeline.aspects]]
id = "synthesis"
name = "Synthesis"
prompt = "Return the final answer with DONE."
```

Aspects default to `type = "worker"`; only human gates need `type = "gate"`.
`max_turns` defaults to 100 and is shared by the root execution and every fork.
`max_recoveries` defaults to 3 and limits formatter-recovery calls across that same run;
set it to 0 to disable recovery. A Worker may revisit itself with `SET`, route to an
`ASPECT` target with `JUMP`, or launch isolated work with `FORK`, but the Rust driver
remains responsible for every transition.

The checked-in pipelines under [`tests/pipelines/`](tests/pipelines/) are small executable
examples of knowledge relay, replacement, fork/join synthesis, arithmetic, and larger
interaction scenarios.

Continue with the human documentation that matches what you are trying to do:

- [Understand Piper's anti-harness design](docs/spec/anti-harness.md).
- [Learn every command and common pipeline pattern](docs/guides/writing-pipelines.md).
- [Build a fan-out research and synthesis pipeline](docs/guides/research-and-synthesis.md).
- [Build a software-engineering readiness review](docs/guides/engineering-review.md).

For concise design rules, see
[`docs/spec/pipeline-design.md`](docs/spec/pipeline-design.md).

## Repository map

- `src/driver.rs` — pipeline state machine, recovery, gates, and forks.
- `src/dsl.rs` — parser for the one-line Worker protocol.
- `src/state.rs` — Corpus, branch state, and shared turn budget.
- `src/config.rs` — TOML loading, environment interpolation, and Aspect resolution.
- `src/llm.rs` — invocation of the configured external LLM command.
- `src/actions/` — filesystem reads and mutations.
- `prompts/` — runtime Worker and recovery templates.
- `tests/pipelines/` — executable pipeline fixtures and examples.
- `docs/` — specifications, decisions, guides, and design notes.

## Development

Before submitting a code change, run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

The crate uses `#![deny(warnings)]`.
