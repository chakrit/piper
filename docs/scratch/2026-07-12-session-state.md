# Session State: 2026-07-12

## Summary
Reworked the Rust driver to be config-driven, renamed the project to `piper`, and fully implemented the Knowledge Corpus model with feed-forward constraints and configurable termination.

### Functional Changes
- Renamed project and binary from `pied-piper` $\rightarrow$ `piper`.
- Moved pipeline definitions to `piper.toml`.
- Implemented Knowledge Corpus:
    - LLM responses now return a keyed map of knowledge (`HashMap<String, String>`).
    - Driver injects the cumulative corpus into every aspect run (Feed-Forward).
    - Agents are instructed via `system_prompt.txt` and `aspect-refiner.md` to treat the corpus as binding constraints.
- Decoupled Final Synthesis:
    - Final alignment is no longer hard-coded in the driver.
    - Added `terminate` field to `LlmResponse`; driver stops when `terminate == true`.
    - This allows "Synthesis" aspects to be defined as configurations in `piper.toml`.
- CLI: Added `-f` / `--file` flag for config overrides.
- Integrated `src/system_prompt.txt` as a global prefix.

### Law Alignment (Audit Results)
- Implemented **Unit-of-Work Pattern** for filesystem mutations.
- Established **TDD loop** for core logic.

## Recent Updates (Current Session)
- **Git Setup**: Created GitHub repository `chakrit/piper` and configured remote `gh`.
- **Documentation**:
    - Initialized and expanded `README.md` with architecture and project structure.
    - Formalized **AOP Engine Specification** (`docs/spec/aop-engine.md`), aligning it with the Rust implementation (TOML, corpus, dynamic routing, and termination).
    - Added **Pipeline Design Specification** (`docs/spec/pipeline-design.md`), establishing the "Refining Funnel" hierarchy (Foundational $\rightarrow$ Structural $\rightarrow$ Superficial).
- **Pipeline Forking & DSL**:
    - Implemented a discrete Action-based DSL (`AspectAction`) for LLM responses, replacing fixed result structs. Actions include `Refine`, `AddKnowledge`, `EditKnowledge`, `Fork`, `Jump`, `Merge`, and `Terminate`.
    - Added `SessionContext` to track result sets across virtual forks, allowing them to be merged back into a main pipeline via the `Merge` action.
    - Enabled "virtual forks" where aspects can spin off secondary pipelines with their own goals and prompts.
- **Reliability & Recovery**:
    - Decoupled `LlmClient` from parsing; it now returns raw strings.
    - Implemented a recovery loop: if initial JSON parsing fails, the driver uses a recovery system prompt (`aspect-recover.md`) to ask the LLM to fix the formatting of the bad response.

## Working Tree Status
- Core refactor implemented and prompts aligned.
- `piper.toml` remains a local artifact.
- Visualizations of the flow stored in `docs/spec/`.
- Branch `main` is ahead of `gh/main` by 4 commits.

## Next Steps
- **Prompting**: Update `aspect-refiner.md` to use the new DSL format.
- **Deployment**: Push local commits to the `gh` remote.
- Verify end-to-end run using a test configuration.
- Stress-test dynamic pipeline routing via `next_aspect_id` and the new `Jump` action.
