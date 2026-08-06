# Blueprint design

- **Status:** draft (target design; supersedes the retired `pipeline-design.md`)
- **Scope:** designing focused Station sequences within a Blueprint.

Terms are in [`glossary.md`](glossary.md); structure is in [`blueprint.md`](blueprint.md);
execution is in [`engine.md`](engine.md). This spec is the authoring guidance.

## Philosophy

A Blueprint decomposes a **Goal** into isolated Stations. A Worker preserves the Goal and
uses `set` to contribute durable Knowledge to the Corpus; later Stations consume that
expanded Corpus, so each concern is handled once and its findings stay available. Stations
also do general work (code, git, commands), not only text.

## Station ordering

Order Stations so dependent work follows the Knowledge it needs:

1. **Foundational** — correctness, logic, safety, factual accuracy.
2. **Structural** — architecture, specifications, organization, flow.
3. **Presentation** — voice, tone, brand alignment, polish.
4. **Synthesis** — produce a `done` Result when the Blueprint must return an answer rather
   than only an expanded Corpus.

## Design principles

- **Goal integrity** — a Worker never rewrites the Goal.
- **One contribution per Station** — `set` replaces the Worker's own contribution; keep it a
  concise consolidated statement, not arbitrary keys.
- **Corpus first** — a later Station consumes earlier findings rather than rediscovering them.
- **Normal progression** — `continue` joins completed child Forks and advances at the root,
  or returns the branch Corpus to the parent in a Fork.
- **Explicit routing** — use `jump` only for a meaningful revisit to another Station.
- **Forks are barriers** — use `fork` only for independent exploration; a child narrows the
  same Station, then `continue`s to return its isolated Corpus, which the parent joins
  before another Station starts.
- **Explicit termination** — `stop <reason>` for a clean blocked/abandoned Run; `done
  <result>` for a completed synthesis.
- **Bounded execution** — set `max_turns` high enough for intentional `set` loops and Forks,
  low enough to stop a malformed or indecisive Run; keep `max_recoveries` much lower.
- **Stable identity** — give a Blueprint a concise, durable name; it selects the Blueprint's
  active Run, so do not derive it from mutable contents or casually rename it.
- **Resumable gates** — a Gate is a durable pause, not an in-engine prompt. Place it only
  after all context a later human decision needs has been checkpointed. Each approval
  advances exactly one waiting Gate.

## Entrypoints

A Blueprint may expose more than one **entrypoint** — named Stations a Run can start at —
when the pipes genuinely share Stations and one design surface (full-task, quick-fix,
review-only over the same Station set). Give each entrypoint a distinct starting Station.
Pipes that share nothing belong in separate Blueprints, not extra entrypoints.

## Design workflow

1. **Goal** — state the desired outcome as the Run's one immutable Goal.
2. **Identity** — choose a durable Blueprint name meaningful across edits.
3. **Constraint extraction** — identify the durable facts and decisions later Stations need.
4. **Station design** — give each Station one concern; set its prompt, model, and toolset;
   order by dependency.
5. **Entrypoint design** — add an entrypoint only for a genuinely distinct start over shared
   Stations.
6. **Fork design** — add a Fork only for independent exploration; give each branch a distinct
   narrowing directive; have the child `continue` and the parent consolidate before advancing.
7. **Gate design** — place Gates after the durable analysis a reviewer needs; expect a
   separate approval per Gate in unattended execution.
8. **Termination** — add a synthesis Station only when a final Result is needed; otherwise a
   Blueprint may end after its Corpus is expanded.
