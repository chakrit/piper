<!-- not spec/decision because: this is a point-in-time handoff record, not current design -->

# Session State: 2026-07-12

## Summary
Reworked the Rust driver to be config-driven, renamed the project to `piper`, and fully implemented the Knowledge Corpus model with feed-forward constraints and configurable termination.

## Resolved Since This Record

- The fixed response schema and implicit final synthesis were replaced with the action
  protocol documented in `docs/spec/aop-engine.md`.
- Worker prompts moved to top-level `prompts/`; pipeline fixtures and examples now live
  under `tests/pipelines/`.
- The CLI default prompt directory is `prompts`; `aspect-recover.md` is required for the
  one-shot malformed-response recovery path.
- `max_turns` bounds root and fork execution; fork corpus keys are namespaced and merged
  explicitly.
- The reference-config test, recovery test, turn-limit test, and fork behavior coverage
  pass alongside the rest of the test suite.
- `tests/pipelines/smoke.toml` completed a real Worker Lens run with the expected terminating
  response.

## Current Handoff

Piper is now a standalone tool. Current behavior belongs in `docs/spec/`; use this file
only as historical context for the July 12–13 transition.
