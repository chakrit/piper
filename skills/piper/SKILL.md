---
name: piper
description: Implements Aspect-Oriented Prompting (AOP) to maximize alignment by looping a
  draft through a series of modular "Aspects". Use this skill when you need to refine
  a complex output against multiple, potentially conflicting constraints (e.g.,
  security, performance, brand, UX) without using adversarial prompting or
  monolithic prompts that cause instruction drift. Trigger on "piper",
  "aspect-oriented prompting", "AOP loop", "pipe through aspects", or when a task
  requires multi-dimensional alignment.
---

# Piper: Aspect-Oriented Prompting (AOP) Engine

Piper is a context engineering engine that adapts the software engineering
concept of Aspect-Oriented Programming (AOP) to LLM prompting. Instead of a single,
monolithic prompt or manual iterative refinement, Piper uses a deterministic
pipeline of modular "Aspects" executed by a state-machine driver.

## The Concept: What is an Aspect?

An **Aspect** is a specialized, modular lens of constraint. It handles exactly one
dimension (e.g., "Security", "Brand Voice").

By "piping" a draft through a sequence of Aspects via a programmatic driver, we
ensure every constraint is isolated and enforced in a fresh context, eliminating
instruction drift.

## The Engine Workflow

The Piper architecture consists of a **Deterministic Driver** (a small
program) and **Worker Lenses** (LLM calls).

### 1. Pipeline Definition
A **Pipeline** is a structured sequence of Aspects in `piper.toml`. Each Aspect has an
ID, name, type, prompt, and optional model override. A pipeline also declares a
`max_turns` safety limit (100 by default) shared by the root execution and its forks.

### 2. The Autonomous Loop (The State Machine)
The Driver program begins with the first Aspect and normally advances sequentially.
Each Worker Lens returns an ordered JSON action list. Actions can refine the draft,
add or edit corpus knowledge, jump to an Aspect, fork a branch, merge completed fork
results, or terminate. Gates pause for human review. There is no implicit final-synthesis
pass; model a final pass explicitly as an Aspect if one is needed.

For each transition:
1. **Provision Worker**: The Driver creates a fresh LLM context.
2. **Inject Aspect**: The Driver provides the current Working Draft + Aspect Lens +
   Machine-Parsable Output requirement.
3. **Parse Output**: The Driver validates the Worker's structured action response. If
   the first parse fails, it makes one recovery call using `aspect-recover.md`.
4. **Update State**: The Driver applies actions in order. Fork corpus entries are
   namespaced as `fork-id/key`, avoiding collisions with root and sibling branches.

The engine specification in `docs/spec/aop-engine.md` is the source of truth for the
configuration and action protocol.

## Automation & Apparatus

- **`piper.toml`**: TOML definitions of Aspect sequences and their turn limits.
- **`piper` (Rust binary)**: The high-performance state-machine driver that automates the pipe, supports human-in-the-loop "Stop-Gates", and allows for LLM-triggered "Rewinds" to previous aspects.
- **`prompts/`**: Templated prompts that enforce machine-parsable output for Worker
  Lenses.
