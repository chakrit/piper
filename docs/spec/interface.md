# Interface

- **Status:** draft (target design)
- **Scope:** how a human sees and operates Piper — the world model, the floating chrome,
  and the design/run separation.

Terms are in [`glossary.md`](glossary.md). The interface presents the Supervisor seat
([`supervisor.md`](supervisor.md)); the boundary there is medium-agnostic, so this spec
fixes the presentation, not the capability rules.

## Two layers

The interface is a **floating layer** over a **world layer**.

- **Floating layer** — persistent instruments that hover above and never reload: the nav
  (itself a floating, movable window, not a docked rail), the Supervisor summon, the
  Run/Stop control, run cards. Movable and collapsible.
- **World layer** — exactly **one world** beneath the chrome. Navigating slides the world
  out and the next in; the instruments stay put and re-scope to whatever world is beneath.

## Two worlds: design mode and run mode

A world is either a Blueprint in **design mode** or a Run in **run mode**. The separation
is deliberate and load-bearing:

- **Design mode** — editing the Blueprint canvas: add/remove/rewire Stations, edit prompts,
  settings, toolsets, topology, entrypoints. This is Blueprint-plane work on the *current*
  Version.
- **Run mode** — a Run, live or replayed. The canvas shows the Run's **pinned Version**,
  animated as material flows through it. Live and replayed are the same durable event stream
  ([`control-api.md`](control-api.md)) — tailed or replayed. Here you *steer*, you do not
  restructure.

The mode is the plane and the Version rule made visible: design mode edits the template;
run mode shows an immutable pinned Version, so "you cannot edit a live Run's Blueprint" is
not a guard — editing simply is not part of run-world.

Run mode patches the instance; design mode fixes the template. Steering (rewind, redo a
Station with a nudge) fixes *this Run*; editing a Station's prompt fixes the *factory*. The
one sanctioned crossing back is **promote** ([`blueprint.md`](blueprint.md)).

## The core loop

The heartbeat: on a Blueprint canvas, press **Run**, watch material flow through the
Stations live, press **Stop** to intervene. `[Run]` opens a launch that binds the work
plane (sandbox) and the Goal, then slides into a new Run world. One direct control;
everything else orbits it.

## The Supervisor

The Supervisor is **summoned**, not a constant rail — a center CTA in the control bar. It
is **context-scoped**: with a Blueprint world beneath, it edits the Blueprint; with a Run
world beneath, it steers that Run. It recedes as a Blueprint matures — heavy while a
Blueprint is young, break-glass once it is solid and Runs are stamped out at scale.

## In-run editing limit (phase 1)

Run mode allows **tuning existing Stations** — edit a Station's prompt/settings, rewind,
re-fire — as a Run-local patch. It does **not** allow add/remove/rewire; structure stays
design-mode only. Factory-intuitive: adjust a machine already on the line; do not bolt a
new machine onto a running one.

## Gates in the interface

A pending Gate is a **spatial object** on the canvas, not an inbox: its Station pulses and
floats a "needs you" chip where the Run sits, and a **fly-to-next** control sweeps between the
Run's pending Gates. (A Gate is also a durable per-Run decision record — see
[`run-lifecycle.md`](run-lifecycle.md).)

## Deployment

A **local-served web UI**: the Piper daemon serves the frontend, reachable over the
tailnet, Workers executing where the code is. Not SaaS — hosting the engine would mean
hosting the user's repos, a different product, deferred. The frontend is **Rust-native**
(TypeScript is banned); server-side rendering or Rust→WASM, kept single-language and
self-contained. It is one more frontend over the headless engine + control API, not a
separate authority.
