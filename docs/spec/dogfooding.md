# Self-Dogfooding: Piper Develops Piper

- **Status:** accepted
- **Scope:** Piper's first goal, the Worker engine, and the phased path from
  harness-driven to piper-driven development.

## First goal

Piper's first goal is to develop itself. Piper's own engineering runs through a Piper
pipeline rather than through direct harness chats, so every improvement to the workflow is
exercised on the next change immediately. The product and its development process are one
artifact.

## The Worker engine is Claude

Workers drive a Claude model, default `sonnet`, loaded from `.env`. In the target design
([`engine.md`](engine.md)) a Worker is a **tool-calling loop**, not a one-shot invocation:
the single-line `claude -p --model {{model}} {{prompt}}` call retires in favor of a
tool-calling invocation (API or SDK) so Workers can call work tools and control-flow tools.
The configured system prompt is prepended by the Driver.

## The development pipeline

`ace-piper.toml` encodes the ACE coding workflow as an ordered Aspect sequence — clarify,
plan, test-first, red/green/refactor, verify, audit — with human gates at the points the
workflow stops for approval. Development is then a single invocation:

    piper run ace-piper.toml "<the change to make>"

The driver owns sequencing, forks for independent work streams, gates, and completion;
each Worker supplies judgment for one Aspect. This is the alignment-by-construction
property of [`anti-harness.md`](anti-harness.md) applied to Piper's own development.

## Work ordering

The path to piper-driven development runs in three ordered phases; each depends on the one
before it.

1. **Make the engine work well.** Harden Piper's core functionality — the durable run
   lifecycle, forks, gates, and recovery — until it is reliable enough to trust with real
   work. This precedes any workflow encoding. In the expanded design this phase now spans
   the tool-calling engine ([`engine.md`](engine.md)), the Blueprint/Run data model
   ([`blueprint.md`](blueprint.md)), the Supervisor ([`supervisor.md`](supervisor.md)), and
   the interface ([`interface.md`](interface.md)); the decisions that gate building them are
   in [`OPEN-QUESTIONS.md`](OPEN-QUESTIONS.md).
2. **Define the pipeline.** Encode the ACE workflow as `ace-piper.toml` once the engine is
   solid.
3. **Switch the drive.** Move Piper's own development from harness-driven to
   `ace-piper.toml`-driven.

`ace-piper.toml` and phases 2–3 are the target; the current state is phase 1.

## Phase 2 (target): piper chat mode

A later phase adds an interactive chat mode, deliberately unlike a traditional harness
chat:

- Each user input is entered into the Piper tree as a Request, not sent to a free-form
  model conversation.
- A TUI streams pipeline progress — Aspect position, forks, Corpus growth, and gates.
- The pipeline runs to completion before the user can enter the next input, unless the
  user interrupts it.

This keeps the alignment-by-construction guarantee in an interactive setting: input always
flows through the pipeline, never around it.
