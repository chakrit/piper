use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AspectType {
    #[default]
    Worker,
    Gate,
}

impl AspectType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Worker => "worker",
            Self::Gate => "gate",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Aspect {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "type")]
    pub aspect_type: AspectType,
    pub prompt: String,
    pub model: Option<String>,
}
