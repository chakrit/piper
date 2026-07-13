use crate::models::Aspect;
use anyhow::Result;
use std::collections::HashMap;
use std::num::NonZeroUsize;

pub struct SessionContext {
    turns_taken: usize,
    recoveries_taken: usize,
}

impl SessionContext {
    pub fn new() -> Self {
        Self {
            turns_taken: 0,
            recoveries_taken: 0,
        }
    }

    pub fn consume_turn(&mut self, max_turns: NonZeroUsize) -> Result<()> {
        if self.turns_taken >= max_turns.get() {
            anyhow::bail!("Pipeline exceeded its max_turns limit of {}", max_turns);
        }

        self.turns_taken += 1;
        Ok(())
    }

    pub fn consume_recovery(&mut self, max_recoveries: usize) -> Result<()> {
        if self.recoveries_taken >= max_recoveries {
            anyhow::bail!(
                "Pipeline exceeded its max_recoveries limit of {}",
                max_recoveries
            );
        }

        self.recoveries_taken += 1;
        Ok(())
    }
}

pub struct CompletedFork {
    pub id: String,
    pub prompt: String,
    pub corpus: HashMap<String, String>,
}

pub struct PipelineState {
    pub fork_id: Option<String>,
    pub fork_prompt: Option<String>,
    pub branch_path: Vec<String>,
    pub completed_forks: Vec<CompletedFork>,
    pub request: String,
    pub current_index: usize,
    pub aspects: Vec<Aspect>,
    pub corpus: HashMap<String, String>,
    pub max_turns: NonZeroUsize,
    pub max_recoveries: usize,
}

impl PipelineState {
    pub fn new(
        request: String,
        aspects: Vec<Aspect>,
        max_turns: NonZeroUsize,
        max_recoveries: usize,
    ) -> Self {
        Self {
            fork_id: None,
            fork_prompt: None,
            branch_path: Vec::new(),
            completed_forks: Vec::new(),
            request,
            current_index: 0,
            aspects,
            corpus: HashMap::new(),
            max_turns,
            max_recoveries,
        }
    }

    pub fn contribution_key(&self) -> String {
        let aspect_id = &self.current_aspect().id;
        if self.branch_path.is_empty() {
            return aspect_id.to_string();
        }

        format!("{}/{}", self.branch_path.join("/"), aspect_id)
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
