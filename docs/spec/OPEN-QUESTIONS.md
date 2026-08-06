# Open questions — the decision queue

- **Status:** queue (worked down to empty before development begins)
- **Scope:** every design decision still open across the specs. Development is not
  kickstarted until this queue is empty.

Each item is a decision to make, not a task to build. When one is settled, write the rule
into the spec it names and strike it here. The specs are the build target; this file is
what stands between them and a swarm.

## Working protocol

Worked one item at a time, in order, to completion — no code and no build until this queue
is empty and the specs read pristine. Each item: discuss it at length, arrive at a
conclusion, ratify that conclusion into the spec it names, `ace-save`, then take the next
item. Nothing is half-decided and left; a settled item is written in and struck here in the
same pass.

## Blocking — shared contracts every builder depends on

All five runtime-spine contracts are settled and written into the specs: Sandbox and
persistence store ([`run-lifecycle.md`](run-lifecycle.md)), control API and event stream
([`control-api.md`](control-api.md)), and the provider / registry layer
([`providers.md`](providers.md)).

## Data-model & design decisions

7. **Version identity & storage** — how a Version is identified, stored, and diffed; the
   edit→new-Version mechanics. ([`blueprint.md`](blueprint.md))
8. **In-run local patch** — how a Run-local divergence is represented and diffed against its
   pinned Version, and the exact **promote** mechanics back to a new Version.
   ([`blueprint.md`](blueprint.md), [`interface.md`](interface.md))
9. **Multi-entrypoint scope** — is it in phase 1? Where is the line between one Blueprint
   with many entrypoints and several Blueprints? How are entrypoints declared?
   ([`blueprint.md`](blueprint.md))
10. **Supervisor hosting** — is the Supervisor's model hosted in the engine process, or an
    external agent holding the Supervisor toolset over the control API? Define the concrete
    Supervisor toolset. ([`supervisor.md`](supervisor.md))
11. **Frontend tech** — server-side Rust rendering (htmx / maud) or Rust→WASM (Leptos /
    Dioxus)? TypeScript is banned. ([`interface.md`](interface.md))
12. **Gate-over-web flow** — how a paused Run surfaces in the UI and resumes through the
    control API. Per-Run only; cross-Run attention is dropped. ([`interface.md`](interface.md))
13. **Auth for the served UI** — none, or a per-process token, for a tailnet-reachable local
    daemon? ([`interface.md`](interface.md))
14. **Migration plan** — what current code is kept (headless engine, run lifecycle, frame
    stack) vs rebuilt (DSL parser → control-flow tools, `claude -p` → tool loop, TOML
    config → the store). Resolve the known child-`done` contradiction (child returns a
    Corpus to its parent while `done` is specified to terminate the whole Run) as part of
    this.

## Parked — decide later, not blocking

- **Cross-run eval debugging** and the **meta-pipe** — see
  [`../scratch/2026-08-06-supervisor-future-ideas.md`].
- **SaaS deployment** — hosting the engine and repos remotely; a different product.
