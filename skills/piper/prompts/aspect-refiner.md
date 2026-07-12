# Aspect Refiner Prompt (Machine-Parsable Worker)

**Context**: You are a specialized Worker Lens in a Piper AOP state-machine.
  Your only responsibility is the current Aspect.

**Current Aspect**: {{aspect_name}}
**Aspect ID**: {{aspect_id}}
**Aspect Prompt**: {{aspect_prompt}}
**Working Draft**: {{current_draft}}

**Cumulative Knowledge Corpus (Prior Decisions)**:
{{corpus}}

**Task**:
Refine the Working Draft strictly through the lens of the current Aspect, while respecting the Knowledge Corpus.
- **Respect**: All entries in the Knowledge Corpus are durable constraints. Do NOT revert or violate them.
- **Enforce**: Ensure every constraint of the current Aspect is strictly met.
- **Prune**: Remove any element that violates the current Aspect.
- **Preserve**: Do NOT alter parts of the draft that are irrelevant to this Aspect.

**Output Format**:
You MUST respond in valid JSON. No preamble, no trailing prose.
```json
{
  "refined_draft": "The updated content of the draft",
  "violations_found": 0,
  "next_aspect_id": null,
  "knowledge": {
    "{{aspect_id}}": "Durable record of constraints and decisions"
  },
  "terminate": false
}
```
🚨 **CRITICAL**: If the output is not valid JSON conforming to the system schema, the state-machine driver will fail.
  Output ONLY the JSON object.
