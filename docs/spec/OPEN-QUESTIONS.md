# Open questions — the decision queue

- **Status:** queue (worked down to empty before development begins)
- **Scope:** every design decision still open across the specs. Development is not
  kickstarted until this queue is empty.

Each item is a decision to make, not a task to build. When one is settled, write the rule
into the spec it names and strike it here. The specs are the build target; this file is
what stands between them and a swarm.

## Blocking — shared contracts every builder depends on

These five are the runtime spine. They are cross-cutting: multiple builders code against
them, so they must be decided before any fan-out.

1. **Sandbox / work-plane mechanism** — how each Run gets its isolated checkout, filesystem,
   and git. Container? git worktree? VM? How is the repo provisioned in, and torn down?
   Governs [`blueprint.md`](blueprint.md), [`run-lifecycle.md`](run-lifecycle.md).
2. **Persistence / data store** — where Blueprints, Versions, Runs, Corpora, and in-run
   patches live. DB, files, or an evolution of the implemented XDG run storage? How does it
   reconcile with the current per-pipeline XDG layout in [`run-lifecycle.md`]? Everything
   reads and writes this.
3. **Control API** — the engine↔Supervisor↔web contract: the operation set (start / stop /
   rewind / fork / edit-Blueprint / list / subscribe) and its transport (HTTP+SSE?
   WebSocket?). Both the web frontend and the Supervisor are clients; nail it before either.
4. **Station tool-call loop** — how the Worker's model loop is driven (Claude API vs Agent
   SDK), exactly when a Station ends, how work-tool and control-flow-tool calls interleave,
   and how recovery works under tool-calling. The execution core of [`engine.md`](engine.md);
   it retires one-shot `claude -p` in `llm.rs`.
5. **Live event stream** — the engine→UI eventing (station-started, tool-called,
   knowledge-set, gate-hit) and its transport. The live run world in [`interface.md`]
   depends on it; the engine is batch advance/resume today.

## Data-model & design decisions

6. **Order shape** — free text, or structured/parameterized template variables per Blueprint?
   ([`blueprint.md`](blueprint.md))
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
12. **Gate-over-web flow** — how a paused Run surfaces and resumes through the control API,
    and the cross-Run attention presentation once there are many pending Gates (fleet-scale
    UI is otherwise deferred). ([`interface.md`](interface.md), [`run-lifecycle.md`])
13. **Auth for the served UI** — none, or a per-process token, for a tailnet-reachable local
    daemon? ([`interface.md`](interface.md))
14. **Migration plan** — what current code is kept (headless engine, run lifecycle, frame
    stack) vs rebuilt (DSL parser → control-flow tools, `claude -p` → tool loop, TOML
    config → the store). Resolve the known child-`done` contradiction (child returns a
    Corpus to its parent while `done` is specified to terminate the whole Run) as part of
    this.

## Parked — decide later, not blocking

- **Fleet-scale UI** — presenting 20+ concurrent Runs; the calm heatmap / aggregate view.
- **Cross-run eval debugging** and the **meta-pipe** — see
  [`../scratch/2026-08-06-supervisor-future-ideas.md`].
- **SaaS deployment** — hosting the engine and repos remotely; a different product.
