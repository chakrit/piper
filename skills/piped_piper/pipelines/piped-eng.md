# Aspect Package: Piped-Eng (The Rigour Loop)

**Goal**: Ensure software changes are strictly aligned with specifications,
  verified by TDD, and clean of implementation violations.

## The Pipeline

Apply these Aspects in the following order:

### 1. [Spec-Alignment]
**Lens**: The Source of Truth.
- **Check**: Does the current draft match the project's specifications, RFCs, or
  ADRs?
- **Enforce**: Acceptance criteria must be explicitly met.
- **Prune**: Remove any "feature creep" or logic that isn't requested by the spec.
- **Goal**: A draft that is architecturally and functionally correct according to
  the design.

### 2. [TDD-Compliance]
**Lens**: The Verification Loop.
- **Check**: Is there a failing test that proves the need for this change?
- **Enforce**: The "Red $\rightarrow$ Green $\rightarrow$ Refactor" sequence.
- **Prune**: Remove any implementation code that doesn't have a corresponding test
  case.
- **Goal**: A draft where every line of logic is justified by a passing test.

### 3. [Implementation Rigor]
**Lens**: The Quality Standard.
- **Check**: Is this the "elegant just-enough" solution?
- **Enforce**: `general-coding` skill rules (naming, type safety, minimal nesting).
- **Prune**: Over-engineered abstractions, redundant comments, and "magic" logic.
- **Goal**: Production-grade, maintainable code.

### 4. [Compliance Audit]
**Lens**: The Zero-Violation Filter.
- **Check**: Re-read the full file. Are there any clear rule violations?
- **Enforce**: Zero "Violations" (as defined by ACE). a "Violation" blocks landing.
- **Prune**: Artifacts of the iterative process, debug logs, or inconsistent
  styling.
- **Goal**: A commit-ready diff that passes a principal engineer's audit.
