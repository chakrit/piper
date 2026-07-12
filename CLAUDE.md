# Piper: Aspect-Oriented Prompting (AOP) Engine

Piper is a context engineering engine that implements Aspect-Oriented Prompting.
It decomposes complex alignment constraints into modular "Aspects," processing them
sequentially via a state-machine driver to eliminate instruction drift.

## Architecture
- `skills/piper/pipelines/`: Pipeline definitions (Aspect sequences).
- `skills/piper/prompts/`: Aspect lens templates.
- `src/main.rs`: The Rust state-machine driver.

## Core Tooling
- **Driver**: `cargo run` (or the compiled `piper` binary)
- **Build/Run**: Rust (Cargo)

## ACE Configuration
Managed by [ACE](https://github.com/ace-rs/ace).
- `ace` to start/resume sessions.
- Skills are symlinked from PRODIGY9 Coding School.

## Durable artifacts

`docs/` — file by the routing gate in `docs/README.md`: a ruling → `decisions/`;
third-party lookup → `vendor/`; a how-to → `guides/`; our own design/surface →
`spec/`; unsettled exploration → `scratch/` (last resort, opened with a
"not spec/decision because ___" line). Nothing defaults to `scratch/`.

