<!-- not spec/decision because: this tracks active, unsettled implementation work and session recovery state -->

# Pipeline engine task backlog

## Resume state

- Branch: `main`, ahead of `gh/main` by two committed slices before this session's
  documentation work.
- Committed `37074a7 define join alias to simplify branch completion`.
- Committed `65db0e4 exercise corpus flow through fork joins`.
- Uncommitted human documentation is present in `README.md`,
  `docs/spec/anti-harness.md`, and three files under `docs/guides/`.
- Documentation has not been committed because its CLI examples must follow the pending
  removal of `--goal`.
- Gemma/OpenCode token spend during this session: 0. All pipeline tests used local,
  deterministic Workers.

## Accepted direction

- Piper is an anti-harness: deterministic Rust code owns orchestration while models solve
  narrow Aspect tasks with small context and a tiny one-line DSL.
- `CONTINUE` and `JOIN` are exact aliases. At root they join completed children and
  advance; in a fork they join descendants and return the branch Corpus to the parent.
- Goal belongs in the Request text. Remove Goal as a separate CLI, state, and prompt
  concept.
- Pipeline-oriented CLI convention: pipeline file first, Request second. The intended run
  surface is `piper run <pipeline-file> <request>`.
- Automated tests should use deterministic local Workers. Announce and track any real
  model calls separately.

## Active tasks

1. Align accepted specs with the removal of Goal and the positional Request CLI.
2. Add failing CLI/state/prompt tests for `piper run <pipeline-file> <request>` and the
   absence of a separate Goal.
3. Remove `goal` from CLI parsing, `PipelineState`, prompt substitution, templates, tests,
   fixtures, and dogfooding examples; make the new interface green.
4. Update all uncommitted README and guide command examples and conceptual prose to the
   final CLI.
5. Audit and commit the human documentation slice.
6. Continue adding many small, increasingly complex pipeline fixtures:
   nested forks; fork isolation; conflicting branch contributions; arithmetic chains;
   `JUMP` with retained Corpus; `STOP` propagation; child/root `DONE`; formatter recovery;
   exact shared-budget boundaries; and automated gate behavior where feasible.
7. Fix every engine defect exposed by those fixtures before increasing complexity.
8. Inspect logging tools and libraries in the local `ace-rs/ace` project, then port the
   smallest fitting logging stack into Piper. Add output-focused tests and improve run,
   fork, recovery, gate, stop, and completion logs without mixing this into the Goal-removal
   commit.
   Logs must correlate every event with Aspect ID/name and branch identity, and print the
   exact rendered Worker input plus raw output so a human can eyeball-debug a run. Recovery
   calls need the same visibility.

Recovery is implemented in `src/driver.rs` and covered by a focused unit test. Add
black-box fixtures for successful one-shot recovery, failure after a second malformed
response, preserved Corpus during recovery, and recovery inside a fork.

## Documentation in progress

- `README.md`: human entry point and anti-harness framing.
- `docs/spec/anti-harness.md`: accepted design rationale and code/model boundary.
- `docs/guides/writing-pipelines.md`: command reference and common patterns for humans.
- `docs/guides/research-and-synthesis.md`: longer fan-out/fan-in research walkthrough.
- `docs/guides/engineering-review.md`: longer engineering readiness walkthrough.

## Verification state

Before the Goal-removal task began, these passed:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test` (25 unit tests and 5 CLI tests)
- ShellCheck over the embedded deterministic Bash Worker in `tests/cli.rs`

Rerun all checks after the CLI/state changes and before each code commit.
