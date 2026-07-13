# Piper

Aspect-Oriented Prompting (AOP) Engine.

## Overview
Piper is a context-engineering engine for clarifying a Request through modular Aspects. Each Worker contributes durable Knowledge to a shared Corpus; later Aspects consume the expanded Corpus without rewriting the Request.

## Architecture
- **Aspects**: Modular lens templates that define specific alignment or behavioral constraints.
- **Workers**: LLM-run stages that apply an Aspect to the Request and Corpus.
- **Request**: The immutable user input under consideration.
- **Corpus**: Accumulated Knowledge, constraints, decisions, and findings.
- **Pipelines**: Ordered sequences of Aspects that define the processing flow for a given task.
- **Driver**: A Rust state machine that executes one compact DSL command from each Worker, including knowledge capture, routing, forks, merges, gates, and completion.

## Project Structure
- `src/`: Rust source code for the driver.
- `piper.toml`: Pipeline definitions (Aspect sequences and turn limits).
- `prompts/`: Aspect lens templates.
- `pipelines/`: Reusable pipeline configurations.
- `docs/`: Design specifications, decisions, and guides.

## Getting Started
### Prerequisites
- Rust (latest stable)
- Cargo

### Build & Run

Build the tool and run the checked-in smoke pipeline:

```bash
cargo run -- --file pipelines/smoke.toml run smoke \
  --goal "Verify the Piper driver" --request "Confirm the Worker protocol."
```

`piper.toml` defines named pipelines. Each pipeline has an ordered `aspects` list and an
optional `max_turns` limit (default: 100) shared by the root run and every fork. See
[`docs/spec/aop-engine.md`](docs/spec/aop-engine.md) for the configuration and action
protocol. The tracked [`self-evolve` pipeline](pipelines/self-evolve.toml)
distills `ultra-engineer.md` into a bounded, fork-aware self-improvement loop.

## License
MIT
