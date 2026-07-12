use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AspectType {
    #[default]
    Refiner,
    Gate,
}

impl AspectType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Refiner => "refiner",
            Self::Gate => "gate",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AspectAction {
    Refine(String),
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
            "REFINE" => Self::require_text(arguments).map(Self::Refine),
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
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Aspect {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub aspect_type: AspectType,
    pub prompt: String,
    pub model: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::AspectAction;

    #[test]
    fn test_parse_action_lines() {
        let cases = [
            (
                "REFINE Updated draft",
                AspectAction::Refine("Updated draft".to_string()),
            ),
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
                "DONE Final draft",
                AspectAction::Done("Final draft".to_string()),
            ),
        ];

        for (line, expected) in cases {
            let parsed = AspectAction::parse(line).expect("action should parse");
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn test_parse_action_lines_rejects_invalid_syntax() {
        let invalid = [
            "",
            "refine lower case",
            "REFINE\nsecond line",
            "ADD-KNOWLEDGE key",
            "UPDATE-KNOWLEDGE invalid/key value",
            "FORK branch",
            "JUMP missing argument",
            "MERGE branch,branch",
            "DONE",
            "{\"actions\":[]}",
        ];

        for line in invalid {
            assert!(AspectAction::parse(line).is_err(), "'{line}' should fail");
        }
    }
}
