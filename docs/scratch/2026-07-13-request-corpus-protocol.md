<!-- not spec/decision because: this is a point-in-time implementation handoff record -->

# Session State: 2026-07-13 Request–Corpus Protocol

## Completed

- Commit `8b2448d` established the interim Request/Corpus terminology migration: immutable
  Request, Worker Aspects, and Corpus-oriented prompts.
- The accepted target protocol is now specified in `docs/spec/aop-engine.md` and
  `docs/spec/pipeline-design.md`.

## Accepted Target

- Workers preserve the Request and contribute to the Corpus.
- The DSL is `SET`, `CONTINUE`, `JUMP`, `FORK`, `STOP`, and `DONE`.
- `SET` owns the current Worker's contribution in its current branch; no arbitrary
  knowledge keys or separate add/update commands remain.
- Forks are isolated branches. Each child completes synchronously; the parent receives
  fork history, then `CONTINUE` automatically joins all completed child Corpora.
- `JUMP`, `STOP`, and `DONE` are invalid while the current Worker has unjoined forks.

## Next Implementation Task

Implement the accepted protocol as one coherent migration. Start with failing parser and
driver tests for `SET`, `CONTINUE`, `STOP`, automatic fork joins, and fork-history context.
Then update the runtime data model, prompts, example pipelines, and documentation surface
in lockstep. Do not retain compatibility aliases for `NEXT`, `ADD-KNOWLEDGE`,
`UPDATE-KNOWLEDGE`, or explicit `MERGE`.

## Checkpoint

The accepted protocol specifications are intentionally uncommitted in this working tree;
no implementation work has started after they were written. Resume by reviewing and
committing the specification changes, then begin the test-first protocol migration.
