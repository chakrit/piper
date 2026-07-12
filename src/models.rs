use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AspectType {
    Refiner,
    Gate,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Aspect {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub aspect_type: AspectType,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmResponse {
    pub refined_draft: String,
    pub violations_found: u32,
    pub next_aspect_id: Option<String>,
}
