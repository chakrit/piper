# Glossary

- **Status:** draft
- **Scope:** the one vocabulary for Piper, used identically in code, UI, and docs.

Every term below is the single name for its concept. It is the identifier in code, the
label in the UI, and the word in prose. One concept, one word — no synonyms, no
code-vs-UI split. A rename anywhere is a rename everywhere.

| Term          | Is                                                                        |
|---------------|--------------------------------------------------------------------------|
| **Blueprint** | a reusable process template: a canvas of Stations with named entrypoints. Repo-agnostic. |
| **Version**   | an immutable snapshot of a Blueprint's design. A Run pins one.            |
| **Station**   | one step in a Blueprint: a prompt + model settings + toolset (a mini-harness). |
| **Entrypoint**| a named Station a Run may start at. A Blueprint has one or more.          |
| **Worker**    | a Station fired in a Run — a harnessed model doing the step's work.       |
| **Run**       | one execution instance of a Blueprint: a pinned Version + Order + its own work plane. Many concurrent per Blueprint. |
| **Order**     | the immutable input a Run fulfills (formerly "Request").                  |
| **Knowledge** | one Worker's contribution to the Corpus.                                  |
| **Corpus**    | the shared, accumulated Knowledge within one Run.                         |
| **Sandbox**   | the isolated execution environment realizing a Run's work plane — one per Run, copy-on-write forkable, backed by Dagger. |
| **Driver**    | the deterministic sequencer that moves a Run Station-to-Station.          |
| **Provider**  | the model-facing adapter that drives one model over a standardized wire protocol (OpenAI-compatible). Distinct from the Driver. |
| **Registry**  | the refreshable capability map of available models, projected from an external catalog. |
| **Gate**      | a Station where a Run halts for a human decision.                        |
| **Fork**      | an isolated branch within a Run, with its own temporary Corpus.          |
| **Supervisor**| the interactive control seat over a Run — a human and a model together. See [`supervisor.md`](supervisor.md). |

## Two words that are NOT interchangeable

- **plane** — a *capability* domain, from [`supervisor.md`](supervisor.md): the **Blueprint
  plane** (design/config) and the **work plane** (code, git, commands, Corpus). Names what
  a role may touch.
- **world** — a *visual* layer in the interface, from [`interface.md`](interface.md): the
  one canvas beneath the floating chrome. A Blueprint in design mode and a Run in run mode
  are two worlds you slide between.

They sound alike and mean different things. Keep them distinct.

## Renames from the prior vocabulary

The earlier engine specs used **Request → Order**, **Aspect → Station**, and **Pipeline →
Blueprint**. The old terms are retired; nothing new uses them.

**"Operator" is not a term.** The control seat is the **Supervisor**. The lowercase
"operator" still in the implemented [`run-lifecycle.md`](run-lifecycle.md) means the human
CLI user (the human half of the Supervisor) and is reconciled during the migration.
