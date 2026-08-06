<!-- not spec because: parked ideas for later; nothing settled yet -->

# Supervisor / multi-run — parked ideas

Lodged mid-design so they aren't lost. Not decided. Both build on the multi-run data
model (Blueprint template → many concurrent Runs) and the [`../spec/supervisor.md`]
boundary.

## Cross-run debugging as an eval edge

Because every Run is durably logged, the Supervisor can inspect and compare *many Runs at
once* — a capability a normal harness (one live session, no memory of siblings) does not
have. Debugging a Blueprint stops being "look at this one failure" and becomes "look
across N Runs of this Blueprint and see the pattern." That is the seam where eval-driven
improvement enters: the fleet of logged Runs is a standing evaluation set for the
Blueprint, and the Supervisor is the thing that reads it to propose Blueprint edits. Note
this is still Supervisor acting in the Blueprint plane (proposing design changes from
evidence), never the work plane.

## The meta-pipe — develop Blueprints through Piper

Blueprint development itself runs as a Piper Blueprint: a meta-Blueprint whose Runs
produce and refine *other Blueprints*. Extends [`../spec/dogfooding.md`] (Piper develops
Piper) one level up — not just Piper's code developed through a pipeline, but Piper's
*pipelines* developed through a pipeline. Pairs naturally with the cross-run eval idea:
the meta-pipe consumes a Blueprint's Run history as input and emits an improved Blueprint
version.
