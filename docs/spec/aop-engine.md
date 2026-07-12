# AOP Engine Specification

- **Status:** implemented
- **Scope:** Piper's driver, pipeline configuration, prompt templates, and Worker Lens protocol.

## Purpose

Piper reduces instruction drift by sending a draft through small, ordered Aspect lenses.
Each lens returns one machine-readable DSL command, and the driver applies that command to
the current pipeline state. The driver never performs an implicit synthesis pass; a
pipeline that needs a final review models it as a normal Aspect.

## State and execution

`PipelineState` contains the working draft, original goal, ordered Aspects, current
position, corpus, optional fork ID and prompt, and a non-zero turn limit. A `SessionContext`
is shared by the root pipeline and every fork. It records completed fork results, completed
fork corpora, and the total number of Worker Lens calls.

The normal transition is:

`INPUT → ASPECT_1 → … → ASPECT_N → OUTPUT`

For a `refiner`, the driver loads `aspect-refiner.md`, substitutes the template variables,
calls the configured LLM, parses its one command, then applies it. `REFINE` and `MERGE`
advance to the next Aspect; `JUMP` and `DONE` control flow explicitly. A `gate` instead
pauses for a human to continue, edit the draft, or abort.

Every refiner pass consumes one turn. Its one permitted recovery call, when needed, is part
of that pass and does not consume an additional turn. The shared `max_turns` limit prevents
unbounded jump and fork loops; reaching it fails the run before another refiner pass. Gates
do not consume turns.

If the first response cannot be parsed as one DSL command, the driver makes one
recovery call using `aspect-recover.md`. A recovery response that is still invalid fails
the run.

## Pipeline configuration

Pipelines live in TOML. `max_turns` is optional and defaults to `100`; it must be at least
one. Aspect files use `type`, not `aspect_type`.

```toml
[llm]
invocation = "opencode run -m {{model}} {{prompt}}"
system_prompt = "You are a precise AOP worker."
default_model = "cerebras/gemma-4-31b"

[pipelines.rigour]
max_turns = 50

[[pipelines.rigour.aspects]]
id = "spec-align"
name = "Spec Alignment"
type = "refiner"
prompt = "Ensure the draft matches accepted specifications."
model = "optional-model-override"

[[pipelines.rigour.aspects]]
id = "review"
name = "Human Review"
type = "gate"
prompt = ""
```

An aspect entry may instead reference a TOML file:

```toml
[[pipelines.rigour.aspects]]
file = "aspects/spec-align.toml"
```

The `run` command accepts a pipeline name, `--goal`, `--draft`, and an optional `--prompts`
directory. Its default prompt directory is `prompts`. `--file` selects the
configuration file and defaults to `piper.toml`.

## Worker Lens DSL

Every refiner response is exactly one non-empty line. It has one uppercase verb and its
arguments, separated by whitespace. One final newline is allowed; Markdown fences, JSON,
leading or trailing whitespace, and additional lines are rejected. IDs and knowledge keys
match `[A-Za-z0-9_-]+`. Text arguments are the non-empty remainder of the line.

```text
REFINE Updated draft
ADD-KNOWLEDGE spec-align Preserve API compatibility
UPDATE-KNOWLEDGE spec-align Preserve the stable API
JUMP review
FORK alternative-1 Explore the implementation option
MERGE alternative-1,alternative-2
DONE Final output
```

| Command | Arguments | Effect |
| --- | --- | --- |
| `REFINE` | `<draft>` | Replaces the draft and advances to the next Aspect. |
| `ADD-KNOWLEDGE` | `<key> <value>` | Adds a previously absent key, then re-runs this Aspect. |
| `UPDATE-KNOWLEDGE` | `<key> <value>` | Replaces an existing key, then re-runs this Aspect. |
| `JUMP` | `<aspect-id>` | Transfers control to the named Aspect. |
| `FORK` | `<fork-id> <prompt>` | Runs a child from the next Aspect with the current draft and goal, then re-runs this Aspect. |
| `MERGE` | `<fork-id>[,<fork-id>...]` | Incorporates completed child outputs and corpora, then advances. |
| `DONE` | `<final-draft>` | Replaces the draft and returns it immediately. |

The driver rejects an unknown verb, malformed arguments, duplicate IDs in one `MERGE`, a
duplicate fork ID, a fork from the final Aspect, an unknown or incomplete merge target,
`ADD-KNOWLEDGE` for an existing key, and `UPDATE-KNOWLEDGE` for a missing key.

## Corpus and forks

The corpus is a map of durable constraints injected into every later refiner prompt. Root
pipeline keys are written as provided. A fork automatically writes each of its keys under
`fork-id/key`, for example `alternative/tdd-red`. This prevents sibling and parent
collisions while keeping provenance visible.

A fork begins with a copy of the parent corpus and starts at the Aspect immediately after
the `FORK` command. The fork prompt is injected into its Worker Lens prompt. When a parent
runs `MERGE`, the driver's
current implementation appends the fork's final draft under a labeled delimiter and copies
the fork's corpus entries into the parent corpus. Because fork-owned keys are namespaced,
only an explicit merge makes their new discoveries available to later parent Aspects.

Forks execute synchronously and recursively. A fork can itself fork or merge, subject to
the same shared turn limit.

## Templates

| Template | Role | Variables |
| --- | --- | --- |
| `aspect-refiner.md` | Worker Lens request | `aspect_name`, `aspect_id`, `aspect_prompt`, `current_draft`, `goal`, `fork_prompt`, `corpus` |
| `aspect-recover.md` | One-shot formatter recovery | The rejected raw response is appended by the driver. |

`final-alignment.md` is intentionally absent. Final alignment is an ordinary, explicit
Aspect when a pipeline requires it.

## Known limitations

- A merge concatenates output; it does not semantically reconcile divergent drafts. Use a
  later refiner Aspect to synthesize branches when needed.
- The LLM invocation is split on whitespace, so command templates cannot safely express
  shell-style quoting. The invocation must include `{{prompt}}`; `{{model}}` is optional.
