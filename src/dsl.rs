use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AspectAction {
    Set(String),
    Continue,
    Fork { id: String, prompt: String },
    Jump(String),
    Stop(String),
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
            "SET" => Self::require_text(arguments).map(Self::Set),
            "CONTINUE" | "JOIN" => Self::require_no_arguments(arguments).map(|()| Self::Continue),
            "FORK" => Self::parse_fork(arguments),
            "JUMP" => Self::parse_identifier(arguments, "aspect ID").map(Self::Jump),
            "STOP" => Self::require_text(arguments).map(Self::Stop),
            "DONE" => Self::require_text(arguments).map(Self::Done),
            _ => anyhow::bail!("unknown DSL verb '{verb}'"),
        }
    }

    fn parse_fork(arguments: &str) -> anyhow::Result<Self> {
        let (id, prompt) = Self::split_identifier_and_text(arguments, "fork ID")?;

        Ok(Self::Fork { id, prompt })
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
        crate::identifier::validate(value, name)?;

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
            (
                "SET Preserve compatibility",
                AspectAction::Set("Preserve compatibility".to_string()),
            ),
            ("CONTINUE", AspectAction::Continue),
            ("JOIN", AspectAction::Continue),
            (
                "FORK alternative-1 Explore the implementation option",
                AspectAction::Fork {
                    id: "alternative-1".to_string(),
                    prompt: "Explore the implementation option".to_string(),
                },
            ),
            ("JUMP audit", AspectAction::Jump("audit".to_string())),
            (
                "STOP Missing security requirements",
                AspectAction::Stop("Missing security requirements".to_string()),
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
    fn parses_continue_without_arguments() {
        assert!(AspectAction::parse("CONTINUE").is_ok());
        assert!(AspectAction::parse("CONTINUE another aspect").is_err());
    }

    #[test]
    fn parses_join_as_continue() {
        let parsed = AspectAction::parse("JOIN").expect("JOIN should parse");

        assert_eq!(parsed, AspectAction::Continue);
        assert!(AspectAction::parse("JOIN child").is_err());
    }

    #[test]
    fn parses_set_stop_and_continue() {
        assert!(AspectAction::parse("SET Preserve compatibility").is_ok());
        assert!(AspectAction::parse("STOP Missing security requirements").is_ok());
        assert!(AspectAction::parse("CONTINUE").is_ok());
    }

    #[test]
    fn rejects_legacy_commands() {
        let legacy = [
            "NEXT",
            "ADD-KNOWLEDGE key value",
            "UPDATE-KNOWLEDGE key value",
            "MERGE branch",
        ];

        for line in legacy {
            assert!(AspectAction::parse(line).is_err(), "'{line}' should fail");
        }
    }

    #[test]
    fn rejects_malformed_commands() {
        let invalid = [
            "",
            " NEXT",
            "CONTINUE trailing text",
            "refine lower case",
            "CONTINUE\nsecond line",
            "SET",
            "STOP",
            "FORK branch",
            "JUMP missing argument",
            "DONE",
            "{\"actions\":[]}",
        ];

        for line in invalid {
            assert!(AspectAction::parse(line).is_err(), "'{line}' should fail");
        }
    }
}
