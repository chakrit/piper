use crate::models::Aspect;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;

#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CompletedFork {
    pub id: String,
    pub prompt: String,
    pub corpus: HashMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecutionFrame {
    pub fork_id: Option<String>,
    pub fork_prompt: Option<String>,
    pub branch_path: Vec<String>,
    pub completed_forks: Vec<CompletedFork>,
    pub current_index: usize,
    pub corpus: HashMap<String, String>,
}

impl ExecutionFrame {
    fn root() -> Self {
        Self {
            fork_id: None,
            fork_prompt: None,
            branch_path: Vec::new(),
            completed_forks: Vec::new(),
            current_index: 0,
            corpus: HashMap::new(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PipelineState {
    pub request: String,
    pub aspects: Vec<Aspect>,
    pub frames: Vec<ExecutionFrame>,
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
            request,
            aspects,
            frames: vec![ExecutionFrame::root()],
            max_turns,
            max_recoveries,
        }
    }

    pub fn frame(&self) -> &ExecutionFrame {
        self.frames
            .last()
            .expect("pipeline always has a root frame")
    }

    pub fn frame_mut(&mut self) -> &mut ExecutionFrame {
        self.frames
            .last_mut()
            .expect("pipeline always has a root frame")
    }

    pub fn is_root(&self) -> bool {
        self.frames.len() == 1
    }

    pub fn contribution_key(&self) -> String {
        let aspect_id = &self.current_aspect().id;
        if self.frame().branch_path.is_empty() {
            return aspect_id.to_string();
        }

        format!("{}/{}", self.frame().branch_path.join("/"), aspect_id)
    }

    pub fn context_path(&self) -> String {
        let aspect_id = &self.current_aspect().id;
        if self.frame().branch_path.is_empty() {
            return aspect_id.to_string();
        }

        format!("{aspect_id}/{}", self.frame().branch_path.join("/"))
    }

    pub fn current_aspect(&self) -> &Aspect {
        &self.aspects[self.frame().current_index]
    }

    pub fn jump_to_id(&mut self, id: &str) -> Result<()> {
        let pos = self
            .aspects
            .iter()
            .position(|aspect| aspect.id == id)
            .with_context(|| format!("Aspect ID {id} not found in pipeline"))?;
        self.frame_mut().current_index = pos;
        Ok(())
    }

    pub fn push_fork(&mut self, id: String, prompt: String) -> Result<()> {
        anyhow::ensure!(
            !self
                .frame()
                .completed_forks
                .iter()
                .any(|fork| fork.id == id),
            "fork ID '{id}' already exists"
        );
        let parent = self.frame();
        let mut branch_path = parent.branch_path.clone();
        branch_path.push(id.clone());
        self.frames.push(ExecutionFrame {
            fork_id: Some(id),
            fork_prompt: Some(prompt),
            branch_path,
            completed_forks: Vec::new(),
            current_index: parent.current_index,
            corpus: parent.corpus.clone(),
        });
        Ok(())
    }

    pub fn join_completed_forks(&mut self) {
        let forks = std::mem::take(&mut self.frame_mut().completed_forks);
        for fork in forks {
            self.frame_mut().corpus.extend(fork.corpus);
        }
    }

    pub fn return_current_fork(&mut self) -> Result<()> {
        anyhow::ensure!(!self.is_root(), "root frame cannot return to a parent");
        let child = self.frames.pop().expect("non-root frame exists");
        let parent = self.frame_mut();
        let corpus = child
            .corpus
            .into_iter()
            .filter(|(key, value)| parent.corpus.get(key) != Some(value))
            .collect();
        parent.completed_forks.push(CompletedFork {
            id: child.fork_id.context("child frame must have a fork ID")?,
            prompt: child
                .fork_prompt
                .context("child frame must have a fork prompt")?,
            corpus,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Aspect, AspectType};

    #[test]
    fn frame_stack_round_trips_through_json() {
        let mut state = PipelineState::new(
            "Request".to_string(),
            vec![Aspect {
                id: "research".to_string(),
                name: "Research".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Find evidence".to_string(),
                model: None,
            }],
            NonZeroUsize::new(8).unwrap(),
            2,
        );
        state
            .push_fork("market".to_string(), "Study the market".to_string())
            .unwrap();
        state
            .frame_mut()
            .corpus
            .insert("market/research".to_string(), "Evidence".to_string());

        let serialized = serde_json::to_vec(&state).expect("state should serialize");
        let restored: PipelineState =
            serde_json::from_slice(&serialized).expect("state should deserialize");

        assert_eq!(restored.frames.len(), 2);
        assert_eq!(restored.context_path(), "research/market");
        assert_eq!(
            restored.frame().corpus.get("market/research"),
            Some(&"Evidence".to_string())
        );
    }

    #[test]
    fn nested_frames_return_and_join_without_recursion() {
        let mut state = PipelineState::new(
            "Request".to_string(),
            vec![Aspect {
                id: "research".to_string(),
                name: "Research".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Find evidence".to_string(),
                model: None,
            }],
            NonZeroUsize::new(8).unwrap(),
            2,
        );
        state
            .push_fork("market".to_string(), "Study the market".to_string())
            .unwrap();
        state
            .push_fork("buyers".to_string(), "Study buyers".to_string())
            .unwrap();
        state.frame_mut().corpus.insert(
            "market/buyers/research".to_string(),
            "Buyer evidence".to_string(),
        );

        state.return_current_fork().unwrap();
        assert_eq!(state.frames.len(), 2);
        assert_eq!(state.frame().completed_forks[0].id, "buyers");
        state.join_completed_forks();
        state
            .frame_mut()
            .corpus
            .insert("market/research".to_string(), "Market evidence".to_string());
        state.return_current_fork().unwrap();

        assert_eq!(state.frames.len(), 1);
        let returned = &state.frame().completed_forks[0].corpus;
        assert_eq!(returned.len(), 2);
        assert_eq!(returned["market/buyers/research"], "Buyer evidence");
        assert_eq!(returned["market/research"], "Market evidence");
    }
}
