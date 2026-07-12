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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum AspectAction {
    Refine {
        draft: String,
    },
    AddKnowledge {
        key: String,
        value: String,
    },
    EditKnowledge {
        key: String,
        value: String,
    },
    Fork {
        id: String,
        prompt: String,
        goal: String,
        draft: String,
        start_aspect_id: Option<String>,
    },
    Jump {
        id: String,
    },
    Merge {
        fork_ids: Vec<String>,
    },
    Terminate,
}

impl Default for AspectAction {
    fn default() -> Self {
        Self::Refine { draft: String::new() }
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

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LlmResponse {
    pub actions: Vec<AspectAction>,
    pub violations_found: u32,
}
