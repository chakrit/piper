# Ledger — in-flight items

proposed · agent:inferred — **#7 Version identity & storage — my proposal, awaiting user.**
  Three coupled axes: (identity) content digest over the canonical serialized design +
  per-Blueprint ordered version number for human lineage — digest alone can't order;
  (storage) full immutable row per Version in PostgreSQL keyed `(blueprint_id, digest)`,
  append-only, deltas are a diff query not the stored form; (edit→new-Version) serialize new
  canonical doc → new digest → new row, pinned Versions immutable by construction. Mirrors
  the OCI-digest idiom already chosen for Sandbox snapshots. Open sub-question put to the
  user: what the digest is taken over — raw TOML bytes or a normalized canonical form (I lean
  canonical, so whitespace/key-order churn doesn't mint a spurious Version). No user answer
  yet — this is my derivation, withdraw if they redirect.

deferred · agent:inferred — **Spec-hygiene sweep: retire remaining old vocabulary in prose.**
  `anti-harness.md` body still uses `Request`/`Aspect`/uppercase-DSL (mapped by its term
  note, not rewritten); `dogfooding.md` says "Aspect position"; `run-lifecycle.md` is
  as-built (`Request`, `Aspect`) reconciled by migration #14. Out of #6's scope on purpose;
  a later sweep, not a design decision.

deferred · agent:inferred — **Live model smoke run.** Verify a real model drives a pipeline
  end to end through the neutral Provider port. Held from the AFK run (outward token spend +
  needs eyeballing); now downstream of the tool-call engine migration (OPEN-QUESTIONS #14).

deferred · agent:inferred — **Two audit borderlines (current code).** The interactive run
  selector silently skips runs that fail to load; `prompt_templates_for` collapses to
  embedded templates if a `--prompts` dir ever equals the project root. Minor; left as-is.
