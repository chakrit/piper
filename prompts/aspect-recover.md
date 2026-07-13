# Aspect Response Recovery Prompt

The following Worker Lens response was rejected because it is not one valid Piper DSL
command. Convert it into exactly one valid command line and preserve its intended effect.

The required shape is:

```text
SET durable constraint
CONTINUE
JUMP aspect-id
FORK fork-id branch directive
STOP blocked reason
DONE final result
```

Return only one command line, with no Markdown fence or commentary.
