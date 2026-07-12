# Piped-Piper: Aspect-Oriented Prompting (AOP) Engine

Piped-Piper is a context engineering engine that implements Aspect-Oriented Prompting.
It decomposes complex alignment constraints into modular "Aspects," processing them
sequentially via a state-machine driver to eliminate instruction drift.

## Architecture
- `skills/piped_piper/pipelines/`: Pipeline definitions (Aspect sequences).
- `skills/piped_piper/prompts/`: Aspect lens templates.
- `skills/piped_piper/scripts/driver.py`: The state-machine driver.

## Core Tooling
- **Driver**: `piped-driver` (via `pyproject.toml`)
- **Build/Run**: Python 3.12+, Hatchling.

## ACE Configuration
Managed by [ACE](https://github.com/ace-rs/ace).
- `ace` to start/resume sessions.
- Skills are symlinked from PRODIGY9 Coding School.

## Durable artifacts

`docs/` — file by the routing gate in `docs/README.md`: a ruling → `decisions/`;
third-party lookup → `vendor/`; a how-to → `guides/`; our own design/surface →
`spec/`; unsettled exploration → `scratch/` (last resort, opened with a
"not spec/decision because ___" line). Nothing defaults to `scratch/`.

