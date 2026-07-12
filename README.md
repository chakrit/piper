# Piper

Aspect-Oriented Prompting (AOP) Engine.

## Overview
Piper is a context engineering engine that implements Aspect-Oriented Prompting. It decomposes complex alignment constraints into modular "Aspects," processing them sequentially via a state-machine driver to eliminate instruction drift.

## Architecture
- **Aspects**: Modular lens templates that define specific alignment or behavioral constraints.
- **Pipelines**: Ordered sequences of Aspects that define the processing flow for a given task.
- **Driver**: A Rust state machine that executes ordered actions from each Aspect, including refinement, knowledge capture, routing, forks, merges, gates, and termination.

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
```bash
cargo run -- run rigour --goal "Make the draft accurate" --draft "Initial draft"
```

`piper.toml` defines named pipelines. Each pipeline has an ordered `aspects` list and an
optional `max_turns` limit (default: 100) shared by the root run and every fork. See
[`docs/spec/aop-engine.md`](docs/spec/aop-engine.md) for the configuration and action
protocol. The tracked [`self-evolve` pipeline](pipelines/self-evolve.toml)
distills `ultra-engineer.md` into a bounded, fork-aware self-improvement loop.

For a minimal real-Lens check, run:

```bash
cargo run -- --file pipelines/smoke.toml run smoke \
  --goal "Verify the Piper driver" --draft "Unverified"
```

## License
MIT
