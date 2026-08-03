# Writing Piper pipelines

Piper works best when each Aspect asks a small question and leaves one useful finding for
the next Aspect. The Rust driver owns sequencing, branching, joining, routing, and turn
limits; the model only applies the current lens and returns one DSL command.

This guide shows the common composition patterns. The
[AOP engine specification](../spec/aop-engine.md) remains the authority for exact parsing
and state transitions.

## Configure the model and pipeline separately

Keep model invocation and defaults in the shared configuration, normally `piper.toml`:

```toml
[llm]
invocation = "claude -p --model {{model}} {{prompt}}"
system_prompt = "You are a precise AOP worker."
default_model = "sonnet"
```

Put the ordered work in a separate pipeline file:

```toml
[pipeline]
id = "writing-example"
max_turns = 8
max_recoveries = 2

[[pipeline.aspects]]
id = "constraints"
name = "Extract Constraints"
prompt = "Contribute the constraints that later analysis must preserve."

[[pipeline.aspects]]
id = "answer"
name = "Synthesize Answer"
prompt = "Answer the Request using the accumulated Corpus."
model = "optional-model-override"
```

An Aspect defaults to `type = "worker"`. Use `type = "gate"` only for a human gate.
Aspect IDs must be unique because `JUMP` targets them. An Aspect may also be stored in its
own TOML file and referenced from the sequence:

```toml
[[pipeline.aspects]]
file = "aspects/security.toml"
```

The referenced file contains the Aspect table directly, without an
`[[pipeline.aspects]]` wrapper:

```toml
id = "security"
name = "Security Review"
prompt = "Identify concrete security constraints the plan must preserve."
```

Run the pipeline with an immutable Request that includes the desired outcome:

```sh
cargo run -- --file piper.toml run pipelines/review.toml \
  "Produce a safe plan to move the service without breaking its public API."
```

The selected shared configuration's directory supplies `.env`, then `.env.local`, then
process-environment overrides. `${NAME}` interpolation is available in configuration and
pipeline TOML.

## Make every Aspect narrow

A good Aspect has one concern and produces knowledge another Aspect can use. Prefer
prompts such as “derive the exact invariants the implementation must preserve” over
“analyze everything.” Order the sequence by dependency: establish facts first, analyze
their implications next, and synthesize last.

Do not place an expected derived value in a later Aspect's prompt. If the final Aspect is
supposed to use a token, number, character, or decision produced earlier, mention only the
operation it must perform. This makes a pipeline test prove Corpus flow instead of prompt
memorization.

The Request never changes. Knowledge flows forward through the Corpus as compact
`KNOWLEDGE ...` directives. Piper, rather than a Worker, assigns contribution identity
from the current branch and Aspect.

## Use the Worker commands

Every Worker response must be exactly one non-empty command line.

- `SET <knowledge>` records the current Worker's contribution and re-runs the same
  Aspect. A later `SET` by that Worker replaces its previous contribution; it does not
  append another entry.
- `CONTINUE` joins completed child Corpora, then advances a root run or returns a child
  branch to its parent.
- `JOIN` is an exact alias of `CONTINUE`. It has no distinct merge mode or timing.
- `JUMP <aspect-id>` revisits or skips to an Aspect listed in the runtime `ASPECT`
  directives. It cannot discard unjoined fork work.
- `FORK <fork-id> <directive>` runs an isolated child on the current Aspect. The child sees
  the parent instruction, its narrower fork instruction, and a `CONTEXT` path. The parent
  runs again after the child returns and sees correlated `FORK_KNOWLEDGE`, while that
  knowledge remains pending until the parent continues or joins.
- `STOP <reason>` cleanly stops the entire run without a final Result. A child stop also
  stops its parent run.
- `DONE <result>` immediately returns the final Result. Use it for synthesis, not for
  returning a child branch. It cannot discard unjoined fork work.

`CONTINUE` and `JOIN` deliberately require no choice between two behaviors. Context alone
determines the transition:

| Context | Effect of either command |
| --- | --- |
| Root, no completed forks | Advance to the next Aspect. |
| Root, completed forks | Join all child Corpora, clear fork history, then advance. |
| Child branch | Join any descendants, then return the branch Corpus to its parent. |

## Compose common patterns

### Relay knowledge

Have an early Aspect derive an opaque fact with `SET`, then `CONTINUE`. A later Aspect
must receive that fact through the Corpus before it can produce `DONE`.

```toml
[[pipeline.aspects]]
id = "seed"
name = "Seed Knowledge"
prompt = "Derive one opaque token and contribute it."

[[pipeline.aspects]]
id = "relay"
name = "Relay Knowledge"
prompt = "Return the opaque token supplied through the Corpus."
```

See [`knowledge-relay.toml`](../../tests/pipelines/knowledge-relay.toml).

### Refine by replacement

`SET` is useful for a small self-correction loop. The first pass records a draft; the next
pass sees it as knowledge and replaces it with a refined contribution. Once satisfied, the
Worker continues. Downstream Aspects see only the replacement.

```text
SET VALUE tentative
SET VALUE 17
CONTINUE
```

These are three successive Worker responses, not a multiline response. See
[`knowledge-replacement.toml`](../../tests/pipelines/knowledge-replacement.toml).

### Fan out, join, and synthesize

Use forks for independent work whose results should meet at a later Aspect:

1. The parent emits one `FORK` per independent directive.
2. Each child runs the current Aspect with a copy of the parent's starting Corpus and a
   narrower fork instruction.
3. Each child uses `SET`, then returns with either `CONTINUE` or `JOIN`.
4. The parent runs again after each child. It sees `FORK` history and correlated pending
   results as `FORK_KNOWLEDGE`, but siblings do not see one another's results.
5. The parent may use `SET` to consolidate the pending results inside the same Aspect.
6. The parent emits `CONTINUE` or `JOIN`; Piper joins every completed child Corpus and
   advances.
7. Later Aspects receive all joined contributions as ordinary `KNOWLEDGE`.

```text
FORK term-a calculate seven
FORK term-b calculate eleven
FORK term-c calculate twenty-four
JOIN
```

Those are successive responses from the parent. Child contributions remain isolated from
one another, while the parent can inspect each completed result through correlated
`FORK_KNOWLEDGE`. This makes fan-out behavior easy to test: siblings must not receive one
another's terms, the parent must receive every pending term, and later Aspects must receive
them as joined `KNOWLEDGE`. See
[`parallel-arithmetic.toml`](../../tests/pipelines/parallel-arithmetic.toml) and
[`fantasy-characters.toml`](../../tests/pipelines/fantasy-characters.toml).

Forks execute synchronously today. Determinism comes from the driver applying an ordered
state machine; it does not depend on an LLM deciding how to schedule or merge branches.

### Route and revisit

Use `JUMP review` when accumulated knowledge reveals that a named Aspect needs another
pass. The target must be one of the `ASPECT` IDs supplied in the runtime context. Keep the
reason for revisiting visible in the current contribution, and budget for both the `SET`
pass and the jump target's pass. Prefer ordinary ordering when a revisit is always needed;
reserve `JUMP` for a conditional route.

### Stop or finish

Use `STOP <reason>` when the pipeline cannot responsibly complete, for example because a
required input is missing. Use `DONE <result>` when the Corpus supports the requested
final answer. A child returns knowledge with `CONTINUE` or `JOIN`; it should not use
`DONE` as a branch-return command.

### Insert a human gate

A gate pauses execution for a person without consuming a model turn:

```toml
[[pipeline.aspects]]
id = "approval"
name = "Approve Migration"
type = "gate"
prompt = ""
```

Place a gate after the analysis a reviewer needs and before the work their approval
controls. Gates do not mutate the Request or Corpus.

## Set a realistic shared turn budget

`max_turns` defaults to `100` and must be at least one. The root and every fork share this
single budget. Every Worker pass consumes one turn, including the pass after `SET`, each
parent pass that launches or observes forks, and child passes. Gates consume no turns. A
one-shot parser-recovery call belongs to the failed pass and does not consume an additional
turn, but it consumes the shared `max_recoveries` budget. That budget defaults to `3`; set
it to `0` to disable recovery.

Count the intended happy-path passes, include replacement loops and every fork, then leave
a small allowance for legitimate conditional routing. Keep the turn limit low enough that
a Worker repeatedly setting, jumping, or forking fails promptly. Keep `max_recoveries`
much lower: recovery is for occasional formatting errors, not a normal second call on each
pass. A three-fork pipeline often needs substantially more turns than its two configured
Aspects suggest.

## Test information flow, not just completion

Use deterministic local Workers in automated tests. They spend no model tokens, make
failures reproducible, and can reject prompts that violate isolation or ordering.

For each pipeline fixture:

1. Put it under `tests/pipelines/`; do not use the repository's dogfooding `piper.toml` as
   a fixture.
2. Give the fake Worker values absent from downstream Aspect prompts and the Request.
3. Make each fake response conditional on the expected `KNOWLEDGE`, `FORK`, Aspect, and
   fork-directive context.
4. Return `STOP` on missing, premature, leaked, duplicated, or stale knowledge.
5. Assert the exact machine-checkable result, such as `TOTAL 42`, before adding prose
   synthesis cases.
6. Exercise both `CONTINUE` and `JOIN` in equivalent child and parent positions.

Good small fixtures build a ladder: opaque relay, replacement, arithmetic chain, parallel
fan-out, isolation before join, nested forks, conditional routing, gates, termination,
recovery, and exact budget boundaries. The checked-in CLI tests in
[`tests/cli.rs`](../../tests/cli.rs) demonstrate a shell-backed deterministic Worker and
verify both numeric synthesis and a story containing all independently generated
characters.

Before landing pipeline or engine changes, run:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Diagnose common failures

- **A later Aspect invents or misses a fact:** check that the producer returned `SET`
  before continuing, and that the consumer prompt did not accidentally contain the test
  value. Inspect its `KNOWLEDGE` context.
- **A replacement leaves stale knowledge:** replacement is scoped to the same branch and
  Aspect. Two different Aspects or two forks create distinct contributions even when their
  text looks related.
- **Fork results do not reach later Aspects:** a child must return with `CONTINUE` or
  `JOIN`, and the parent must later do the same to make all pending results durable before
  advancing.
- **A child does not return its knowledge:** `DONE` is not a branch-return command. Use
  `CONTINUE` or `JOIN` in a child so its Corpus returns to the parent.
- **`JUMP` or `DONE` is rejected after forking:** the current parent still has unjoined
  fork work. Join it before routing or completing.
- **A fork repeats forever:** give every branch a distinct ID and directive, and tell the
  parent to inspect its runtime `FORK` history before launching another branch.
- **The run exhausts `max_turns`:** count actual Worker passes, not Aspects. `SET` re-runs
  its Aspect, every child uses the shared budget, and every parent re-run after a fork also
  counts.
- **A response fails to parse:** it must be one uppercase command on one non-empty line,
  with no Markdown fence, commentary, indentation, or second line.
- **A pipeline behaves differently in a live run:** first reproduce it with a deterministic
  fake Worker. That separates driver and context-flow defects from model variability and
  avoids spending model tokens while debugging mechanics.
