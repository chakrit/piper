<!-- not spec/decision because: this tracks active, unsettled implementation work and session recovery state -->

# Pipeline engine task backlog

## Resume state

- Branch: `main`, ahead of `gh/main` by five commits before the recovery-budget slice.
- Committed `37074a7 define join alias to simplify branch completion`.
- Committed `65db0e4 exercise corpus flow through fork joins`.
- Committed `e32a9da fold desired outcome into the request`.
- Committed `c43846a explain Piper as a deterministic anti-harness`.
- Committed `ae663fd track engine hardening work across sessions`.
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

1. Continue adding many small, increasingly complex pipeline fixtures:
   nested forks; fork isolation; conflicting branch contributions; arithmetic chains;
   `JUMP` with retained Corpus; `STOP` propagation; child/root `DONE`; formatter recovery;
   exact shared-budget boundaries; and automated gate behavior where feasible.
2. Fix every engine defect exposed by those fixtures before increasing complexity.
3. Finish the `console 0.16.2` safety audit at the user-selected depth, then port the
   smallest fitting logging stack into Piper. Add output-focused tests and improve run,
   fork, recovery, gate, stop, and completion logs without mixing this into the Goal-removal
   commit.
   Logs must correlate every event with Aspect ID/name and branch identity, and print the
   exact rendered Worker input plus raw output so a human can eyeball-debug a run. Recovery
   calls need the same visibility.
4. Expand recovery acceptance coverage beyond the committed shared-budget test: successful
   one-shot recovery through the CLI, preserved Corpus during recovery, malformed recovery
   failure, and recovery inside a fork.
5. Change forks to execute the same Aspect that launched them. Add `CONTEXT
   <aspect-id>/<fork-path...>`, explicit parent/fork instructions, and correlated
   `FORK_KNOWLEDGE` in the parent prompt. Prove sibling isolation, parent-side synthesis,
   nested context paths, and downstream joined Corpus with deterministic pipelines.

## Verification state

After the recovery-budget implementation, these passed:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test` (26 unit tests and 6 CLI tests)
- ShellCheck over the embedded deterministic Bash Worker in `tests/cli.rs`

Rerun all checks before each code commit.
