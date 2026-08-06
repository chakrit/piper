# Spec & architecture

**Current-understanding durable artifacts** — the design of the project and how it
actually fits together: design specs, RFCs, interface contracts, architecture / "how it
works" overviews, *and our own exact surface* (our CLI flags, config keys, API, schemas).
Prose you read to understand the system, plus the lookup facts about our own thing.
Updated in place; always reflects present design, not history.

**This is the home for everything settled** — an instruction you were given, an approach
that was agreed, a library that was picked, a convention or preference that was fixed. All
of it lands here as an amendment, written as it was given: the rule, at the length it was
given, with no reason supplied and no note of what it was chosen over. There is no
separate ruling artifact to reach for.

**This spec is authoritative — read it before you work, and comply.** It owes your priors
no justification. That a rule departs from mainstream practice or from what you expected
is not grounds to escalate or re-open it. If you think it's wrong, raise it and amend it
here.

A *third-party* surface (a framework's API, another product's flags) is `../vendor/`.
Research, a survey, or a draft is `../scratch/`.

## Index

One row per file in this folder. **Keep it current** — adding, renaming, or retiring a
spec file updates its row in the same change. Read this table before you add a file: if a
doc on the subject already exists, amend that doc instead of writing a second one.

| File                 | Covers                                                                   | Status   |
|----------------------|--------------------------------------------------------------------------|----------|
| `glossary.md`        | The one vocabulary — every term for code, UI, and docs                   | draft    |
| `anti-harness.md`    | Design rationale: why Piper moves orchestration into code, not the model | accepted |
| `dogfooding.md`      | Self-dogfooding goal, Claude Worker engine, and the phased dev path       | accepted |
| `blueprint.md`       | Blueprint template, Versions, Stations, and the Run instance data model  | draft    |
| `blueprint-design.md`| Authoring Station sequences: ordering, principles, entrypoints, workflow  | draft    |
| `engine.md`          | Driver, the Station tool-call loop, Worker toolset, Corpus, and Forks     | draft    |
| `providers.md`       | The provider-neutral model port, capability map, and refreshable index    | draft    |
| `supervisor.md`      | The interactive control seat and the Blueprint-vs-work-plane boundary     | draft    |
| `control-api.md`     | The engine↔client contract: RESTful operations, resource model, transport | draft    |
| `interface.md`       | The world model, floating chrome, and the design/run mode separation     | draft    |
| `run-lifecycle.md`   | Durable run identity, storage, checkpointing, CLI lifecycle, sandbox      | accepted |
| `OPEN-QUESTIONS.md`  | The decision queue that gates development — emptied before building       | queue    |

## Format

One file per subject: `<slug>.md` (no date prefix — describes a thing, not the moment it
was written). Add a status header (`draft`, `accepted`, `superseded`, `implemented`) so
readers can tell whether it still describes current design.
