# Research and synthesis with fan-out and fan-in

This guide builds a Piper pipeline that gives three independent research assignments to
small Worker calls, joins their findings into one Corpus, and asks a final Worker to write
a brief from all three contributions.

The pattern is useful when one large prompt would force a model to research, remember,
compare, and write at the same time. Piper keeps the original Request stable while
each Worker handles one bounded job. The driver, rather than an LLM, owns the execution
loop, branch isolation, joins, routing, and turn limit.

The example researches a fictional product launch so that its execution shape is easy to
inspect. Replace the three branch directives with real research questions for your domain.
Piper invokes the configured model; it does not itself browse or verify external facts, so
real factual research still requires an LLM command with the appropriate tools and a
validation policy.

## What the pipeline should do

Suppose the immutable Request, including its desired outcome, is:

> Assess whether Northstar, a team scheduling product, should launch in the fictional
> market of Alder, and produce a short brief grounded in customer, competitor, and
> operational evidence.

The pipeline gives the model three separate assignments:

1. Customer researcher: identify the strongest customer need and supporting evidence.
2. Competitor researcher: identify the closest alternative and a meaningful gap.
3. Operations researcher: identify the largest launch constraint and a mitigation.

Those branches start from the same Request and parent Corpus. They cannot see one
another's new findings while they work. After all three return, the parent joins their
Corpora. Only then does the synthesis Worker receive all findings as `KNOWLEDGE` directives.

The configured synthesis prompt deliberately contains no customer need, competitor, or
constraint. A successful brief therefore demonstrates that knowledge traveled through the
Corpus rather than being smuggled into the final Aspect's instructions.

## Create the pipeline

Save this as `pipelines/launch-research.toml` (or any path outside `piper.toml`):

```toml
[pipeline]
id = "research-synthesis"
max_turns = 14
max_recoveries = 2

[[pipeline.aspects]]
id = "research-plan"
name = "Plan and Perform Research"
prompt = "At root, launch exactly three independent forks: customer evidence, competitor evidence, and operational evidence. Give each fork a specific directive and use completed FORK history to avoid repeats. In a fork, answer only the Fork Directive, SET one compact finding containing the claim, evidence or source, and uncertainty, then return with CONTINUE or JOIN. At root after all three return, use their correlated FORK_KNOWLEDGE to SET one compact research bundle, then CONTINUE or JOIN to make every finding durable and advance."

[[pipeline.aspects]]
id = "synthesis"
name = "Launch Brief Writer"
prompt = "Use every research finding in the Corpus. Produce a brief with recommendation, evidence from all three research areas, unresolved uncertainty, and the next validation step. Do not invent missing evidence. Finish with DONE."
```

Piper configuration remains in the shared `piper.toml`. At minimum, its `[llm]` table must
provide the invocation, system prompt, and default model. This separation lets many small
pipeline files share one local model setup.

The `research-plan` Aspect owns the complete fan-out/fan-in operation:

- At root, it launches branches and later receives their correlated pending results as
  `FORK_KNOWLEDGE`.
- In a child, `Fork Directive` contains that branch's assignment. The same Aspect records
  its answer with `SET`, is run again with its contribution as `KNOWLEDGE`, and returns.
- Once every result returns, the root can consolidate them with its own `SET` before joining
  and advancing to synthesis.

Piper supplies `CONTEXT research-plan` at root and paths such as `CONTEXT
research-plan/customer` inside forks. The Worker template also explains the parent and fork
instructions, so the model does not infer its branch role from prose alone.

## Run it

From the repository root:

```sh
cargo run -- --file piper.toml run pipelines/launch-research.toml \
  "Assess whether Northstar should launch in Alder and produce an evidence-grounded brief."
```

For an installed binary, replace `cargo run --` with `piper`:

```sh
piper --file piper.toml run pipelines/launch-research.toml \
  "Assess whether Northstar should launch in Alder and produce an evidence-grounded brief."
```

Use `--prompts path/to/prompts` after the request if the Worker templates are not in the
default `prompts/` directory.

## Follow the execution

Exact Worker wording varies, but the command sequence should have this shape:

```text
research-plan (root): FORK customer Find the strongest customer need and evidence.
research-plan/customer: SET Customer finding: ...
research-plan/customer: JOIN

research-plan (root): FORK competitor Find the closest alternative and a meaningful gap.
research-plan/competitor: SET Competitor finding: ...
research-plan/competitor: CONTINUE

research-plan (root): FORK operations Find the largest launch constraint and a mitigation.
research-plan/operations: SET Operations finding: ...
research-plan/operations: JOIN

research-plan (root): SET Research bundle: ...all three pending findings...
research-plan (root): CONTINUE
synthesis (root): DONE ...final brief using all three findings...
```

`CONTINUE` and `JOIN` are exact aliases. Their effect depends on execution context:

- In a child, either command first joins any descendants and then returns the child's
  Corpus to its parent.
- In the root, either command first joins completed child Corpora and then advances to the
  next Aspect.
- With no completed forks, the root simply advances.

Choose the word that reads naturally; there is no semantic decision between them. The
example intentionally uses both to show that they share one transition.

Forks are completed synchronously in the current engine. The planner is rerun after each
child returns and sees both `FORK <id> <directive>` history and correlated
`FORK_KNOWLEDGE <id> <value>`. Those pending results are visible only to the parent; a new
sibling still starts from the unchanged parent Corpus. This prevents the second researcher
from merely echoing the first while allowing the parent to consolidate results in the same
Aspect. When the planner finally emits `CONTINUE` or `JOIN`, Piper merges every completed
child Corpus, clears the fork history, and advances.

## Why small contexts help

Each branch call receives the stable Request, one focused Aspect prompt, its
single branch directive, and the Corpus available when the branch began. It does not need
the other researchers' scratch work or the eventual writing instructions. That narrower
job is more tractable for a smaller model and makes failures easier to attribute.

The final writer receives compact findings rather than full research conversations. This
is the central fan-out/fan-in trade: branch Workers spend their context on independent
analysis; the synthesis Worker spends its context on comparison and writing. Piper carries
the facts between them mechanically as Corpus entries.

Keep contributions compact. `SET` does not append another note from the same Aspect and
branch; it replaces that Worker's existing contribution and reruns the Worker. A useful
research contribution usually includes:

- a specific claim;
- the evidence or source supporting it;
- uncertainty or a caveat;
- any consequence the synthesis Worker must preserve.

## Set a realistic turn budget

`max_turns` is shared by the root and every branch. Every normal Worker pass consumes one
turn. In this example, the expected minimum is twelve turns:

- five root research passes: three `FORK` commands, one consolidating `SET`, and one join;
- six child passes: one `SET` and one return for each of three branches;
- one synthesis pass.

Therefore `max_turns = 14` leaves two turns for a Worker that refines its own contribution
with another `SET`. Parser recovery is an extra model call within the same Worker pass and
does not consume another turn, but it consumes the separate shared `max_recoveries` budget
and still uses model tokens or provider quota.

Do not set a large limit merely to make loops disappear. Estimate the intended command
sequence, allow a small refinement margin, and treat exhaustion as evidence that an Aspect
prompt or transition is unclear. For metered models, tests should use a deterministic fake
client; reserve real runs for deliberate dogfooding and record both ordinary and recovery
calls.

## Validate information flow

A polished brief is not enough to prove the engine behaved correctly. Test the data path
with facts that have exact, machine-checkable identities.

For a deterministic test variant, direct the three branches to produce unique markers such
as `CUSTOMER-ALDER-17`, `COMPETITOR-BRASS-29`, and `OPERATIONS-PORT-41`. Keep those markers
out of the synthesis Aspect's configured prompt. Then make the fake LLM inspect each prompt
and reject an unexpected state:

1. A child prompt contains only its assigned marker and the Corpus copied at fork time.
2. Later sibling prompts do not contain earlier markers.
3. The planner's root prompt contains every marker as correlated `FORK_KNOWLEDGE`, but not
   as ordinary joined `KNOWLEDGE`.
4. The synthesis prompt contains all three markers as `KNOWLEDGE`.
5. The text passed to `DONE` contains every marker exactly as required.

Also run the repository checks after changing engine behavior or fixtures:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

For real research, add domain validation on top of the transport checks: require source
identifiers in each contribution, reject unsupported claims, check dates and units, and
have the synthesis call expose uncertainty instead of filling gaps. These checks belong in
focused Aspects or test assertions, not in an assumption that a joined Corpus is correct.

## Common failure modes

Repeated forks usually mean the planner ignored its `FORK` history. Tell it the exact set
of assignments and require unique IDs. Piper rejects a duplicate fork ID from the same
Worker, but a model can still waste turns inventing new IDs for the same question.

Missing findings at synthesis usually mean the parent has not joined. A completed result is
visible to the parent as pending `FORK_KNOWLEDGE`, but it is not ordinary Corpus knowledge
until the parent emits `CONTINUE` or `JOIN`. Attempting `JUMP`, `STOP`, or `DONE` with
unjoined fork work is rejected rather than silently discarding it.

A child that fails to return keeps rerunning the research Aspect. Its second pass should
recognize its own contribution in `KNOWLEDGE` and emit `CONTINUE` or `JOIN`. In a child,
that command returns to the parent; it does not advance to the next Aspect.

An endless refinement loop means a Worker keeps emitting `SET`. Since `SET` replaces its
own contribution and reruns the same Aspect, the prompt must define when the finding is
complete and when to return. The shared turn budget is the final mechanical guard.

Confident but weak synthesis is a research-quality problem, not a join problem. Corpus
membership proves that knowledge was transported, not that it was true. Preserve source
and uncertainty information in branch contributions and validate them before treating the
final result as evidence-based.
