# Control API

- **Status:** draft (target design)
- **Scope:** the engine↔client contract — the operation set, its resource model, its
  transport, and the live event stream.

Terms are in [`glossary.md`](glossary.md).

## One API, three clients

The engine is headless ([`engine.md`](engine.md)): it advances a Run to one outcome —
waiting at a Gate, `done`, `stopped`, `failed`, or an interrupted call — and never prompts.
Every interactive surface drives it through this one API and none implements transitions:

- the **web UI** ([`interface.md`](interface.md)),
- the **Supervisor** ([`supervisor.md`](supervisor.md)), and
- the **CLI** — the implemented `start` / `continue` / `abort` / `status`
  ([`run-lifecycle.md`](run-lifecycle.md)) becomes a client of this same API.

The API is client-agnostic: it fixes the wire contract regardless of where a client's model
(if any) runs.

## The boundary holds over the wire

The API surface exposes **only Blueprint-plane and Run-control operations — never a
work-plane action**. There is no endpoint that writes a file, runs a command, makes a
commit, or emits Knowledge; those enter the work plane only through a fired Worker's
mediated tool call inside the engine ([`engine.md`](engine.md)). The Supervisor's "no work
tools" boundary ([`supervisor.md`](supervisor.md)) is therefore unrepresentable to cross
over the wire, not a guard checked after the fact.

## RESTful resource model

Routes are RESTful — resource nouns, HTTP verbs, status codes carrying the outcome. No
invented RPC routes.

| Operation | Surface |
|-----------|---------|
| list, inspect | `GET /blueprints`, `GET /blueprints/{id}`, `GET /runs`, `GET /runs/{id}`, `GET /runs/{id}/corpus` |
| edit a Blueprint → new Version | `POST /blueprints/{id}/versions` — the Version is immutable; the edit is the body |
| start a Run | `POST /runs` — body pins a Version, entrypoint, and Goal; advances to the first outcome |
| fork a Run | `POST /runs/{id}/forks` — creates a new Run |
| resume a Gate | `POST /runs/{id}/gate-decisions` — records approval of that exact Gate record |
| rewind a Run | `POST /runs/{id}/rewinds` — targets a checkpoint; appends, never destroys forward history |
| stop / abort a Run | a Run-control record under the Run; the terminal-transition resource shape is settled in build |
| subscribe to live events | `GET /runs/{id}/events` — Server-Sent Events; see Event stream below |

A command advances the headless engine to its next outcome; the response carries the Run's
new state. This generalizes the implemented CLI, where `start` and `continue` advance to a
Gate or terminal status.

## Transport

Commands are HTTP request/response. The live event stream is **Server-Sent Events** —
one-directional engine→client, riding plain HTTP with built-in reconnect. WebSocket is not
used: events flow one way and commands are already discrete POSTs, so bidirectional framing
adds cost without need.

## Event stream

The live stream is **per-Run and derived from the durable checkpoint log**
([`run-lifecycle.md`](run-lifecycle.md)) — not a second emission path. One event marks each
checkpointed boundary: run status, station-started / station-ended, tool-called and its
result, knowledge-set, gate-hit / gate-resolved, and fork-launched / fork-joined. Transport is
Server-Sent Events over `GET /runs/{id}/events`; each event's id is its checkpoint sequence, so
a client reconnecting with `Last-Event-ID` resumes from the log and loses nothing. There is no
cross-Run stream — the surface is per-Run only.
