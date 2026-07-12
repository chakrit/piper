use crate::models::Aspect;
use anyhow::Result;
use std::collections::HashMap;

pub struct SessionContext {
    pub fork_results: HashMap<String, String>,
}

impl SessionContext {
    pub fn new() -> Self {
        Self {
            fork_results: HashMap::new(),
        }
    }
}

pub struct PipelineState {
    pub fork_id: Option<String>,
    pub fork_prompt: Option<String>,
    pub working_draft: String,
    pub original_goal: String,
    pub current_index: usize,
    pub aspects: Vec<Aspect>,
    pub corpus: HashMap<String, String>,
}

impl PipelineState {
    pub fn new(working_draft: String, original_goal: String, aspects: Vec<Aspect>) -> Self {
        Self {
            fork_id: None,
            fork_prompt: None,
            working_draft,
            original_goal,
            current_index: 0,
            aspects,
            corpus: HashMap::new(),
        }
    }

    pub fn current_aspect(&self) -> &Aspect {
        &self.aspects[self.current_index]
    }

    pub fn jump_to_id(&mut self, id: &str) -> Result<()> {
        let Some(pos) = self.aspects.iter().position(|a| a.id == id) else {
            anyhow::bail!("Aspect ID {} not found in pipeline", id);
        };

        self.current_index = pos;
        Ok(())
    }
}
