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
max_turns = 14
max_recoveries = 2

[[pipeline.aspects]]
id = "research-plan"
name = "Research Planner"
prompt = "Launch exactly three independent forks: customer evidence, competitor evidence, and operational evidence. Give each fork a specific directive. Use completed FORK directives to avoid repeating an assignment. After all three are complete, join them."

[[pipeline.aspects]]
id = "evidence"
name = "Evidence Researcher"
prompt = "When Fork Directive is present, answer only that assignment. SET one compact finding containing the claim, its evidence or source, and any uncertainty; once that contribution is visible as KNOWLEDGE, return to the parent. When no Fork Directive is present, do not add a finding; continue to synthesis."

[[pipeline.aspects]]
id = "synthesis"
name = "Launch Brief Writer"
prompt = "Use every research finding in the Corpus. Produce a brief with recommendation, evidence from all three research areas, unresolved uncertainty, and the next validation step. Do not invent missing evidence. Finish with DONE."
```

Piper configuration remains in the shared `piper.toml`. At minimum, its `[llm]` table must
provide the invocation, system prompt, and default model. This separation lets many small
pipeline files share one local model setup.

The `evidence` Aspect has two roles because a child starts at the Aspect immediately after
the one that forked it:

- In a child, `Fork Directive` contains that branch's assignment. The Worker records its
  answer with `SET`, is run again with its contribution now present as `KNOWLEDGE`, and then
  returns to the parent.
- In the root run, there is no fork directive. After the parent joins, the root advances
  through `evidence`; that pass simply continues to `synthesis` without creating a fourth
  finding.

This distinction belongs in the Aspect prompt. Piper provides the branch directive, but it
does not infer different jobs for root and child executions of the same Aspect.

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
evidence (customer):  SET Customer finding: ...
evidence (customer):  JOIN

research-plan (root): FORK competitor Find the closest alternative and a meaningful gap.
evidence (competitor): SET Competitor finding: ...
evidence (competitor): CONTINUE

research-plan (root): FORK operations Find the largest launch constraint and a mitigation.
evidence (operations): SET Operations finding: ...
evidence (operations): JOIN

research-plan (root): CONTINUE
evidence (root):       CONTINUE
synthesis (root):      DONE ...final brief using all three findings...
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
child returns and sees a `FORK <id> <directive>` history entry, but it does not yet see that
child's new knowledge. This isolation prevents the second researcher from merely echoing
the first. When the planner finally emits `CONTINUE` or `JOIN`, Piper merges every completed
child Corpus, clears the fork history, and advances.

## Why small contexts help

Each evidence call receives the stable Request, one focused Aspect prompt, its
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

- four planner passes: three `FORK` commands and one join/continue;
- six child passes: one `SET` and one return for each of three branches;
- one root pass through `evidence`;
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
2. The planner's prompt after a child returns contains that child's `FORK` history but not
   its marker as `KNOWLEDGE`.
3. The synthesis prompt contains all three markers as `KNOWLEDGE`.
4. The text passed to `DONE` contains every marker exactly as required.

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

Missing findings at synthesis usually mean the parent has not joined. A completed child is
listed in parent fork history while its new Corpus remains hidden. The parent must emit
either `CONTINUE` or `JOIN`; attempting `JUMP`, `STOP`, or `DONE` with unjoined fork work is
rejected rather than silently discarding it.

A child that reaches synthesis has failed to return from `evidence`. Its second pass should
recognize its own contribution in `KNOWLEDGE` and emit `CONTINUE` or `JOIN`. In a child,
that command returns to the parent; it does not advance to the next Aspect.

An unwanted fourth finding means the evidence prompt did not distinguish root execution
from fork execution. Require a non-empty Fork Directive before researching and make the
no-directive path continue without `SET`.

An endless refinement loop means a Worker keeps emitting `SET`. Since `SET` replaces its
own contribution and reruns the same Aspect, the prompt must define when the finding is
complete and when to return. The shared turn budget is the final mechanical guard.

Confident but weak synthesis is a research-quality problem, not a join problem. Corpus
membership proves that knowledge was transported, not that it was true. Preserve source
and uncertainty information in branch contributions and validate them before treating the
final result as evidence-based.
