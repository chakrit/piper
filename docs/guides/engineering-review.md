# Run a software-engineering readiness review

This guide builds a substantial Piper pipeline for reviewing a proposed code change before
implementation. The pipeline extracts requirements, runs independent correctness, security,
and testability reviews, joins their findings, pauses for human approval, and returns a final
readiness assessment.

The example is intentionally a review pipeline, not an autonomous coding agent. The change
request remains immutable. Workers contribute analysis to the Corpus; Piper, rather than an
LLM, controls execution, branch isolation, joining, routing, limits, and completion.

## What the pipeline establishes

The run separates five jobs:

1. Convert the change request into durable requirements knowledge.
2. Launch three independent specialist branches from the same starting Corpus.
3. Join all specialist findings before an integration review sees them.
4. Let the integration reviewer either accept the evidence, stop on a blocker, or route back
   to requirements when the request is genuinely underspecified.
5. Pause at a human gate, then produce one final readiness result.

This shape helps smaller models because no Worker has to remember or orchestrate the whole
review. Each call receives one narrow Aspect, the immutable Request, and the Corpus
available at that point. The Rust driver performs the execution loop mechanically.

## Create the pipeline

Save the following as `engineering-review.toml`:

```toml
[pipeline]
id = "engineering-review"
max_turns = 24
max_recoveries = 3

[[pipeline.aspects]]
id = "requirements"
name = "Requirements"
prompt = "Extract a concise implementation contract: required behavior, compatibility constraints, explicit non-goals, and acceptance criteria. Use SET to record one consolidated contribution. Once that contribution is present in the Corpus, use CONTINUE. If the request cannot support a safe review, use STOP with the missing information."

[[pipeline.aspects]]
id = "dispatch"
name = "Dispatch and Consolidate Specialist Reviews"
prompt = "At root, launch exactly three independent reviews with distinct fork IDs: correctness for behavioral and state-transition errors; security for trust boundaries, validation, and abuse cases; testability for observable acceptance checks and regression coverage. Use completed FORK history to avoid duplicates. In a fork, perform only the Fork Directive using the Request and requirements knowledge, SET one concise finding with severity and concrete verification, then return with CONTINUE or JOIN. At root after all three return, use correlated FORK_KNOWLEDGE to SET one consolidated specialist record, then CONTINUE or JOIN to make every finding durable and advance."

[[pipeline.aspects]]
id = "integration"
name = "Integrated Engineering Review"
prompt = "Reconcile the requirements and every joined specialist finding. SET one readiness record with blockers, required tests, and a go/no-go recommendation. On the next pass, use CONTINUE when the evidence is sufficient, STOP when a known blocker makes progress unsafe, or JUMP requirements only when the immutable Request lacks information needed for a defensible decision."

[[pipeline.aspects]]
id = "approval"
name = "Human Approval"
type = "gate"
prompt = ""

[[pipeline.aspects]]
id = "result"
name = "Readiness Result"
prompt = "Using only the Request and accumulated Corpus, return a short implementation-readiness verdict with DONE. Include the decision, blocking conditions if any, and the first verification step."
```

Aspect IDs must contain only letters, digits, hyphens, or underscores. Aspects default to
`type = "worker"`; only the approval checkpoint needs `type = "gate"`. A per-Aspect `model`
may be set when a specialist benefits from a different configured model.

## Run it

The shared LLM configuration remains in `piper.toml`; the file above contains only the
pipeline:

```bash
cargo run -- --file piper.toml run engineering-review.toml \
  "Decide whether adding idempotency keys to POST /payments without changing successful response bodies is specified and safe enough to implement."
```

`--file` defaults to `piper.toml`. The positional file after `run` is the pipeline. Use
`--prompts <directory>` only when running with a custom set of Worker templates; it defaults
to `prompts`.

Piper loads environment interpolation values from `.env`, then `.env.local`, then the
process environment. Later sources take precedence. Put personal model or invocation
overrides in the gitignored `.env.local`, not in the pipeline.

## Follow the execution

### Requirements become knowledge

The requirements Worker should first return one line such as:

```text
SET Preserve existing success bodies; apply each idempotency key within its authenticated payment scope; identical retries return the original outcome; conflicting reuse is rejected; cover concurrency and persistence failures.
```

`SET` creates or replaces this Worker's contribution, then reruns the same Aspect. It does
not advance. On the rerun, the Worker sees its contribution as a `KNOWLEDGE` directive and
can return:

```text
CONTINUE
```

The Request itself has not changed. Piper has added a durable fact to the Corpus and moved
the root run to `dispatch`.

Each branch and Aspect has one contribution slot. A later `SET` from the same Worker in the
same branch replaces that contribution instead of appending another arbitrary key. Prompts
should therefore ask for one consolidated record.

### The dispatcher creates isolated reviews

The dispatcher is rerun after every completed child. It can issue these commands over
successive passes:

```text
FORK correctness Review behavioral invariants, retries, and concurrent state transitions.
FORK security Review authentication scope, untrusted keys, leakage, and resource exhaustion.
FORK testability Define observable acceptance, concurrency, persistence, and regression checks.
JOIN
```

`FORK <id> <directive>` starts an isolated child synchronously on the current `dispatch`
Aspect. All three branches therefore share the same focused parent instruction; the fork
directive narrows that instruction to one lens. A fork receives a copy of the current
Corpus, including requirements knowledge, and a path such as `CONTEXT dispatch/security`.

When a child sets its finding, its next response is either:

```text
JOIN
```

or:

```text
CONTINUE
```

These commands are exact aliases. Inside a child, either joins any descendants and returns
the child's Corpus to its parent. A child does not continue through the remaining root
pipeline.

After each child returns, the dispatcher sees a `FORK` history directive and correlated
`FORK_KNOWLEDGE`. These pending results are visible to the parent but do not enter the
starting Corpus of later siblings, so one review cannot anchor another.
Duplicate fork IDs from the same Worker are rejected.

When the dispatcher has every result, it can `SET` one consolidation while remaining in the
same Aspect. Its final `JOIN` (or identical alias `CONTINUE`) joins all completed child
Corpora and advances root directly to `integration`, where every specialist result is
ordinary durable knowledge.

### Integration can progress, route, or stop

The integration Worker consolidates the evidence with `SET`, sees that record on its rerun,
and chooses one transition:

```text
CONTINUE
JUMP requirements
STOP The request does not define idempotency retention or conflict behavior.
```

Use `CONTINUE` for normal progression. `JUMP <aspect-id>` is for a meaningful revisit, not
ordinary looping: Piper already reruns an Aspect after `SET`. Every configured Aspect is
presented as an `ASPECT <id>` target, and an unknown target fails the run.

A jump does not rewrite the Request or erase the Corpus. In this example, jumping to
`requirements` lets that Worker replace its earlier interpretation if the immutable request
supports a better one. It cannot invent missing product decisions. Repeated routing still
consumes the shared turn budget, so an unresolved cycle fails instead of running forever.

`STOP <reason>` cleanly terminates the entire run without a final Result. A child `STOP`
also propagates to the parent run. Use it for a known block, not as a substitute for an
ordinary negative readiness verdict.

`JUMP`, `STOP`, and `DONE` are rejected while the current Worker has unjoined fork work;
the Worker must use `CONTINUE` or `JOIN` first so completed analysis is not silently
discarded.

### The human gate and final result

At `approval`, Piper pauses and offers `Continue` or `Abort`. Gates are deterministic driver
checkpoints, do not call an LLM, and do not consume a turn. The current gate display presents
the immutable Request; arrange any additional review of the accumulated findings through
your surrounding operating process before approving. Aborting fails the run at that gate.

After approval, the final Worker returns exactly one command:

```text
DONE Ready with conditions: enforce authenticated key scope and transactional uniqueness; first run the concurrent duplicate-request integration test.
```

`DONE <result>` terminates immediately and supplies the final output printed by the CLI. It
does not add knowledge. Reserve it for the synthesis Aspect; earlier Workers should normally
use `SET` and a progression command.

## Why the budget is 24 turns

Every Worker pass consumes one turn across the root and every fork. Gates consume none.
`max_turns` is one shared run-wide limit, not a fresh allowance per branch.

An expected successful path here uses roughly:

- two requirements passes: `SET`, then `CONTINUE`;
- five dispatcher passes: three `FORK` commands, one consolidating `SET`, then `JOIN`;
- two passes in each of three children: `SET`, then child return;
- two integration passes: `SET`, then `CONTINUE`;
- one final `DONE` pass.

That is 16 Worker turns. A limit of 24 leaves room for a deliberate replacement or route
without making an accidental loop effectively unbounded. Tune the limit from the intended
command path, including every fork and every expected `SET` rerun.

If a first Worker response is malformed, Piper may make one formatter-recovery call. That
call belongs to the same Worker pass and does not consume an additional turn, but it uses
the shared `max_recoveries` budget. A malformed recovery response fails the run, and Piper
fails before any recovery call beyond the configured budget.

## Validate the pipeline before trusting it

Use deterministic fake Workers in automated tests. A useful test should prove data flow,
not merely that Piper accepted a sequence of commands:

- Give each specialist branch a unique marker that exists in neither the integration prompt
  nor the fake integration logic.
- Assert that the dispatcher sees completed `FORK` history and correlated
  `FORK_KNOWLEDGE`, while later siblings never receive earlier child markers.
- Assert that the integration prompt contains all markers as `KNOWLEDGE` after the join.
- Make the final fake derive an exact verdict from those received markers and assert the CLI
  output.
- Exercise both `JOIN` and `CONTINUE` in child and parent positions; their state transitions
  must be identical.
- Set an exact low `max_turns` in a loop fixture and assert that root and child calls exhaust
  the same budget.

Deterministic tests spend no model tokens and catch orchestration defects precisely. A real
model run is still valuable as a separate dogfood check for prompt quality, but it should not
be the only evidence that isolation, joining, or routing works.

## Common failure modes

- **Extra prose or Markdown from a Worker:** output must be exactly one non-empty DSL line.
  Piper attempts one recovery, then fails.
- **A branch loops in the dispatch Aspect:** child `CONTINUE` and `JOIN` return to the
  parent; ensure the child emits one after setting its contribution.
- **Joined findings are missing:** the parent sees only fork history until it emits
  `CONTINUE` or `JOIN`. Do not expect child knowledge during dispatch.
- **A duplicate branch fails:** fork IDs are unique per launching Worker. Use completed fork
  history to avoid launching the same ID twice.
- **The run ends with no final output:** reaching the end can leave only an expanded Corpus.
  Use a synthesis Aspect with `DONE` when the caller needs a Result.
- **The turn limit arrives early:** count `SET` reruns, dispatcher reruns, every child pass,
  routes, and the final synthesis against the single shared budget.

For the exact command grammar and state transitions, see
[`../spec/aop-engine.md`](../spec/aop-engine.md). For general Aspect ordering and design
principles, see [`../spec/pipeline-design.md`](../spec/pipeline-design.md).
