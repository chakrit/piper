# Pipeline Design Specification

## Philosophy
Aspect-Oriented Prompting (AOP) requires the decomposition of alignment constraints into isolated lenses. To avoid instruction drift and ensure high-fidelity results, pipelines should be designed as a "refining funnel."

## The Funnel Hierarchy
Aspects must be ordered by the scope of their impact, moving from the most fundamental to the most superficial:

1. **Foundational Aspects**:
   - **Focus**: Core correctness, logic, safety, and factual accuracy.
   - **Goal**: "Is the content correct and safe?"
   - **Example**: `security-audit`, `fact-check`, `logic-validation`.

2. **Structural Aspects**:
   - **Focus**: Organization, flow, architecture, and adherence to specifications (RFCs, ADRs).
   - **Goal**: "Is the content structured correctly?"
   - **Example**: `spec-alignment`, `modular-decomposition`, `flow-optimization`.

3. **Superficial Aspects**:
   - **Focus**: Voice, tone, brand alignment, typography, and polish.
   - **Goal**: "Does the content sound and look right?"
   - **Example**: `brand-voice`, `conciseness-filter`, `formatting-pass`.

## Design Principles
- **Isolation**: Each Aspect should manage exactly one dimension of alignment. If an Aspect is trying to fix both "security" and "grammar," it should be split.
- **Idempotency**: Applying an Aspect multiple times to the same draft should not degrade the output.
- **Additive Knowledge**: Use the `corpus` to pass discoveries (e.g., a Foundational Aspect identifying a specific bug) to later Aspects so they can enforce the fix without re-discovering the bug.

## Design Workflow
1. **Constraint Extraction**: Identify all "deal-breakers" and requirements.
2. **Clustering**: Group constraints into Aspects based on the hierarchy (Foundational $\rightarrow$ Structural $\rightarrow$ Superficial).
3. **Ordering**: Sequence Aspects to ensure that a fix in a Foundational layer is not accidentally reverted by a Superficial layer.
