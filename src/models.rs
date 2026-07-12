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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Aspect {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub aspect_type: AspectType,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LlmResponse {
    pub refined_draft: String,
    pub violations_found: u32,
    pub next_aspect_id: Option<String>,
}
