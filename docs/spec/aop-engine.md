# AOP Engine Specification

- **Status:** implemented
- **Scope:** Piper's driver, pipeline configuration, prompt templates, and Worker Lens protocol.

## Purpose

Piper reduces instruction drift by sending a draft through small, ordered Aspect lenses.
Each lens returns a machine-readable action list, and the driver applies those actions to
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
calls the configured LLM, parses its response, then applies every returned action in order.
If no action jumps or terminates, the driver advances to the next Aspect. A `gate` instead
pauses for a human to continue, edit the draft, or abort.

Every Worker Lens call consumes one turn. The shared `max_turns` limit prevents unbounded
jump and fork loops; reaching it fails the run before another LLM call. Gates do not
consume turns.

If the first response cannot be parsed as the action JSON object, the driver makes one
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

[pipelines.rigour]
max_turns = 50
```

An aspect entry may instead reference a TOML file:

```toml
[[pipelines.rigour.aspects]]
file = "aspects/spec-align.toml"
```

The `run` command accepts a pipeline name, `--goal`, `--draft`, and an optional `--prompts`
directory. Its default prompt directory is `skills/piper/prompts`. `--file` selects the
configuration file and defaults to `piper.toml`.

## Worker Lens action protocol

Each response is one JSON object with an ordered `actions` array and a numeric
`violations_found` count. Omit actions that are not needed.

```json
{
  "actions": [
    { "refine": { "draft": "Updated draft" } },
    { "add_knowledge": { "key": "spec-align", "value": "Preserve API compatibility." } },
    { "jump": { "id": "review" } }
  ],
  "violations_found": 0
}
```

| Action | Effect |
| --- | --- |
| `refine` | Replaces the working draft. |
| `add_knowledge` | Inserts a durable corpus entry. |
| `edit_knowledge` | Replaces a durable corpus entry. |
| `jump` | Sets the next Aspect by ID instead of advancing sequentially. |
| `fork` | Starts a nested pipeline with its own ID, prompt, goal, draft, and optional starting Aspect ID. |
| `merge` | Appends each completed fork result to the working draft and merges its namespaced corpus entries. |
| `terminate` | Stops the current pipeline and returns its current draft. |

Actions execute in array order. A `jump` changes control flow after all actions in the
current response have applied. `terminate` ends that pipeline after the response's other
actions have applied.

## Corpus and forks

The corpus is a map of durable constraints injected into every later refiner prompt. Root
pipeline keys are written as provided. A fork automatically writes each of its keys under
`fork-id/key`, for example `alternative/tdd-red`. This prevents sibling and parent
collisions while keeping provenance visible.

A fork begins with a copy of the parent corpus. When a parent runs `merge`, the driver's
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
| `aspect-definition.md` | Human/LLM aid for designing a pipeline | No driver substitution. |

`final-alignment.md` is intentionally absent. Final alignment is an ordinary, explicit
Aspect when a pipeline requires it.

## Known limitations

- A merge concatenates output; it does not semantically reconcile divergent drafts. Use a
  later refiner Aspect to synthesize branches when needed.
- The action protocol does not yet validate duplicate fork IDs, references to incomplete
  forks, or action combinations beyond their runtime effects.
- The LLM invocation is split on whitespace, so command templates cannot safely express
  shell-style quoting. The invocation must include `{{prompt}}`; `{{model}}` is optional.
