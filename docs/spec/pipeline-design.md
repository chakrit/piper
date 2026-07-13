# Pipeline Design Specification

- **Status:** accepted
- **Scope:** Designing safe, focused Aspect sequences for Piper pipelines.

## Philosophy

Aspect-Oriented Prompting (AOP) decomposes a Request into isolated lenses. Workers preserve
the Request and use `SET` to contribute durable knowledge to the Corpus. Later Aspects use
that expanded Corpus, so each concern is considered once and its findings remain available.

## Aspect ordering

Order Aspects so dependent analysis follows the knowledge it needs:

1. **Foundational Aspects** — correctness, logic, safety, and factual accuracy.
2. **Structural Aspects** — architecture, specifications, organization, and flow.
3. **Presentation Aspects** — voice, tone, brand alignment, typography, and polish.
4. **Synthesis Aspects** — produce a `DONE` Result when the pipeline requires an answer
   rather than only an expanded Corpus.

## Design principles

- **Request integrity**: Workers must never rewrite the Request.
- **One contribution per Aspect**: `SET` replaces the current Worker's own contribution;
  use a concise, consolidated statement rather than arbitrary knowledge keys.
- **Corpus first**: A later Aspect should consume earlier findings, not independently
  rediscover them.
- **Normal progression**: `CONTINUE` and `JOIN` are aliases. In the root run, either joins
  completed child Corpora and advances. In a fork, either returns the branch Corpus to its
  parent.
- **Explicit routing**: Use `JUMP` only for a meaningful revisit to an `ASPECT` target.
- **Forks are barriers**: Use `FORK` only for independent exploration. A child uses
  the same Aspect with a narrower fork instruction, then uses `CONTINUE` or `JOIN` to
  return its isolated Corpus. The parent sees correlated pending fork knowledge and uses
  either alias to join all child Corpora before another Aspect starts.
- **Explicit termination**: Use `STOP <reason>` for a clean blocked or abandoned run and
  `DONE <result>` for a completed final synthesis.
- **Bounded execution**: Set `max_turns` high enough for intentional `SET` loops and forks,
  but low enough to stop a malformed or indecisive run.
- **Bounded recovery**: Keep `max_recoveries` much lower than `max_turns`. Recovery repairs
  occasional formatting mistakes; it must not become the normal execution path.
- **Stable identity**: Give `[pipeline].id` a concise, durable name. It selects the
  pipeline's active run, so do not derive it from mutable file contents or casually rename
  it.
- **Resumable gates**: A gate is a durable pause, not an in-engine prompt. Place it only
  after all context needed for a later operator decision has been checkpointed. Each
  approval advances exactly one waiting gate.

## Design workflow

1. **Request**: State the desired outcome and immutable input together as one Request.
2. **Identity**: Choose a required `[pipeline].id` that remains meaningful across edits to
   the pipeline file.
3. **Constraint extraction**: Identify the durable facts and decisions later Aspects need.
4. **Aspect design**: Give each Aspect one question or concern and order them by dependency.
5. **Fork design**: Add a fork only for genuinely independent exploration. Give each branch
   a distinct directive that narrows the current Aspect. Have the child return with
   `CONTINUE` or its semantic alias `JOIN`, then let the parent consolidate pending results
   in the same Aspect before advancing.
6. **Gate design**: Place gates after the durable analysis a reviewer needs. Expect a
   separate `continue` invocation for each gate in unattended execution.
7. **Termination**: Add a synthesis Aspect only when a final Result is needed; otherwise a
   pipeline may end after its Corpus has been expanded.
