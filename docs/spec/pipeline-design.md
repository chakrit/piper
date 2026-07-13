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
  `CONTINUE` or `JOIN` to return its isolated Corpus. The parent sees its completed fork
  history and uses either alias to join all child Corpora before another Aspect starts.
- **Explicit termination**: Use `STOP <reason>` for a clean blocked or abandoned run and
  `DONE <result>` for a completed final synthesis.
- **Bounded execution**: Set `max_turns` high enough for intentional `SET` loops and forks,
  but low enough to stop a malformed or indecisive run.

## Design workflow

1. **Request**: State the desired outcome and immutable input together as one Request.
2. **Constraint extraction**: Identify the durable facts and decisions later Aspects need.
3. **Aspect design**: Give each Aspect one question or concern and order them by dependency.
4. **Fork design**: Add a fork only for genuinely independent exploration. Give each branch
   a distinct directive so the parent Worker can recognize it in fork history, and have the
   child return with `CONTINUE` or its semantic alias `JOIN`.
5. **Termination**: Add a synthesis Aspect only when a final Result is needed; otherwise a
   pipeline may end after its Corpus has been expanded.
