# Engine

- **Status:** draft (target design; supersedes the retired `aop-engine.md` DSL protocol)
- **Scope:** the Driver, the Station execution loop, the Worker toolset, Corpus, and Forks.

The Driver owns orchestration in code; a Worker supplies judgment for one Station. This is
the anti-harness property of [`anti-harness.md`](anti-harness.md) — the control plane is
mechanical and inspectable, not re-enacted by a model from prose. What follows is the
target design; the current implementation is the single-line DSL described in git history,
to be migrated to the toolset protocol below.

## A Station is a mini-harness

A Station is a prompt + model settings + a **toolset**. Firing it produces a **Worker**:
the model is invoked in a tool-calling loop and *does the step's work*. "Work" is general —
write code, run commands, use git, reason — not only text. See [`blueprint.md`](blueprint.md)
for how Stations and toolsets are defined.

A Station's output is any mix of:

- **effects** in the work plane — files written, commits made, commands run; and
- **Knowledge** into the Corpus — the shared-context channel later Stations read.

A pure-effect Station emits no Knowledge at all. Knowledge is one channel, not the
definition of the work.

## The Station loop

The Worker runs as a tool-calling loop, not a single parsed line. Each Station:

1. The Driver invokes the model — through a **Provider** ([`providers.md`](providers.md)),
   the provider-neutral model adapter — with the Station's prompt, the Order, the current
   Corpus, and the Station's toolset.
2. The model calls tools. Two kinds:
   - **work tools** — read/write files, run commands, git, and whatever the Station is
     provisioned with. Executed in the Run's work plane (see [`run-lifecycle.md`]).
   - **control-flow tools** — the signals that were the old DSL, now tools the Driver
     applies to Run state (below).
3. The loop continues until the model calls a control-flow tool that ends the Station
   (`continue`, `jump`, `fork` resolution, `stop`, or `done`).

Every tool call is mediated and checkpointed by the Driver. That mediation is the
provenance guarantee: every work-plane effect traces to a fired Worker's tool call, and
the [`supervisor.md`](supervisor.md) control seat — which lacks the work tools — can never
produce one.

## Control-flow tools

These replace the single-line DSL. They are ordinary tools in the Worker's toolset; the
Driver validates and applies each as one defined state transition.

| Tool       | Arguments        | Effect                                                                 |
|------------|------------------|------------------------------------------------------------------------|
| `set`      | `<knowledge>`    | Creates or replaces the current Worker's own Knowledge contribution in its branch. |
| `continue` | none             | Joins any completed child Forks, then advances to the next Station (root) or returns the branch Corpus to the parent (fork). |
| `jump`     | `<station-id>`   | Transfers control to another Station. Invalid while the Worker has unjoined Forks. |
| `fork`     | `<id> <directive>` | Runs an isolated child on the current Station with a copy of the Corpus and a narrowing directive. |
| `stop`     | `<reason>`       | Ends the whole Run cleanly with no Result. A child `stop` stops its parent Run too. |
| `done`     | `<result>`       | Returns a final Result. Invalid while the Worker has unjoined Forks.    |

The Driver rejects invalid transitions: unknown targets, duplicate Fork ids from one
Worker, and `jump`/`stop`/`done` that would discard unjoined Fork work.

## Corpus and Forks

- **Corpus** is per-Run accumulated Knowledge, fed forward to later Stations. Contributions
  retain their branch and Station identity internally; Workers do not address entries by key.
- **Fork** copies the parent's starting Corpus into an isolated child running on the same
  Station under a narrowing directive. The child may `set` repeatedly, then `continue` to
  return its branch Corpus to the parent as *pending* Knowledge. The parent sees each
  completed Fork's directive and pending Knowledge, may consolidate with `set` or spawn
  more siblings (each from the unchanged parent Corpus), and eventually `continue`s to join
  all children and advance. Forks are semantic isolation, not parallelism; joins are
  mechanical.

## Bounds

The root Run and every Fork share one budget: a `max_turns` limit on Worker passes
(prevents unbounded jump/fork loops) and a `max_recoveries` limit on malformed-output
repairs. Gates consume neither. Reaching a limit fails the Run before another pass.

Tool-calling reduces but does not eliminate malformed output; a Station that fails to
produce a valid control-flow transition within budget fails the Run. Recovery is bounded
so repair never becomes the normal path.

## Headless

The engine is headless: it advances until it produces one outcome — waiting at a Gate,
`done`, `stopped`, `failed`, or an interrupted call — and never prompts. Attended and
interactive surfaces (CLI, the Supervisor) drive it through the control API
([`control-api.md`](control-api.md)); they choose when to advance and how to present a
pause, but they never implement transitions. Durable
identity, checkpointing, the work plane, and Gate resumption are in
[`run-lifecycle.md`](run-lifecycle.md).
