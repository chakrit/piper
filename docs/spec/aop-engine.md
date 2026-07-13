# AOP Engine Specification

- **Status:** implemented
- **Scope:** Piper's driver, pipeline configuration, prompt templates, and Worker protocol.

## Purpose

Piper clarifies an immutable Request through small, ordered Aspect lenses. Each Worker
consumes the Request and the current Corpus, then emits one machine-readable DSL command.
Workers add durable Knowledge to the Corpus; they never rewrite the Request. A pipeline
that needs a final synthesis models it as an explicit Aspect and terminates with `DONE`.

## State and execution

`PipelineState` contains the Request, goal, ordered Aspects, current position, Corpus,
optional fork ID and prompt, and a non-zero turn limit. A `SessionContext` is shared by the
root pipeline and every fork. It records completed fork Corpora and the total number of
Worker calls.

The normal transition is:

`REQUEST + ASPECT + CORPUS → ADDITIONAL CORPUS`

For a `worker`, the driver loads `aspect-worker.md`, substitutes the template variables,
calls the configured LLM, parses its one command, then applies it. `NEXT` and `MERGE`
advance to the next Aspect; `JUMP` and `DONE` control flow explicitly. A `gate` instead
pauses for a human to continue or abort; it cannot alter the Request.

Every Worker pass consumes one turn. Its one permitted recovery call, when needed, is part
of that pass and does not consume an additional turn. The shared `max_turns` limit prevents
unbounded jump and fork loops; reaching it fails the run before another Worker pass. Gates
do not consume turns.

If the first response cannot be parsed as one DSL command, the driver makes one recovery
call using `aspect-recover.md`. A recovery response that is still invalid fails the run.

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
type = "worker"
prompt = "Record requirements that must be met."
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

The `run` command accepts a pipeline name, `--goal`, `--request`, and an optional
`--prompts` directory. Its default prompt directory is `prompts`. `--file` selects the
configuration file and defaults to `piper.toml`.

## Worker DSL

Every Worker response is exactly one non-empty line. It has one uppercase verb and its
arguments, separated by whitespace. One final newline is allowed; Markdown fences, JSON,
leading or trailing whitespace, and additional lines are rejected. IDs and knowledge keys
match `[A-Za-z0-9_-]+`. Text arguments are the non-empty remainder of the line.

```text
NEXT
ADD-KNOWLEDGE spec-align Preserve API compatibility
UPDATE-KNOWLEDGE spec-align Preserve the stable API
JUMP review
FORK alternative-1 Explore the implementation option
MERGE alternative-1,alternative-2
DONE Final result
```

| Command | Arguments | Effect |
| --- | --- | --- |
| `NEXT` | none | Advances to the next Aspect without changing the Request. |
| `ADD-KNOWLEDGE` | `<key> <value>` | Adds a previously absent key, then re-runs this Aspect. |
| `UPDATE-KNOWLEDGE` | `<key> <value>` | Replaces an existing key, then re-runs this Aspect. |
| `JUMP` | `<aspect-id>` | Transfers control to the named Aspect. |
| `FORK` | `<fork-id> <prompt>` | Runs a child from the next Aspect with the same Request, goal, and Corpus, then re-runs this Aspect. |
| `MERGE` | `<fork-id>[,<fork-id>...]` | Incorporates completed child Corpus entries, then advances. |
| `DONE` | `<final-result>` | Returns a final Result immediately. |

The driver rejects an unknown verb, malformed arguments, duplicate IDs in one `MERGE`, a
duplicate fork ID, a fork from the final Aspect, an unknown or incomplete merge target,
`ADD-KNOWLEDGE` for an existing key, and `UPDATE-KNOWLEDGE` for a missing key.

## Corpus and forks

The Corpus is a map of durable Knowledge injected into every later Worker prompt. Root
pipeline keys are written as provided. A fork automatically writes each of its keys under
`fork-id/key`, for example `alternative/tdd-red`. This prevents sibling and parent
collisions while keeping provenance visible.

A fork begins with a copy of the parent Corpus and starts at the Aspect immediately after
the `FORK` command. The fork prompt is injected into its Worker prompt. When a parent runs
`MERGE`, the driver copies the fork's Corpus entries into the parent Corpus. Only an
explicit merge makes a fork's discoveries available to later parent Aspects.

Forks execute synchronously and recursively. A fork can itself fork or merge, subject to
the same shared turn limit.

## Templates

| Template | Role | Variables |
| --- | --- | --- |
| `aspect-worker.md` | Worker request | `aspect_name`, `aspect_id`, `aspect_prompt`, `request`, `goal`, `fork_prompt`, `context` |
| `aspect-recover.md` | One-shot formatter recovery | The rejected raw response is appended by the driver. |

## Known limitations

- The LLM invocation is split on whitespace, so command templates cannot safely express
  shell-style quoting. The invocation must include `{{prompt}}`; `{{model}}` is optional.
