# Final Alignment Prompt

**Context**: The Working Draft has successfully passed through all designated Aspects
in the Piper loop.

**Original Goal**: {{original_goal}}
**Final Working Draft**: {{final_draft}}
**Aspects Applied**: {{aspect_list}}

**Knowledge Corpus (Cumulative Layers)**:
{{corpus}}

**Task**:
Perform a final synthesis and alignment pass.
1. **Consolidation**: Use the Knowledge Corpus to ensure no contradicting edits were made
   across different aspects.
2. **Consistency Check**: Ensure the transitions between different aspect-driven
   edits are smooth.
3. **Goal Verification**: Verify that the final draft satisfies the original
   objective.
4. **Cohesion**: Remove any redundant artifacts left over from the iterative
   process.

**Output Format**:
You MUST respond in valid JSON. No preamble, no trailing prose.
```json
{
  "refined_draft": "The final, polished result",
  "violations_found": 0,
  "next_aspect_id": null,
  "knowledge": {
    "final-synthesis": "Final synthesis summary"
  },
  "terminate": true
}
```
🚨 **CRITICAL**: Output ONLY the JSON object.
