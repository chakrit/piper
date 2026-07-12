# Aspect Definition Prompt

**Goal**: Transform a high-level objective into a sequence of modular Aspects for the
  Piper loop.

**Instructions**:
1. Analyze the user's goal.
2. Identify all critical constraints, requirements, and "deal-breakers".
3. Group these constraints into logical "Aspects".
4. Order the Aspects in a logical pipeline (Foundational -> Structural ->
   Superficial).

**Output Format**:
## Objective: [Brief summary]
## The Pipe
1. **[Aspect Name]**: [Short description of the lens and what it must enforce/remove]
2. **[Aspect Name]**: ...
...
## Rationale
[Explain why this order was chosen]
