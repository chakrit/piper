# Aspect Response Recovery Prompt

The following Worker Lens response was rejected because it is not one valid Piper DSL
command. Convert it into exactly one valid command line and preserve its intended effect.

The required shape is:

```text
REFINE updated draft
ADD-KNOWLEDGE key durable constraint
UPDATE-KNOWLEDGE key corrected constraint
JUMP aspect-id
FORK fork-id branch directive
MERGE fork-id,other-fork-id
DONE final draft
```

Return only one command line, with no Markdown fence or commentary.
