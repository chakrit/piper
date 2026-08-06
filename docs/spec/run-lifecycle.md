# Durable Run Lifecycle Specification

- **Status:** accepted (implemented); target additions below are draft
- **Scope:** Persistent run identity, storage, selection, checkpointing, and CLI lifecycle.

> Terminology follows [`glossary.md`](glossary.md). The implemented sections below predate
> the Blueprint/Version model and use "pipeline" for what is now a **Blueprint** and
> "Aspect" for a **Station**; the durable mechanics they describe are unchanged. How the
> storage layout evolves for multiple Blueprints, Versions, and a web store is an open
> decision — see [`OPEN-QUESTIONS.md`](OPEN-QUESTIONS.md).

## Identity and storage

Piper stores restartable state outside the project under the XDG state directory:

```text
${XDG_STATE_HOME:-$HOME/.local/state}/piper/
└── projects/
    └── <project-hash>/
        ├── project.json
        └── pipelines/
            └── <pipeline-id>/
                ├── active
                ├── operation.lock
                └── runs/
                    └── <run-ulid>.json
```

An empty or unset `XDG_STATE_HOME` uses `$HOME/.local/state`. A relative
`XDG_STATE_HOME` is invalid and is ignored in favor of that fallback. The project hash is
the lowercase hexadecimal SHA-256 digest of the canonical project root. The project root
is the parent directory of the selected shared configuration file. When no `--file` is
given, Piper searches the current directory and its ancestors for the nearest
`piper.toml`. `project.json` stores the canonical root and configuration path, and Piper
validates both before trusting the namespace.

Every pipeline declares a required, human-readable `[pipeline].id`. The ID uses Piper's
identifier grammar, `[A-Za-z0-9_-]+`, and names that pipeline's state directory. Every
execution receives a ULID. Full ULIDs are persisted; displays use a short prefix,
lengthened when necessary to remain unique among matching runs. Command arguments may use
any unique prefix. An ambiguous or unknown prefix is rejected rather than guessed.

Each pipeline has its own `active` pointer containing one full ULID. It remains on a
terminal run for inspection until a later `start` selects a new run. Historical run
snapshots are not deleted when the pointer changes.

On Unix, Piper creates state directories with mode `0700` and state, pointer, metadata,
and lock files with mode `0600`.

## Durable snapshot

A run snapshot is schema-versioned and contains enough information to resume without
reconstructing execution from the current source tree:

- the run ULID, pipeline ID, canonical project/config/pipeline/prompt paths, immutable
  Request, and resolved Aspect definitions;
- the current Corpus, shared turn and recovery budgets, and their consumed counts;
- a serializable execution-frame stack for the root and nested forks, including current
  Aspect positions, branch paths, contributions, pending child Corpora, and fork history;
- each rendered prompt, call phase, exact raw model output when received, and accepted DSL
  transition; and
- the current run status and terminal result, reason, or error when applicable.

The frame stack is the durable execution stack. A process restart must not depend on a
Rust call stack to recover nested forks.

Run statuses are:

```text
ready | running | waiting_at_gate | interrupted_call
done | stopped | aborted | failed
```

The final four statuses are terminal. `STOP` produces `stopped`, `DONE` produces `done`,
an operator action produces `aborted`, and an execution error produces `failed`.

## Checkpoints and crash behavior

Piper checkpoints before each model call with that turn marked `calling`. After the call
returns, it checkpoints the exact raw output before parsing or applying it. It then
checkpoints every accepted DSL transition and every gate decision. A stored raw response
is always parsed and applied before Piper considers another model call.

If Piper opens a snapshot whose latest call is still `calling` and has no stored response,
it changes the observable status to `interrupted_call`. It never silently repeats that
possibly completed external call. The operator must resolve or abort the run explicitly.

Each snapshot and pointer update is an atomic replacement: serialize to a sibling
temporary file, flush and sync it, rename it over the destination, then sync the parent
directory. A failed update leaves the previous valid checkpoint authoritative. Mutating
commands acquire the pipeline's exclusive operating-system file lock without waiting and
hold it through their final checkpoint and pointer update. A concurrent mutation fails
immediately. `status` is read-only and relies on atomic snapshots rather than taking the
mutation lock.

Loading rejects malformed or truncated JSON, unknown schema versions, invalid ULIDs or
pipeline IDs, pointer/snapshot identity mismatches, and project/config path mismatches.
Validation failures do not rewrite, recover, or partially trust the stored state.

## Command lifecycle

`start` creates a new ULID run, selects it in the pipeline's `active` pointer, and executes
until a gate or terminal status. It refuses to replace that pipeline's active run while
the active run is non-terminal.

`continue` selects a run, requires `waiting_at_gate`, records approval of that exact gate,
and executes until the next gate or terminal status. Repeated invocations keep selecting
the same active ULID unless the operator explicitly selects another run.

`abort` selects a non-terminal run and records an operator abort without consuming a
Worker turn or recovery. `status` only reads and reports run state.

Pipeline IDs select the active run for that pipeline. `--run <ulid-or-prefix>` selects a
specific run and must resolve uniquely within the current project. When `continue` or
`abort` has neither a pipeline ID nor `--run`, Piper opens a `dialoguer` selector across
eligible project runs if both input and output are terminals. `continue` offers runs
waiting at gates; `abort` offers all non-terminal runs. Cancellation exits without a
mutation. In a non-interactive process, omission is an error that requires a pipeline ID
or `--run`.

The existing `run` command is the attended wrapper over the same persistent engine. It
starts durably, presents gates in the terminal, and applies the operator's continue or
abort choice until the run pauses, terminates, or the terminal interaction is cancelled.
The engine itself is headless: reaching a gate returns `waiting_at_gate` and never reads
from the terminal. Cancelling an attended prompt leaves the run waiting at that gate.

Continuation reloads runtime secrets and the LLM invocation from the stored canonical
configuration path. An explicitly supplied configuration, pipeline, or prompt path must
canonicalize to the path stored in the snapshot; a mismatch fails closed. Secrets are not
copied into run snapshots.

## Target additions (draft)

These extend the durable model for the Blueprint/Version and factory-harness design; the
mechanism choices are open ([`OPEN-QUESTIONS.md`](OPEN-QUESTIONS.md)).

- **Work plane per Run.** Each Run owns an isolated sandbox — its checkout, filesystem, and
  git — so Runs of one Blueprint execute in parallel without collision. Work tools
  ([`engine.md`](engine.md)) act only inside it. The sandbox mechanism (container, git
  worktree, or other) is undecided.
- **Version pinning.** A Run snapshots the Blueprint **Version** it launched from and
  resumes against that pinned Version, so editing a Blueprint never mutates an in-flight
  Run and concurrent Runs may span Versions ([`blueprint.md`](blueprint.md)). An in-run
  local patch is a Run-local divergence from the pinned Version.
- **Gate as a decision record.** A Gate pause is a durable, per-Run record of a pending
  human decision, queryable across all Runs — the substrate for the cross-Run attention
  view in [`interface.md`](interface.md). Resuming a Gate over the control API records
  approval of that exact record, as the implemented `continue` does today.
- **Every tool call is checkpointed.** The checkpoint discipline above extends from model
  calls to each mediated work-tool and control-flow-tool call, giving every work-plane
  effect a logged, resumable provenance.
