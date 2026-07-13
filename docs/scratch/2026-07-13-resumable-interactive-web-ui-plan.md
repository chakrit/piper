<!-- not spec/decision because: this is a proposed multi-feature implementation plan whose command and storage surfaces are not yet accepted -->

# Resumable execution, interactive gates, and web UI plan

## Outcome

Build one persistent Piper execution core and expose it through three operator experiences:

1. `piper start <pipeline-file> <request>` creates a durable run and advances mechanically
   until a gate or terminal outcome.
2. `piper continue` approves the active gate and advances the same durable run until the
   next gate or terminal outcome.
3. `piper run <pipeline-file> <request>` remains the attended mode: it uses the same durable
   engine but presents terminal prompts at gates and continues until completion or abort.
4. `piper ui` serves a local browser control plane for starting runs, inspecting exact
   Worker inputs/outputs and Corpus state, and clicking through gates.

The state machine remains authoritative. Terminal and web interfaces may choose when to
invoke it and how to present a pause, but they do not implement pipeline transitions.

## Decisions to carry into accepted specs

### One active run, durable history

- Store run data under project-local `.piper/`, which must be gitignored because Requests,
  Corpus entries, prompts, and model outputs may be sensitive.
- Store immutable and mutable state in `.piper/runs/<run-id>.json`.
- Store the selected run ID in `.piper/active` using an atomic replace.
- `piper start` fails if the active run is non-terminal. It never silently replaces work.
- A new run may start after the prior run reaches `done`, `stopped`, `aborted`, or `failed`;
  prior JSON files remain inspectable.
- `piper continue` uses the active run by default. A future `--run <id>` can select history,
  but is not required for the first slice.
- Add read-only `piper status` and explicit `piper abort` commands because persistent state
  otherwise has no safe inspection or escape path.

### Serializable execution stack

Replace recursive `Driver::run_inner` calls with an explicit stack:

```text
RunState
├── schema_version
├── run_id, timestamps, status
├── config_path, pipeline_path, prompt_dir
├── immutable Request and resolved Aspect snapshot
├── max_turns, max_recoveries, consumed counters
├── execution_frames[]
├── terminal Result/STOP/error, if any
└── turn_records[]

ExecutionFrame
├── current Aspect index
├── fork ID/instruction and branch path
├── branch Corpus
└── completed pending forks and their Corpus deltas
```

- The top frame is the currently executing root or fork.
- `FORK` pushes a same-Aspect child frame.
- Child `CONTINUE`/`JOIN` pops the frame and attaches its Corpus delta to its parent.
- Root `CONTINUE`/`JOIN` joins pending deltas and advances.
- This removes the process call stack as hidden engine state, making nested forks resumable.
- Snapshot resolved Aspects and budgets at start so editing a pipeline file cannot mutate an
  in-progress run.
- Store configuration and prompt paths, not interpolated secrets. Each invocation reloads
  the configured LLM command; document that local model configuration may change between
  continuations.

### Atomic checkpoints and paid-call boundaries

- Route all run-state reads and writes through dedicated actions under `src/actions/`.
- Save atomically with a sibling temporary file, flush, then rename.
- Checkpoint after every accepted DSL transition and every gate decision.
- Before a model call, persist a turn record with the rendered prompt and `calling` status.
- Immediately after receiving output, persist the raw output before parsing or applying it.
- Persist recovery prompts and raw recovery outputs the same way.
- On resume, apply a stored response without calling the model again.
- If the process dies while a provider call is in flight, Piper cannot know whether the
  provider charged or completed it. Mark the turn `interrupted`; require an explicit retry
  in attended/web modes and fail with a recovery hint in unattended `continue`.
- Keep the existing shared turn and recovery counters in durable state.

### Pause and terminal states

Use a typed status rather than booleans or magic strings:

```text
ready | running | waiting_at_gate | interrupted_call
done | stopped | aborted | failed
```

- The engine advances until it produces one `AdvanceOutcome`: `WaitingAtGate`, `Done`,
  `Stopped`, `Failed`, or `InterruptedCall`.
- A gate never prompts inside the engine. It returns `WaitingAtGate` with Aspect ID/name,
  Request, current Corpus, and branch context.
- `piper continue` is an explicit approval of the active `waiting_at_gate` state.
- `piper abort` records an operator abort without consuming a Worker turn.
- `STOP` remains a model-directed clean terminal outcome distinct from operator abort and
  engine failure.

## Phase 1: persistent `start` / `continue`

### Specifications first

Update:

- `docs/spec/aop-engine.md`: serializable stack, checkpoint boundaries, pause/terminal
  states, one-active-run storage, and continuation semantics.
- `docs/spec/pipeline-design.md`: gate placement and resumability implications.
- Add an accepted CLI/storage spec only after reviewing the proposed surface above.
- Update README and human guides in the same behavior commit.

### Tests first

Add deterministic pipeline fixtures and black-box tests for:

1. `start` reaches a gate, exits successfully, and writes an inspectable active state.
2. `continue` approves that exact gate, preserves Corpus, and reaches `DONE`.
3. Two gates require two separate `continue` invocations.
4. A nested fork pauses/resumes without losing its execution frame or sibling isolation.
5. Turn and recovery counters survive process boundaries.
6. A stored raw model response is applied on resume without another fake-model call.
7. An interrupted in-flight call is not silently replayed.
8. Corrupt, unknown-version, truncated, or path-mismatched state fails closed with a useful
   recovery message.
9. `start` refuses to overwrite a non-terminal active run.
10. `status` is read-only and `abort` makes the active run terminal.
11. Atomic-save failure leaves the last valid checkpoint loadable.

### Implementation slices

1. Add serializable state DTOs and schema-version validation without changing execution.
2. Add `LoadRunState`, `SaveRunState`, and active-pointer actions with isolated temp-dir
   tests; add `.piper/` to `.gitignore`.
3. Replace recursive forks with the explicit frame stack under existing in-memory tests.
4. Change the driver to one-transition/advance-until-pause operations with no terminal IO.
5. Persist after each transition and raw response; add interrupted-call handling.
6. Add CLI `start`, `continue`, `status`, and `abort` composition.
7. Rebuild existing `run` on the same engine as the attended wrapper; delete the old direct
   recursive path rather than retaining parallel implementations.

Likely files:

- `src/state.rs`: durable run/status/frame/turn types.
- `src/driver.rs`: headless transition engine and `AdvanceOutcome`.
- `src/actions/load_run_state.rs`, `save_run_state.rs`, `set_active_run.rs`.
- `src/main.rs` or a new focused `src/commands/` directory once there is a second real
  command consumer.
- `src/models.rs`: only if shared serialized enums belong outside state.
- `tests/cli.rs` and new `tests/pipelines/resume-*.toml` fixtures.

## Phase 2: attended terminal gates

`piper run` becomes the human-attended loop over the persistent engine:

1. Start and advance.
2. On `WaitingAtGate`, render a compact gate screen.
3. Let the operator select `Continue` or `Abort` using keyboard navigation and Enter.
4. Save the decision before advancing.
5. Repeat until a terminal outcome.

Terminal UI requirements:

- Show gate Aspect name/ID and `CONTEXT` path.
- Show the immutable Request, accumulated Corpus, pending fork results, turns used/limit,
  and recoveries used/limit.
- Keep `Continue` as the single primary action and visually separate `Abort`.
- On Ctrl+C or prompt interruption, leave the run at the gate; never convert interruption
  into approval or abort.
- The TUI is keyboard-operated. Literal mouse-click buttons belong to the web UI.
- Remove prompting from `Driver`; terminal IO belongs only to the attended command.

Tests:

- Record a gate view model independently of terminal rendering.
- Fake `Continue`, `Abort`, and interruption decisions.
- Assert gates consume neither turn nor recovery budget.
- Assert rerunning attended mode attaches to the active waiting run rather than duplicating
  it.

Dependency note: decide whether to keep `dialoguer` or adopt ACE's `inquire` only after the
required safety audit. Do not keep both prompt libraries.

## Phase 3: `piper ui` local web control plane

### Architecture copied from Ticket Hamster

Reuse the proven shape from `/Users/chakrit/Documents/chakrit/ticket-hamster`:

- `piper ui --host 127.0.0.1 --port 7331` starts a raw `hyper` HTTP/1 server on a dedicated
  Tokio runtime and blocks.
- Default to localhost. A wide bind requires an explicit `--host 0.0.0.0`.
- Best-effort open `http://localhost:<port>` after the listener starts; support
  `PIPER_NO_OPEN=1` for headless use.
- Serve a Parcel-built vanilla HTML/CSS/JS site from committed `websites/console/dist/`,
  embedded with `include_dir`, so release binaries need no Node installation.
- Keep routing as a pure `resolve(method, path) -> Route` function with unit tests.
- Use one single-slot subprocess supervisor. It spawns this binary's `start`, `continue`,
  or `abort` commands, captures stdout/stderr, and broadcasts tagged events over SSE.
- Read structured status from the durable run-state file; the subprocess log is an
  observable stream, not the state authority.
- Inject a fresh per-process API token into served HTML and require it on `/api/*`; use a
  query token for `EventSource`, which cannot set headers.
- Do not add accounts, remote storage, WebSockets, or multi-run concurrency.

### Initial API

```text
GET  /health
GET  /api/pipelines
GET  /api/runs/active
POST /api/runs/start       { pipeline, request }
POST /api/runs/continue
POST /api/runs/abort
GET  /api/runs/stream      SSE: log, state, gate, terminal
GET  /*                    embedded static console
```

- `/api/pipelines` lists validated pipeline TOML files under the server working directory;
  the browser does not receive an arbitrary filesystem-reading endpoint.
- Mutating endpoints return `409` when another subprocess is active or state does not allow
  the requested transition.
- SSE has a bounded broadcast backlog. `GET /api/runs/active` restores truth after reload.
- The run state and exact turn records supply prompt/output inspection after a browser
  reconnect; SSE alone is not history.

### Browser information architecture

Desktop:

```text
┌ Piper · active run/status · budgets ─────────────────────────────┐
├ Pipeline + Request ┬ Aspect timeline ┬ Turn inspector             │
│ Start/Abort        │ root/fork paths  │ exact prompt / raw output  │
│                    │ Corpus changes   │ recovery + parsed action   │
├────────────────────┴──────────────────┴────────────────────────────┤
│ Gate bar: context, Continue, Abort (only while waiting)           │
└────────────────────────────────────────────────────────────────────┘
```

- At mobile widths, stack controls, timeline, and inspector vertically; keep the gate action
  reachable without horizontal scrolling.
- Selecting a turn deep-links with a URL fragment or query so refresh preserves inspection.
- Exact Worker input/output is shown in escaped `<pre>` blocks with copy buttons.
- Corpus and fork deltas show source context paths; do not rely on color alone.
- Gate buttons are at least 44px high, keyboard reachable, and have visible focus states.
- Use `aria-live` for state transitions and terminal outcomes.
- Respect `prefers-reduced-motion`; no decorative animation or auto-scrolling when the user
  has scrolled up.

### Simple Piper visual style

The UI design query recommended a dark code-tool palette with a run-green accent and
monospace typography. Adapt that into a restrained **signal console**, not Ticket Hamster's
warm/editorial identity and not a generic card dashboard:

```css
:root {
  --canvas: #0b0f14;
  --surface: #111821;
  --surface-raised: #17212c;
  --border: #2a3745;
  --text: #edf3f8;
  --text-muted: #9cacba;
  --signal: #45d483;       /* active, continue, successful transition */
  --gate: #f2b84b;         /* waiting for human authority */
  --danger: #f06a6a;
  --focus: #7dd3fc;
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-6: 24px;
  --radius: 6px;
  --font-ui: system-ui, -apple-system, "Segoe UI", sans-serif;
  --font-mono: ui-monospace, "SFMono-Regular", Menlo, monospace;
}
```

- Dark-first for the initial console; no theme toggle in MVP.
- System fonts only: no CDN and no font asset dependency.
- Hairline borders, nearly flat surfaces, one tight radius, and no decorative gradients,
  glass, glow, mascot, or card grid.
- Monospace is reserved for context paths, counters, prompts, outputs, and DSL actions; UI
  labels use the system sans stack.
- Signal green means active/continue/success, amber always includes a `Waiting at gate`
  label, and red always includes failure/abort text.
- Primary controls meet 4.5:1 contrast and 44px touch targets. Focus uses a 2px visible
  cyan outline rather than color-only background changes.

The UI skill's single-column landing recommendation is intentionally not used: Piper is an
operator workbench with three related inspection regions. Its dark/run-green/monospace
recommendations are retained because they fit a local pipeline console.

### Frontend files

Mirror Ticket Hamster's self-contained structure:

```text
websites/console/
├── package.json, pnpm-lock.yaml
├── src/index.html
├── src/styles.css
├── src/api.js
├── src/app.js
└── dist/                 # committed generated assets
scripts/build-console.sh  # Parcel build + orphan bundle pruning
```

Prefer one page and a few plain modules. Do not introduce React, TypeScript, Tailwind, a
component library, or client-side routing for this surface.

### Web tests

- Pure route resolution and content-type/static-path behavior.
- API-token header/query authorization and rejection.
- Start/continue/abort state conflicts and one-subprocess-at-a-time behavior.
- SSE serialization for log/state/gate/terminal messages and reconnect via status.
- Subprocess stdout/stderr streaming with a deterministic fake binary mode.
- Static asset embedding and `/` fallback.
- Frontend unit tests for ANSI escaping/rendering, state reducer, gate button enablement,
  and API errors where a lightweight no-browser runner suffices.
- Manual keyboard/mobile/contrast pass at 375, 768, 1024, and 1440px.

## Dependency and safety gates

No dependency is added during planning. Before implementation:

1. Finish the pending safety audit for `console 0.16.2` used by the logging slice.
2. Audit and pin serialization/prompt changes (`serde_json`; `inquire` only if replacing
   `dialoguer`).
3. Audit the direct Rust web closure copied from Ticket Hamster: `tokio`, `hyper`,
   `hyper-util`, `http-body-util`, `tokio-stream`, and `include_dir`.
4. Audit Parcel/pnpm and its lockfile before installing or running it. Commit exact lockfile
   and generated `dist/`; normal Rust builds must not invoke Node or install scripts.
5. Measure clean and incremental Rust compile time before/after the web dependencies. If the
   copied stack is too expensive, preserve the API/UI design but reconsider the server
   implementation before coding further.

## Recommended implementation order next session

1. Review and accept the proposed CLI/storage/status contract.
2. Implement serializable state and atomic actions with tests.
3. Replace recursive execution with explicit frames.
4. Add `start`, `continue`, `status`, and `abort`.
5. Rebuild attended `run` and TUI gates on the persistent engine.
6. Finish correlated logging before the web layer so CLI and SSE share readable events.
7. Add the raw-hyper server and subprocess supervisor.
8. Build and embed the single-page console.
9. Run the complete Rust, CLI-pipeline, shell, frontend, accessibility, and manual browser
   verification gates.

Each numbered item is a separate test-first commit. Do not combine the state-machine
refactor, terminal UI, server, and frontend into one diff.
