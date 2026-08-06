# The Supervisor

- **Status:** draft
- **Scope:** the interactive control layer over a Run, and the one boundary it may
  never cross.

## What the Supervisor is

The Supervisor is the control seat over a Run. It is co-occupied: a human and a model
together. The human holds intent, notices misalignment, and gives the go; the model turns
intent into concrete edits ("this step is too vague, tighten it toward X"), reasons about
the Run, and bounces ideas before anything is done. Neither occupant works alone — the
model has no authority without the human's word, and the human cannot reshape the Run by
hand without the model as the surface. This is the same relationship a person has with an
assistant while designing: think together, commit nothing except through a deliberate act.

The Supervisor is how a human steers a Run without reading raw state. Stop it, rewind it,
fork it, and — centrally — build, tune, and tear down its steps, all by talking to the
Supervisor.

## What a step is

Piper is a general factory-harness. A step is any task a normal harness performs — write
code, run tests, use git, reason over context — and a Worker is that step fired, a
harnessed model doing the task. A step's work lands as some mix of:

- **Effects** in the world — files written, commits made, commands run.
- **Knowledge** in the Corpus — the shared-context channel the next steps read.

A step may produce both, or effects with no Knowledge at all (a pure-effect step: commit,
run the suite). Knowledge is one output channel, not the definition of a step's work. What
defines a Worker is that it *does the task*, with whatever effects that entails.

## Worker and Supervisor

A Worker acts in the **work plane** — the codebase, git, commands, the Corpus — the
substance the task operates on. The Supervisor acts in the **Blueprint plane** — a step's
prompt, settings, and the Run's topology — the factory's design.

The Supervisor never staffs a step. It configures them. Building a new step, retuning a
step's prompt, deleting a step, rewiring the order — that is the Supervisor's whole job,
because a step *is* design and adjusting the Run *is* editing that design through
conversation. Workers run the steps; the Supervisor shapes the steps and drives the Run.

## The boundary

The line is not "the Supervisor may not touch the step." Touching the step — editing what
it is — is the Supervisor's purpose. The line is one notch deeper:

> **Configure the machine, never be the machine.** The Supervisor may author a step's
> prompt, settings, and the Run's topology. It may never perform a step's work — never
> write the code, make the commit, run the command, or emit the Knowledge that firing the
> step would produce.

Editing what a step *is* stays in the Blueprint plane. Performing what a step *does* is
the work plane, and the work plane belongs only to a fired Worker. The Supervisor changes
the conditions; the work happens only by running a step under those conditions.

## Provenance

Every effect and output of a step — a file written, a commit made, a command run,
Knowledge added to the Corpus — traces to a Worker execution: a step, fired, its model
invoked. None originates from the Supervisor's own action. The Supervisor acts only in the
Blueprint plane and on Run controls; it never acts in the work plane. Design and control
flow from the Supervisor; work flows only from a step firing.

This is what keeps a model in the control seat from cheating the pipeline on a step's
behalf: not a rule it is asked to honor, but a plane it cannot reach. The Supervisor's
edits are visible design the human co-authorizes; a step's work is performed by a
driver-sequenced Worker. The two never merge into a path where the Supervisor's own
action reaches the work plane directly.

## Two toolsets

The boundary is drawn by capability, not by stance — and with tool-calling Workers
([`engine.md`](engine.md)) the capability split *is* a difference in which tools each role
holds:

- **Worker toolset** — the **work tools** (read/write files, run commands, git) plus the
  **control-flow tools** (`set`, `continue`, `jump`, `fork`, `stop`, `done`). Only a fired
  Worker holds these; they are how a Station's effects and Knowledge enter the work plane.
- **Supervisor toolset** — Blueprint edits (create / edit / delete Stations, settings,
  topology) and Run controls (stop, rewind, fork, inspect). It holds **no work tools**.

Because the split is which tools exist for the role, a Supervisor performing step work is
not a policy violation to detect after the fact — it is unrepresentable: the Supervisor's
toolset simply contains no work tool, so its generation can never reach the work plane.
This is why the boundary holds while Piper itself is built with a model in the Supervisor
seat.

## Human-triggered

Every Supervisor action originates from a human instruction. The Supervisor holds control
capabilities; it does not exercise them autonomously. The capability surface scopes *what*
it may touch (the Blueprint plane and Run controls, never the work plane); the
human-triggered rule scopes *when* it acts (on the human's word, never on its own).
