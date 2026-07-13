use crate::models::Aspect;
use anyhow::Result;
use std::collections::HashMap;
use std::num::NonZeroUsize;

pub struct SessionContext {
    pub fork_corpora: HashMap<String, HashMap<String, String>>,
    turns_taken: usize,
}

impl SessionContext {
    pub fn new() -> Self {
        Self {
            fork_corpora: HashMap::new(),
            turns_taken: 0,
        }
    }

    pub fn consume_turn(&mut self, max_turns: NonZeroUsize) -> Result<()> {
        if self.turns_taken >= max_turns.get() {
            anyhow::bail!("Pipeline exceeded its max_turns limit of {}", max_turns);
        }

        self.turns_taken += 1;
        Ok(())
    }
}

pub struct PipelineState {
    pub fork_id: Option<String>,
    pub fork_prompt: Option<String>,
    pub request: String,
    pub goal: String,
    pub current_index: usize,
    pub aspects: Vec<Aspect>,
    pub corpus: HashMap<String, String>,
    pub max_turns: NonZeroUsize,
}

impl PipelineState {
    pub fn new(
        request: String,
        goal: String,
        aspects: Vec<Aspect>,
        max_turns: NonZeroUsize,
    ) -> Self {
        Self {
            fork_id: None,
            fork_prompt: None,
            request,
            goal,
            current_index: 0,
            aspects,
            corpus: HashMap::new(),
            max_turns,
        }
    }

    pub fn corpus_key(&self, key: &str) -> String {
        self.fork_id
            .as_ref()
            .map(|fork_id| format!("{fork_id}/{key}"))
            .unwrap_or_else(|| key.to_string())
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
