# Piper

Piper is a standalone Rust CLI for context-engineering pipelines. It applies ordered
Aspect Workers to an immutable Request and Goal, accumulates Knowledge in a shared Corpus,
and supports routing, isolated forks, merges, human gates, bounded execution, and explicit
completion.

## Repository shape

- `src/main.rs`: CLI parsing and `init` / `run` dispatch.
- `src/driver.rs`: pipeline state machine, prompt execution, recovery, gates, and forks.
- `src/dsl.rs`: parser for the single-line Worker action protocol.
- `src/state.rs`: pipeline state, fork Corpora, and the shared turn budget.
- `src/config.rs`: TOML configuration and inline/file-backed Aspect resolution.
- `src/llm.rs`: external LLM command invocation.
- `src/actions/`: filesystem reads and mutations.
- `prompts/`: runtime Worker templates.
- `pipelines/`: checked-in pipeline configurations.

## Verification

Run before completing code changes:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

The crate uses `#![deny(warnings)]`. Tests are colocated with the Rust modules they cover.

## Conventions

- Keep the Request immutable; Workers add or update Knowledge in the Corpus.
- Worker output is exactly one DSL command on one non-empty line.
- The root run and all forks share one global turn budget.
- Keep filesystem access behind actions in `src/actions/`.
- Use short, imperative commit subjects, consistent with recent history.

## ACE skills

- `general-coding`: project-wide planning, testing, editing, review, and commit rules.
- `rust-coding`: Rust and Cargo conventions.
- `ace`: recurring per-task workflow.
- `ace-save`: durable session handoff.
- `ace-docs`: durable documentation routing and scaffolding.
- `ace-audit`: focused audits of implementation and specification changes.

## Durable artifacts

Use [`docs/README.md`](docs/README.md) as the routing gate. Project behavior and exact
surfaces belong in `docs/spec/`; decisions in `docs/decisions/`; operating guides in
`docs/guides/`; third-party references in `docs/vendor/`; and genuinely unsettled work in
`docs/scratch/`.
