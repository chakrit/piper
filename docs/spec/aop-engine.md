# AOP Engine Specification

- **Status:** accepted
- **Scope:** Core Driver and Integration Contracts

## Overview

The **Aspect-Oriented Prompting (AOP) Engine** minimizes instruction drift in LLM outputs by decomposing complex alignment constraints into modular "Aspects". Instead of a single monolithic prompt, the engine pipes a working draft through a sequence of specialized lenses, each responsible for exactly one dimension of alignment (e.g., Security, Brand, UX).

### Core Workflow
The engine follows a strict state-machine transition:
`INPUT` $\rightarrow$ (`ASPECT_1` $\rightarrow$ `ASPECT_2` $\rightarrow$ ... $\rightarrow$ `ASPECT_N`) $\rightarrow$ `SYNTHESIS` $\rightarrow$ `OUTPUT`

## The Driver (State Machine)

The `PipedPiperEngine` manages the global state and orchestrates the movement of the draft through the pipeline.

### State Management
- **`working_draft`**: The primary state variable. It is initialized with the `initial_draft` and updated after every Aspect pass.
- **`corpus`**: A persistent key-value store enabling Aspects to pass metadata or extracted knowledge to subsequent steps.
- **Pipeline**: A deterministic sequence of Aspects loaded from a TOML configuration.

### Execution Logic
1. **Aspect Loop**: 
   - For each Aspect in the pipeline:
     - Load the `aspect-refiner.md` template.
     - Inject the Aspect's metadata (`name`, `description`), the `working_draft`, and the current `corpus`.
     - Execute the LLM call.
     - Parse the structured JSON response to extract:
       - `refined_draft`: Updates the `working_draft`.
       - `violations_found`: Numeric count of alignment failures.
       - `knowledge`: Merged into the global `corpus`.
       - `terminate`: If true, the engine skips remaining Aspects and jumps to synthesis.
       - `next_aspect_id`: If present, the engine jumps to the specified Aspect ID, overriding the linear sequence.
2. **Synthesis Pass**:
   - Load the `final-alignment.md` template.
   - Inject the `original_goal`, the `final_draft` (the result of the loop), and the list of Aspects applied.
   - Execute a final LLM call to ensure cohesion and joint alignment.

## Integration Contracts

### 1. Pipeline Schema (TOML)
Pipelines are defined in `piper.toml` as named tables containing an array of aspects:
```toml
[[pipelines.rigour.aspects]]
id = "spec-align"
name = "Spec Alignment"
type = "refiner"
prompt = "Ensure draft matches specifications..."
```

### 2. Worker Lens Output (Aspect Response)
The LLM serving as a Worker Lens must return a JSON object:
```json
{
  "refined_draft": "The updated text",
  "violations_found": 0,
  "next_aspect_id": "optional-id",
  "knowledge": {
    "key": "value"
  },
  "terminate": false
}
```

### 3. LLM Interface
The driver defines a `call_llm(prompt: str) -> str` hook. Any integration must implement this method to handle the actual API communication.

## Template Variable Map

| Template | Variable | Source | Description |
|-----------|-----------|--------|-------------|
| `aspect-refiner.md` | `{{aspect_name}}` | Pipeline TOML | The name of the current Aspect. |
| `aspect-refiner.md` | `{{aspect_description}}` | Pipeline TOML | The specific constraints for this Aspect. |
| `aspect-refiner.md` | `{{current_draft}}` | Engine State | The current `working_draft`. |
| `aspect-refiner.md` | `{{corpus}}` | Engine State | The current global knowledge map. |
| `final-alignment.md`| `{{original_goal}}` | CLI Arg | The user's intended outcome. |
| `final-alignment.md`| `{{final_draft}}` | Engine State | The result after all Aspect passes. |
| `final-alignment.md`| `{{aspect_list}}` | Pipeline TOML | Comma-separated list of applied Aspects. |
