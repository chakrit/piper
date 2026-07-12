# Piper

Aspect-Oriented Prompting (AOP) Engine.

## Overview
Piper is a context engineering engine that implements Aspect-Oriented Prompting. It decomposes complex alignment constraints into modular "Aspects," processing them sequentially via a state-machine driver to eliminate instruction drift.

## Architecture
- **Aspects**: Modular lens templates that define specific alignment or behavioral constraints.
- **Pipelines**: Ordered sequences of Aspects that define the processing flow for a given task.
- **Driver**: A Rust-based state machine that manages the transition between Aspects and handles the prompting loop.

## Project Structure
- `src/`: Rust source code for the driver.
- `skills/piper/pipelines/`: Pipeline definitions (Aspect sequences).
- `skills/piper/prompts/`: Aspect lens templates.
- `docs/`: Design specifications, decisions, and guides.

## Getting Started
### Prerequisites
- Rust (latest stable)
- Cargo

### Build & Run
```bash
cargo run
```

## License
MIT
