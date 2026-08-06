# Blueprint & data model

- **Status:** draft (target design)
- **Scope:** the Blueprint template, its Versions and Stations, and the Run instance model.

Terms are defined in [`glossary.md`](glossary.md). This spec fixes their structure and
relationships.

## Blueprint is a template

A **Blueprint** is a reusable process template, **repo-agnostic** — it names no working
copy. It is a canvas of **Stations** with one or more named **Entrypoints**. One Blueprint
is stamped into many **Runs**, which may run concurrently.

Blueprints hold multiple pipes on one canvas *only* to share Stations and a single design
surface — an "ace" Blueprint with `full-task`, `quick-fix`, and `review-only` entrypoints
reusing the same Station definitions. Fully independent pipes are separate Blueprints, not
extra entrypoints.

## Versions

A Blueprint has identity plus an ordered history of **Versions**. A Version is an
**immutable** snapshot of the whole design — Stations, toolsets, topology, entrypoints.
Editing a Blueprint produces a new Version; existing Versions never change.

A **Run pins the Version it launched from.** Editing a Blueprint therefore cannot mutate an
in-flight Run, and two concurrent Runs may execute different Versions.

## Station

A Station within a Version is:

- a **prompt**, **model settings**, and the **capabilities** it requires of the model
  (resolved against the Registry, see [`providers.md`](providers.md));
- a **toolset** — the work tools and control-flow tools the Worker may call
  (see [`engine.md`](engine.md));
- a **type** — `worker` (default) or `gate`;
- its place in the **topology** — ordering, `jump` targets, `fork` points; and
- whether it is an **entrypoint**.

## Run

A **Run** is one execution instance. It carries:

- a reference to its pinned **Version** and the **entrypoint** it started at;
- the run-wide **Goal** it pursues — its one immutable objective, broadcast to every
  Station firing (each firing's per-Station **Order** is assembled by the Driver, see
  [`engine.md`](engine.md));
- its own **work plane** — an isolated **Sandbox** (filesystem and process scope). Each Run
  has its own, which is what lets Runs of one Blueprint execute in parallel without
  collision. The Sandbox mechanism is defined in [`run-lifecycle.md`](run-lifecycle.md);
- its **Corpus** and shared execution budget; and
- durable execution state (position, frame stack, status).

The work plane binds **at the Run, never on the Blueprint** — forced by parallel Runs
needing parallel sandboxes.

## In-run local patch

A Run may be edited while live, within the limit in [`interface.md`](interface.md) (phase
1: tune an existing Station's prompt/settings; no add/remove/rewire). Such an edit is a
**Run-local patch**: the Run diverges from its pinned Version onto its own variant. The
Blueprint's canonical Versions are untouched.

A good patch reaches the template only through the **promote** bridge — the one sanctioned
crossing from a Run back into the Blueprint, producing a new Version. This preserves the
patch-vs-fix separation: in-run editing patches *this Run*; promotion, or design-mode
editing, fixes the *template*.

## Relationships

```text
Blueprint 1───* Version 1───* Station
Blueprint 1───* Run
Run *───1 Version (pinned)      Run 1───1 Goal       Run 1───1 Corpus      Run 1───1 work plane
Run  ─ optional local patch (diverged from its pinned Version)
```

The persistence store (PostgreSQL for structured state, an OCI registry for Sandbox
snapshots) and the Sandbox mechanism are fixed in
[`run-lifecycle.md`](run-lifecycle.md). The one open choice that still gates implementation
here — Version identity (item 7) — is tracked in [`OPEN-QUESTIONS.md`](OPEN-QUESTIONS.md),
not decided here.
