<!-- not spec/decision because: this tracks active, unsettled implementation work and session recovery state -->

# Pipeline engine task backlog

## Resume state

- Branch: `main`, ahead of `gh/main` by 17 commits before this breadcrumb update. The only
  unrelated working-tree entry is the pre-existing untracked `tmp/` directory; leave it
  untouched unless the user explicitly scopes it in.
- Accepted the durable run contract in `docs/spec/run-lifecycle.md`: XDG project state,
  stable pipeline IDs, ULID run IDs with human-friendly prefixes, per-pipeline active
  pointers, and interactive selection for ambiguous `continue`/`abort` commands.
- Landed the first execution groundwork in `2b3ccc6` through `2337b58`: accepted spec,
  pipeline identity, XDG project metadata, restartable in-memory frames, and an initial
  model-call checkpoint experiment.
- Completed a two-phase ACE audit and committed its fixes as
  `9da7c99 keep durable execution state coherent`. The audit made `PipelineState` the sole
  budget owner, replaced invalid frame option pairs with `FrameScope`, validates frame
  invariants before driving, anchors environment files beside the selected shared config,
  hardens private XDG metadata creation, and removes the premature checkpoint/serde seam
  until a real durable run aggregate owns it.
- Added `piper config <pipeline.toml>` and checkout-independent runtime templates in
  `e01b3a0 make runtime configuration inspectable and portable`. Effective TOML includes
  environment interpolation, defaults, and resolved file-backed Aspects. The built-in
  Worker and recovery templates are compiled into the binary; `--prompts` remains the
  explicit custom-directory override.
- Required verification passed after the audit: `cargo fmt --check`, clippy over all
  targets/features with warnings denied, 46 unit tests, 9 CLI tests, and `git diff --check`.
- A complete checkout scan found no tracked, ignored, cached, or hidden Python artifacts.
  Git history only records the already-deleted legacy Python tree in `67cd25f` and
  `0fe8100`; do not rewrite history unless explicitly requested.
- The XDG `project.json` namespace remains intentionally because it is accepted behavior
  and provides a concrete canonical-project identity/collision check. Durable run
  snapshots, active pointers, lifecycle commands, and terminal selection are not yet
  implemented.
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
5. Review and execute the phased plan in
   `docs/scratch/2026-07-13-resumable-interactive-web-ui-plan.md`: persistent
   `start`/`continue`, attended TUI gates, then Ticket-Hamster-style `piper ui`.
6. Resolve the existing child-`DONE` contradiction before persistence: the driver currently
   returns a child Corpus to its parent, while the accepted engine specification says
   `DONE` terminates the entire run. Add an exact fixture and make code and spec agree.

## Verification state

After `e01b3a0`, these passed:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test` (46 unit tests and 9 CLI tests)
- `git diff --check`

Rerun all checks before each code commit.
