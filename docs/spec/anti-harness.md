# Piper as an Anti-Harness

- **Status:** accepted
- **Scope:** Piper's design rationale and the boundary between model work and mechanical orchestration.

## The idea

Piper is an anti-harness: it deliberately gives the model less responsibility.

Many agent harnesses ask one general-purpose model to interpret a large instruction set,
remember workflow state, choose tools, coordinate branches, recover from mistakes, and
decide when the job is complete. That flexibility makes the model part of the control
plane. Success then depends on it consistently following orchestration instructions while
also doing the actual intellectual work.

Piper moves the control plane into ordinary code. The Rust driver owns ordering, routing,
branch isolation, joins, turn accounting, recovery, gates, and termination. A Worker sees
one Aspect and emits exactly one line of DSL. The model supplies judgment; Piper supplies
execution.

This does not make model output deterministic. It makes the workflow around that output
mechanical and inspectable. Given a valid command, the driver applies one defined state
transition rather than asking another model invocation to interpret what should happen
next.

## Reduce the job before improving the model

Piper treats a broad prompt as material to decompose, not as an instruction block every
Worker must master at once. A pipeline splits the goal into narrow Aspects such as checking
constraints, exploring alternatives, calculating an intermediate result, reviewing risk,
or synthesizing the final answer.

Each Worker therefore has a smaller task and a smaller instruction surface:

- the immutable Request includes the desired outcome and establishes what the run is for;
- the current Aspect asks one focused question;
- the Corpus supplies durable findings from earlier work;
- an optional fork directive narrows independent branch work; and
- the line DSL limits the response to one unambiguous action.

This is context engineering by construction. The driver does not rely on a model to
remember a long procedure or simulate a state machine in prose. Pipeline authors can tune
each Aspect and its model independently, giving a smaller or less-capable model a better
chance of succeeding because each pass demands less breadth. The advantage is not that
small models become universally capable; it is that the system stops spending their
limited attention on orchestration.

## Alignment by construction

A Worker cannot drift from the workflow because it is never given the room to. It sees one
Aspect, the immutable Request, the Corpus, and a one-line DSL; it does not hold the
procedure, choose the next step, or coordinate branches. Every decision the model could
get wrong about orchestration has already been made in code before the Worker runs.

This reduces the misalignment problem to a single step. A general harness asks the model
to stay aligned across an entire multi-step procedure it must remember and re-enact, so
the per-step failure probability compounds over every step. Piper asks only whether one
narrow Aspect produced one valid contribution; a whole-run misalignment would require the
same isolated, single-purpose call to fail in the same direction repeatedly, which the
structure makes unlikely rather than merely discouraged.

The same property relaxes the context budget. Because a Worker never carries the procedure
or sibling work, its prompt stays small, and a smaller or shorter-context model has enough
room to do the one job well. Making a step impossible to get structurally wrong and making
it fit a small context are the same design move.

## Request, Corpus, and Result

Piper keeps three concerns separate:

- The **Request** is the original input and remains immutable. Workers cannot quietly
  rewrite the problem as they process it.
- The **Corpus** is accumulated Knowledge. `SET` creates or replaces the current Worker's
  own contribution, and later Aspects receive the expanded Corpus.
- The **Result** is an explicit final synthesis produced with `DONE`. It is not confused
  with intermediate Knowledge.

This separation turns collaboration between Aspects into an observable data flow. An
early Worker records a fact once; a later Worker consumes that fact instead of being
expected to rediscover it or inherit it through conversational memory. Internally,
contributions retain branch and Aspect identity even though Workers do not manage keys.

## A deliberately small control language

The DSL is closer to a set of control signals than a programming language:

- `SET` contributes Knowledge and reruns the same Worker;
- `CONTINUE` or its exact alias `JOIN` completes the current transition;
- `JUMP` routes to another Aspect;
- `FORK` requests isolated work under a focused directive;
- `STOP` ends the run without a Result; and
- `DONE` returns the Result.

Every response is one command on one non-empty line. Piper rejects extra prose, malformed
arguments, and invalid state transitions. If formatting fails, it permits one focused
recovery call rather than letting an ambiguous response leak into the control plane.

`CONTINUE` and `JOIN` do not ask the Worker to choose between two execution policies. They
are names for the same context-sensitive transition. At the root, the transition joins
completed child Corpora and advances. In a fork, it joins any descendants and returns the
branch Corpus to its parent.

## Complex execution without a complex Worker

The driver provides richer behavior while preserving the small Worker contract:

- **Forks** copy the parent's starting Corpus into an isolated child. Child Knowledge is
  withheld from the parent until the parent joins all completed branches.
- **Joins** merge completed child Corpora mechanically. A Worker does not copy, reconcile,
  or address branch namespaces itself.
- **Routing** validates `JUMP` targets against configured Aspects and changes the pipeline
  position in code.
- **Bounds** apply one shared `max_turns` budget to the root and every fork, preventing a
  branch from escaping the run's execution limit.
- **Human gates** pause at explicit configured points for continue-or-abort authority.
  They cannot mutate the Request and do not consume Worker turns.
- **Completion** is explicit: `DONE` returns a Result, while `STOP` records why the run
  ended without one.

Forks currently execute synchronously. The important property is not parallel speed but
semantic isolation: independent Workers can produce findings without seeing sibling work,
then a later Aspect can synthesize the joined Corpus.

## The design boundary

Piper should use models where interpretation and judgment are valuable, and code where the
next action can be defined exactly. Adding more prose about workflow to every prompt moves
that boundary in the wrong direction. Adding a precise state transition, validation rule,
or focused Aspect moves it in the right direction.

The resulting engine is intentionally less magical than a general agent harness. Its
pipelines are explicit, its state is inspectable, and its failures can be exercised with
small deterministic test pipelines. That constraint is the point: autonomous execution
comes from the driver reliably applying a simple protocol, not from hoping a model will
faithfully reenact the protocol from a large prompt.
