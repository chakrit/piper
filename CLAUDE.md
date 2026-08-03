# Piper

Piper is a standalone Rust CLI for context-engineering pipelines. It applies ordered
Aspect Workers to an immutable Request, accumulates Knowledge in a shared Corpus,
and supports routing, isolated forks, human gates, bounded execution, and explicit
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
- `piper.toml`: committed self-dogfooding pipeline for developing Piper itself.
- `.env`: committed non-production defaults for clone-and-run development.
- `tests/pipelines/`: checked-in pipeline fixtures and example configurations.
- `tests/cli.rs`: black-box CLI coverage that runs the compiled binary.

## Verification

Run before completing code changes:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

The crate uses `#![deny(warnings)]`. Keep focused module tests colocated with the Rust
modules they cover, and put black-box CLI behavior tests under `tests/`.

## Conventions

- Keep the Request immutable; Workers contribute Knowledge to the Corpus with `SET`.
- Worker output is exactly one DSL command on one non-empty line.
- Current Worker DSL: `SET`, `CONTINUE`, `JUMP`, `FORK`, `STOP`, `DONE`.
- `SET` replaces the current Worker's own contribution for the current branch/aspect.
- `CONTINUE` advances and joins any completed forks launched by the current Worker.
- Aspects default to `worker`; only gate aspects need `type = "gate"` in TOML.
- `piper.toml` is the real dogfooding config, not a test fixture.
- Purpose-built test pipelines live under `tests/pipelines/`; do not use repo-root
  `piper.toml` as a fixture.
- Config TOML may interpolate `${VAR}` from the process environment, `.env`, and
  `.env.local`. Load order is `.env`, then `.env.local`, then process environment.
- `.env` is committed for working non-production defaults; `.env.local` is gitignored for
  personal overrides. Do not add `.env.example`.
- The root run and all forks share one global turn budget.
- Keep filesystem access behind actions in `src/actions/`.
- Use short, imperative commit subjects, consistent with recent history.

## ACE skills

- `code`: execution laws — TDD, unit-of-work, naming, commit discipline.
- `architect`: design laws — abstraction quality, boundaries, API and data shape.
- `rust-coding`: Rust and Cargo conventions.
- `ace`: recurring per-task workflow.
- `ace-save`: durable session handoff.
- `ace-docs`: durable documentation routing and scaffolding.
- `ace-audit`: focused audits of implementation and specification changes.

## Durable artifacts

`docs/` holds this project's durable record.

**`docs/spec/` is authoritative — read the relevant spec before working, and comply.**
It owes no justification: a rule that departs from mainstream practice, from what you
would have picked, or from what you expected is not grounds to escalate, annotate, or
re-open it. If a spec is wrong, raise it with the user and amend the spec.

**Everything settled is a `docs/spec/` amendment** — an instruction you were given, an
approach that was agreed, a library that was picked, a convention or preference that
was fixed. Write it there as it was given: the rule, at the length it was given, with
no reason supplied and no note of what it was chosen over. A one-sentence rule — "use
RESTful routes" — is a complete entry. There is no decisions log.

File new material by the gate, first match wins: third-party lookup → `vendor/`; a
how-to → `guides/`; our own design or surface → `spec/`; unsettled exploration →
`scratch/` (last resort, opened with a "not spec because ___" line). Nothing defaults
to `scratch/`.

**Before writing into a `docs/` folder, read that folder's `README.md` first.** It
holds the folder's filing test, filename format, and lifecycle rules, and they are
binding. Nothing else surfaces them.

**`docs/spec/README.md` indexes every spec file — keep it current.** Read the index
before adding a spec file, so you amend the existing doc on a subject instead of
writing a second one. Adding, renaming, or retiring a file updates its row in the same
change.
