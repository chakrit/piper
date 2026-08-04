use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PipelineId(String);

impl PipelineId {
    pub fn new(value: &str) -> Result<Self> {
        validate(value, "pipeline.id")?;
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn validate(value: &str, name: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if !valid {
        anyhow::bail!("{name} must use only ASCII letters, digits, hyphens, or underscores")
    }

    Ok(())
}
