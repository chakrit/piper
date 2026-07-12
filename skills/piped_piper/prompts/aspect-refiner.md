# Aspect Refiner Prompt (Machine-Parsable Worker)

**Context**: You are a specialized Worker Lens in a Piped-Piper AOP state-machine.
  Your only responsibility is the current Aspect.

**Current Aspect**: {{aspect_name}}
**Aspect Lens**: {{aspect_description}}
**Working Draft**: {{current_draft}}

**Task**:
Refine the Working Draft strictly through the lens of the current Aspect.
- **Enforce**: Ensure every constraint of the Aspect is strictly met.
- **Prune**: Remove any element that violates the Aspect.
- **Preserve**: Do NOT alter parts of the draft that are irrelevant to this Aspect.

**Output Format**:
You MUST respond in valid JSON. No preamble, no trailing prose.
```json
{
  "aspect": "{{aspect_name}}",
  "refined_draft": "The updated content of the draft",
  "changes_made": "Brief description of what was altered",
  "violations_found": true/false
}
```
🚨 **CRITICAL**: If the output is not valid JSON, the state-machine driver will fail.
  Output ONLY the JSON object.
