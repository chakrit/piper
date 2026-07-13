#[derive(Debug, Clone, PartialEq)]
pub enum AspectAction {
    Next,
    AddKnowledge { key: String, value: String },
    UpdateKnowledge { key: String, value: String },
    Fork { id: String, prompt: String },
    Jump(String),
    Merge(Vec<String>),
    Done(String),
}

impl AspectAction {
    pub fn parse(response: &str) -> anyhow::Result<Self> {
        let line = response
            .strip_suffix("\r\n")
            .or_else(|| response.strip_suffix('\n'))
            .unwrap_or(response);
        if line.is_empty() || line.trim() != line || line.contains(['\r', '\n']) {
            anyhow::bail!("DSL response must be exactly one non-empty line")
        }

        let mut command = line.splitn(2, char::is_whitespace);
        let verb = command.next().expect("non-empty line has a verb");
        let arguments = command.next().map(str::trim_start).unwrap_or_default();

        match verb {
            "NEXT" => Self::require_no_arguments(arguments).map(|()| Self::Next),
            "ADD-KNOWLEDGE" => Self::parse_knowledge(arguments, true),
            "UPDATE-KNOWLEDGE" => Self::parse_knowledge(arguments, false),
            "FORK" => Self::parse_fork(arguments),
            "JUMP" => Self::parse_identifier(arguments, "aspect ID").map(Self::Jump),
            "MERGE" => Self::parse_fork_ids(arguments).map(Self::Merge),
            "DONE" => Self::require_text(arguments).map(Self::Done),
            _ => anyhow::bail!("unknown DSL verb '{verb}'"),
        }
    }

    fn parse_knowledge(arguments: &str, is_add: bool) -> anyhow::Result<Self> {
        let (key, value) = Self::split_identifier_and_text(arguments, "knowledge key")?;

        if is_add {
            Ok(Self::AddKnowledge { key, value })
        } else {
            Ok(Self::UpdateKnowledge { key, value })
        }
    }

    fn parse_fork(arguments: &str) -> anyhow::Result<Self> {
        let (id, prompt) = Self::split_identifier_and_text(arguments, "fork ID")?;

        Ok(Self::Fork { id, prompt })
    }

    fn parse_fork_ids(arguments: &str) -> anyhow::Result<Vec<String>> {
        let mut fork_ids = Vec::new();
        for id in arguments.split(',') {
            let id = Self::parse_identifier(id, "fork ID")?;
            if fork_ids.contains(&id) {
                anyhow::bail!("MERGE cannot name the same fork more than once")
            }
            fork_ids.push(id);
        }

        if fork_ids.is_empty() {
            anyhow::bail!("MERGE requires at least one fork ID")
        }
        Ok(fork_ids)
    }

    fn split_identifier_and_text(arguments: &str, name: &str) -> anyhow::Result<(String, String)> {
        let mut parts = arguments.splitn(2, char::is_whitespace);
        let identifier = parts.next().unwrap_or_default();
        let text = parts.next().map(str::trim_start).unwrap_or_default();
        let identifier = Self::parse_identifier(identifier, name)?;
        let text = Self::require_text(text)?;

        Ok((identifier, text))
    }

    fn parse_identifier(value: &str, name: &str) -> anyhow::Result<String> {
        let valid = !value.is_empty()
            && value.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            });
        if !valid {
            anyhow::bail!("{name} must use only ASCII letters, digits, hyphens, or underscores")
        }

        Ok(value.to_string())
    }

    fn require_text(value: &str) -> anyhow::Result<String> {
        if value.is_empty() {
            anyhow::bail!("DSL action requires a non-empty argument")
        }

        Ok(value.to_string())
    }

    fn require_no_arguments(value: &str) -> anyhow::Result<()> {
        if !value.is_empty() {
            anyhow::bail!("DSL action does not accept arguments")
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AspectAction;

    #[test]
    fn parses_each_command() {
        let cases = [
            ("NEXT", AspectAction::Next),
            (
                "ADD-KNOWLEDGE requirement Preserve compatibility",
                AspectAction::AddKnowledge {
                    key: "requirement".to_string(),
                    value: "Preserve compatibility".to_string(),
                },
            ),
            (
                "UPDATE-KNOWLEDGE requirement Preserve the stable API",
                AspectAction::UpdateKnowledge {
                    key: "requirement".to_string(),
                    value: "Preserve the stable API".to_string(),
                },
            ),
            (
                "FORK alternative-1 Explore the implementation option",
                AspectAction::Fork {
                    id: "alternative-1".to_string(),
                    prompt: "Explore the implementation option".to_string(),
                },
            ),
            ("JUMP audit", AspectAction::Jump("audit".to_string())),
            (
                "MERGE alternative-1,alternative-2",
                AspectAction::Merge(vec![
                    "alternative-1".to_string(),
                    "alternative-2".to_string(),
                ]),
            ),
            (
                "DONE Final result",
                AspectAction::Done("Final result".to_string()),
            ),
        ];

        for (line, expected) in cases {
            let parsed = AspectAction::parse(line).expect("valid action should parse");
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn accepts_one_trailing_line_ending() {
        let parsed =
            AspectAction::parse("DONE Final result\r\n").expect("line ending should parse");
        assert_eq!(parsed, AspectAction::Done("Final result".to_string()));
    }

    #[test]
    fn parses_next_without_arguments() {
        assert!(AspectAction::parse("NEXT").is_ok());
        assert!(AspectAction::parse("NEXT another aspect").is_err());
    }

    #[test]
    fn rejects_malformed_commands() {
        let invalid = [
            "",
            " NEXT",
            "NEXT trailing text",
            "refine lower case",
            "NEXT\nsecond line",
            "ADD-KNOWLEDGE key",
            "UPDATE-KNOWLEDGE invalid/key value",
            "FORK branch",
            "JUMP missing argument",
            "MERGE branch,branch",
            "MERGE branch,",
            "DONE",
            "{\"actions\":[]}",
        ];

        for line in invalid {
            assert!(AspectAction::parse(line).is_err(), "'{line}' should fail");
        }
    }
}
