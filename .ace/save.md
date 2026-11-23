# Session trail — Piper

## Now

The OPEN-QUESTIONS **blocking spine is empty** — all five shared runtime contracts settled
and written into `docs/spec/`, committed as `d30bb03` (not pushed). Strict 1-by-1 with the
user; each ruling written into the spec it governs and struck from the queue:

- **Sandbox** work plane behind a provision/teardown contract; phase-1 backend the **Dagger**
  Rust SDK; Fork = copy-on-write branch; a checkpoint pins an OCI digest for durable resume.
- **Persistence:** PostgreSQL for structured state + an **OCI registry** for Sandbox
  snapshots (digest-referenced).
- **Control API:** one RESTful HTTP contract, three clients (web, Supervisor, CLI); no
  work-plane endpoint, so the Supervisor boundary holds over the wire.
- **Station loop:** a provider-neutral **Model port** over the **OpenAI-compatible Chat
  Completions** surface + a **capability map** fed by a **refreshable external index**
  (models.dev), so new models arrive without a rebuild. New spec `providers.md`.
- **Live event stream:** per-Run **SSE** derived from the durable checkpoint log, with
  `Last-Event-ID` replay. **Cross-run support dropped entirely** (user: streamline first).

**#6 (Order shape) settled and written** — commit `94766fd` (not pushed). The Goal/Order
split, now in the specs: **Goal** = one immutable per-Run objective, broadcast unchanged to
every Station firing (this is what code's `request` becomes); **Order** = the per-firing
bundle the Driver assembles and hands one Worker (the Goal, the current Corpus, and the
Station's own parameters — prompt, model settings, toolset, work plane), never free
model-generated prose passed Station-to-Station; **Corpus** = Knowledge built down the
production line. Reconciled across `glossary`, `engine`, `blueprint`, `blueprint-design`,
`control-api`, `interface`, `dogfooding`, and the `anti-harness` term note.

**Working protocol (now in `OPEN-QUESTIONS.md`):** one queue item at a time, in order —
discuss at length, ratify the conclusion into the spec it names, `ace-save`, take the next.
No code and no build until the queue is empty and the specs read pristine.

Next: **seven design decisions remain, #7–14** in `OPEN-QUESTIONS.md` — Version identity
(#7, **open now** — my proposal on the table, awaiting the user's read; see ledger),
in-run patch/promote, multi-entrypoint scope, Supervisor hosting, frontend tech,
gate-over-web, served-UI auth, migration plan. Queue empty → development fan-out.

The prior phase-1 "harden the engine" work (durable run lifecycle, forks, gates, recovery)
is implemented and green; it is now the foundation the reframed specs build on, not the
frontier.

## Standing facts

- **Long trail is expected right now** — heavy architect/design phase with many open issues
  (the #7–14 queue). Don't trim `save.md`/`save.ledger.md` for size; items graduate as
  decisions land in `docs/spec/`, not before.

- First goal: self-dogfooding — Piper develops Piper; Claude is the Worker engine
  (`docs/spec/dogfooding.md`).
- `docs/spec/` is authoritative and vocabulary-aligned via `docs/spec/glossary.md`:
  Blueprint / Version / Station / Worker / Run / Order / Knowledge / Corpus / Driver /
  Provider / Registry / Sandbox / Gate / Fork / Supervisor. "world" = visual layer,
  "plane" = capability domain; **"Operator" is not a term** (the seat is the Supervisor).
  **Driver** = the Run sequencer; **Provider** = the model-facing adapter (do not conflate).
- **Piper is provider / harness / model-agnostic by law.** Reuse standards and external
  indexes over hand-rolling; the engine names no vendor. Claude is one configured backend
  for dogfooding, reached through the neutral Provider port — never coupled in the contract.
- **Cross-run support is dropped** — the run event/attention surface is per-Run only until a
  deliberate streamline pass. Fleet-scale UI was removed from the parked list.
- The reframe specs are status `draft` (target design, not yet built); the current code
  still runs the single-line DSL and one-shot `claude -p`. Spec deliberately outruns code;
  the migration is OPEN-QUESTIONS #14.
- Verify before completing code: `cargo fmt --check`; `cargo clippy --all-targets
  --all-features -- -D warnings`; `cargo test`. Crate is `#![deny(warnings)]`.
- Tests use deterministic fake Workers (shell-backed via `PIPER_LLM_INVOCATION`); no live
  model calls in the suite. Durable run state lives under XDG state, not project-local.

## Pointers

- `docs/spec/OPEN-QUESTIONS.md` — the decision queue that gates development. The morning read.
- `docs/spec/README.md` — the full spec index and one-line summaries of every spec.
- `docs/spec/providers.md`, `docs/spec/control-api.md` — the two specs added this session
  (provider/registry layer; engine↔client contract + event stream).
- `docs/scratch/2026-08-06-supervisor-future-ideas.md` — parked ideas (cross-run eval as an
  eval edge; the meta-pipe developing Blueprints through Piper).
- `.ace/afk.log` — historical: the engine-hardening AFK handoff, commit-by-commit.
