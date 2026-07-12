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
A **Pipeline** is a structured sequence of Aspects (defined in `pipelines/`). Each
Aspect contains a name and a rigorous lens description.

### 2. The Autonomous Loop (The State Machine)
The Driver program executes the following state transitions:
`INPUT` $\rightarrow$ `ASPECT_1` $\rightarrow$ `ASPECT_2` $\rightarrow$ ... $\rightarrow$
`ASPECT_N` $\rightarrow$ `SYNTHESIS` $\rightarrow$ `OUTPUT`

For each transition:
1. **Provision Worker**: The Driver creates a fresh LLM context.
2. **Inject Aspect**: The Driver provides the current Working Draft + Aspect Lens +
   Machine-Parsable Output requirement.
3. **Parse Output**: The Driver extracts the refined draft from the Worker's
   structured response (JSON).
4. **Update State**: The extracted draft becomes the global "Working Draft" for the
   next state.

### 3. Final Synthesis
The Driver performs a final "Joint Alignment" pass to ensure cohesion across the
piped edits.

## Automation & Apparatus

- **`pipelines/`**: JSON definitions of Aspect sequences.
- **`piper` (Rust binary)**: The high-performance state-machine driver that automates the pipe, supports human-in-the-loop "Stop-Gates", and allows for LLM-triggered "Rewinds" to previous aspects.
- **`prompts/`**: Templated prompts that enforce machine-parsable output for Worker
  Lenses.
