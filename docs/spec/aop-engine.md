# AOP Engine Specification

- **Status:** accepted
- **Scope:** Piper's driver, pipeline configuration, prompt templates, and Worker protocol.

## Purpose

Piper clarifies an immutable Request through small, ordered Aspect lenses. Each Worker
consumes the Request and current Corpus, then emits one machine-readable DSL command. A
Worker contributes knowledge; it never rewrites the Request. Later Aspects consume the
expanded Corpus, and a pipeline that needs a final synthesis terminates explicitly with
`DONE`.

The normal transition is:

`REQUEST + ASPECT + CORPUS → CONTRIBUTION TO CORPUS`

## Core concepts

- **Request** — immutable original user input, including the desired outcome.
- **Aspect** — a named perspective or question applied to the Request.
- **Worker** — the LLM-run stage that applies one Aspect.
- **Corpus** — durable knowledge contributed by Workers and fed to later Workers.
- **Result** — explicit final synthesis returned by `DONE`.
- **Fork** — an isolated child execution branch with its own temporary Corpus.

Each contribution is identified internally by the branch that produced it and the Aspect
that set it. Workers never supply arbitrary knowledge keys.

## State and execution

`PipelineState` contains the Request, ordered Aspects, current position, Corpus,
serializable execution-frame stack, a non-zero turn limit, and a recovery limit. The root
and every fork share total Worker-call and recovery-call counts. The frame stack captures
branch scope, pending children, and return positions so execution can be checkpointed and
resumed without relying on the process call stack.

For a `worker`, the driver loads `aspect-worker.md`, substitutes the template variables,
calls the configured LLM, parses one command, and applies it. A `gate` returns a typed
pause to its caller; it cannot alter the Request. The engine is headless and never prompts
for approval. Persistent checkpoint and gate behavior are defined in
[`run-lifecycle.md`](run-lifecycle.md).

Every Worker pass consumes one turn. A formatter-recovery call, when needed, is part of
that pass and does not consume an additional turn, but it consumes the separate shared
recovery budget. The shared `max_turns` limit prevents unbounded jump and fork loops;
reaching it fails the run before another Worker pass. The shared `max_recoveries` limit
prevents a long run from repairing malformed output on every pass. Gates consume neither
budget.

If the first response cannot be parsed as one DSL command and recovery budget remains, the
driver makes one recovery call using `aspect-recover.md`. A recovery response that is still
invalid fails the run. Reaching the recovery limit fails before another recovery call.

## Pipeline configuration

Shared configuration and pipelines live in separate TOML files. `piper.toml` contains
runtime configuration such as `[llm]`. A pipeline file contains one `[pipeline]` table.
Piper loads both files, merges the pipeline into the loaded configuration, and processes
the resulting effective configuration. `id` is required and uses `[A-Za-z0-9_-]+`; it is
the stable, human-readable identity used to select durable runs. `max_turns` is optional,
defaults to `100`, and must be at least one. `max_recoveries` is optional, defaults to `3`,
and may be `0` to disable recovery. Both budgets are shared by root and fork execution.
Aspects default to `worker`; set `type = "gate"` only for gate Aspects.

```toml
[llm]
invocation = "opencode run -m {{model}} {{prompt}}"
system_prompt = "You are a precise AOP worker."
default_model = "cerebras/gemma-4-31b"

[pipeline]
id = "security-review"
max_turns = 50
max_recoveries = 3

[[pipeline.aspects]]
id = "spec-align"
name = "Spec Alignment"
prompt = "Record requirements that must be met."
model = "optional-model-override"

[[pipeline.aspects]]
id = "review"
name = "Human Review"
type = "gate"
prompt = ""
```

The `run` command accepts a pipeline TOML path followed by the immutable Request and an
optional `--prompts` directory. The Request includes the desired outcome rather than
splitting it into a separate Goal field. Its default prompt directory is `prompts`.
`--file` explicitly selects the shared configuration file; when omitted, Piper searches
the current directory and its ancestors for the nearest `piper.toml`. For example:

```text
piper --file piper.toml run pipeline.toml "Review this change for security regressions."
```

## Worker DSL

Every Worker response is exactly one non-empty line. It has one uppercase verb and its
arguments, separated by whitespace. One final newline is allowed; Markdown fences, JSON,
leading or trailing whitespace, and additional lines are rejected. IDs match
`[A-Za-z0-9_-]+`. Text arguments are the non-empty remainder of the line.

```text
SET Preserve public API compatibility.
CONTINUE
JOIN
JUMP review
FORK alternative-1 Explore migration risk.
STOP Missing security requirements.
DONE Final readiness assessment.
```

| Command | Arguments | Effect |
| --- | --- | --- |
| `SET` | `<knowledge>` | Sets the current Worker's contribution in its current branch, then re-runs that Worker. It creates the contribution when absent and replaces it when present. |
| `CONTINUE` | none | Alias of `JOIN`. Joins any completed child Corpora first. In the root run, advances to the next Aspect; in a fork, returns the branch Corpus to its parent. |
| `JOIN` | none | Alias of `CONTINUE`. Joins any completed child Corpora first. In the root run, advances to the next Aspect; in a fork, returns the branch Corpus to its parent. |
| `JUMP` | `<aspect-id>` | Transfers control to an `ASPECT` target. It is invalid while the current Worker has unjoined forks. |
| `FORK` | `<fork-id> <directive>` | Runs an isolated child on the current Aspect with the same Request and starting Corpus. The child receives the parent Aspect instruction plus its branch directive. The child completes before the parent Worker runs again. |
| `STOP` | `<reason>` | Stops the entire run cleanly without a final Result. A child `STOP` stops its parent run as well. |
| `DONE` | `<result>` | Returns a final Result immediately. It is invalid while the current Worker has unjoined forks. |

The driver rejects unknown verbs, malformed arguments, duplicate fork IDs from one Worker,
and `JUMP`, `STOP`, or `DONE` that would discard unjoined fork work. `SET` is the sole
knowledge mutation command; `ADD-KNOWLEDGE`, `UPDATE-KNOWLEDGE`, `NEXT`, and `MERGE` are
not part of this protocol.

## Corpus context

The driver provides each Worker with compact directives:

```text
CONTEXT security-review
KNOWLEDGE Preserve public API compatibility.
ASPECT security-review
ASPECT final-synthesis
FORK alternative-1 Explore migration risk.
FORK_KNOWLEDGE alternative-1 Preserve rollback compatibility.
```

- `CONTEXT` identifies the current Aspect followed by any nested fork path. A root pass uses
  only the Aspect ID, such as `CONTEXT security-review`; a child uses paths such as
  `CONTEXT security-review/alternative-1`.
- `KNOWLEDGE` contains the current Corpus. Source metadata may be displayed for orientation,
  but Workers do not address entries by key.
- `ASPECT` lists the valid `JUMP` targets.
- `FORK` lists completed forks launched by the current Worker, including the directive used
  to create each fork. This history prevents the Worker from repeating equivalent branches.
- `FORK_KNOWLEDGE` exposes a completed child's contributions to its parent Worker, correlated
  by fork ID. These pending contributions are not part of the parent Corpus until the
  parent emits `CONTINUE` or `JOIN`.

`FORK` history is scoped to the Worker that created those branches. A child has independent
fork history if it creates further forks. New sibling forks start from the same parent
Corpus and do not inherit pending contributions from previously completed siblings.

## Fork lifecycle

A fork is a branch, not a namespace the Worker must manage.

1. A Worker emits `FORK alternative-1 Explore migration risk.`
2. The driver runs that child synchronously on the same Aspect with a copy of the parent
   Corpus. Its prompt says it is in a fork, repeats the parent Aspect instruction, supplies
   the fork instruction, and identifies the branch as
   `CONTEXT <aspect-id>/<fork-path...>`.
3. The child may use `SET` repeatedly to establish or replace its contribution.
4. The child emits `CONTINUE` or `JOIN`. Both commands join any completed descendants and
   return the branch Corpus to its parent instead of advancing the child to another Aspect.
5. The driver retains the completed child Corpus and returns to the parent Worker. The
   parent reruns on the same Aspect with `FORK alternative-1 …` history and correlated
   `FORK_KNOWLEDGE alternative-1 …` directives. The child contributions remain pending,
   not yet part of the parent Corpus.
6. The parent may use the pending results to `SET` a consolidated contribution or create
   more sibling forks. Every sibling starts from the unchanged parent Corpus.
7. The parent eventually emits either `CONTINUE` or `JOIN`.
8. The command waits for all child work, joins every completed child Corpus into the parent
   Corpus, clears the fork history, and advances the root run to the next Aspect.

`CONTINUE` and `JOIN` are two names for the same context-sensitive transition, not separate
execution choices. A Worker may use whichever verb best expresses its perspective. Later
root Aspects never start before all child work is complete and joined. Selective or partial
merges are deliberately deferred until there is a concrete need for them.

## Templates

| Template | Role | Variables |
| --- | --- | --- |
| `aspect-worker.md` | Worker request | `aspect_name`, `aspect_id`, `aspect_prompt`, `request`, `fork_prompt`, `context` |
| `aspect-recover.md` | One-shot formatter recovery | The rejected raw response is appended by the driver. |

## Implementation note

This is the accepted target protocol. The implementation and example pipelines must be
migrated to it together; no compatibility aliases for removed verbs are required.
