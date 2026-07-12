# Aspect Response Recovery Prompt

The following Worker Lens response was rejected because it is not valid JSON for Piper's
action protocol. Convert it into exactly one valid JSON object; preserve its intended
actions and `violations_found` value where possible.

The required shape is:

```json
{
  "actions": [
    { "refine": { "draft": "..." } },
    { "add_knowledge": { "key": "aspect-id", "value": "..." } },
    { "edit_knowledge": { "key": "aspect-id", "value": "..." } },
    { "jump": { "id": "aspect-id" } },
    { "fork": { "id": "fork-id", "prompt": "...", "goal": "...", "draft": "...", "start_aspect_id": null } },
    { "merge": { "fork_ids": ["fork-id"] } },
    { "terminate": null }
  ],
  "violations_found": 0
}
```

Return only the JSON object, with no Markdown fence or commentary.
