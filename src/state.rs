use crate::identifier::PipelineId;
use crate::models::Aspect;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;

#[derive(Debug, Serialize, Deserialize)]
pub struct CompletedFork {
    pub id: String,
    pub prompt: String,
    pub corpus: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FrameScope {
    Root,
    Fork { id: String, prompt: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionFrame {
    pub scope: FrameScope,
    pub completed_forks: Vec<CompletedFork>,
    pub current_index: usize,
    pub corpus: HashMap<String, String>,
}

impl ExecutionFrame {
    fn root() -> Self {
        Self {
            scope: FrameScope::Root,
            completed_forks: Vec::new(),
            current_index: 0,
            corpus: HashMap::new(),
        }
    }

    pub fn fork_id(&self) -> Option<&str> {
        match &self.scope {
            FrameScope::Root => None,
            FrameScope::Fork { id, .. } => Some(id),
        }
    }

    pub fn fork_prompt(&self) -> Option<&str> {
        match &self.scope {
            FrameScope::Root => None,
            FrameScope::Fork { prompt, .. } => Some(prompt),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PipelineState {
    pub pipeline_id: PipelineId,
    pub request: String,
    pub aspects: Vec<Aspect>,
    pub frames: Vec<ExecutionFrame>,
    pub max_turns: NonZeroUsize,
    pub max_recoveries: usize,
    pub turns_taken: usize,
    pub recoveries_taken: usize,
}

impl PipelineState {
    pub fn new(
        pipeline_id: PipelineId,
        request: String,
        aspects: Vec<Aspect>,
        max_turns: NonZeroUsize,
        max_recoveries: usize,
    ) -> Self {
        Self {
            pipeline_id,
            request,
            aspects,
            frames: vec![ExecutionFrame::root()],
            max_turns,
            max_recoveries,
            turns_taken: 0,
            recoveries_taken: 0,
        }
    }

    pub fn frame(&self) -> &ExecutionFrame {
        self.frames
            .last()
            .expect("pipeline always has a root frame")
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(!self.frames.is_empty(), "pipeline state has no root frame");
        anyhow::ensure!(
            matches!(self.frames[0].scope, FrameScope::Root),
            "first execution frame must have root scope"
        );
        anyhow::ensure!(
            self.frames[1..]
                .iter()
                .all(|frame| matches!(frame.scope, FrameScope::Fork { .. })),
            "non-root execution frames must have fork scope"
        );
        anyhow::ensure!(
            self.turns_taken <= self.max_turns.get(),
            "consumed turns exceed max_turns"
        );
        anyhow::ensure!(
            self.recoveries_taken <= self.max_recoveries,
            "consumed recoveries exceed max_recoveries"
        );

        let mut aspect_ids = HashSet::new();
        for aspect in &self.aspects {
            crate::identifier::validate(&aspect.id, "aspect ID")?;
            anyhow::ensure!(
                aspect_ids.insert(&aspect.id),
                "duplicate aspect ID '{}'",
                aspect.id
            );
        }
        for frame in &self.frames {
            anyhow::ensure!(
                frame.current_index <= self.aspects.len(),
                "execution frame has an out-of-range Aspect position"
            );
            if let FrameScope::Fork { id, .. } = &frame.scope {
                crate::identifier::validate(id, "fork ID")?;
            }
            let mut fork_ids = HashSet::new();
            for fork in &frame.completed_forks {
                crate::identifier::validate(&fork.id, "fork ID")?;
                anyhow::ensure!(
                    fork_ids.insert(&fork.id),
                    "duplicate completed fork ID '{}'",
                    fork.id
                );
            }
        }

        Ok(())
    }

    pub fn consume_turn(&mut self) -> Result<()> {
        if self.turns_taken >= self.max_turns.get() {
            anyhow::bail!(
                "Pipeline '{}' exceeded its max_turns limit of {}",
                self.pipeline_id.as_str(),
                self.max_turns
            );
        }

        self.turns_taken += 1;
        Ok(())
    }

    pub fn consume_recovery(&mut self) -> Result<()> {
        if self.recoveries_taken >= self.max_recoveries {
            anyhow::bail!(
                "Pipeline '{}' exceeded its max_recoveries limit of {}",
                self.pipeline_id.as_str(),
                self.max_recoveries
            );
        }

        self.recoveries_taken += 1;
        Ok(())
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
        let branch_path = self.branch_path();
        if branch_path.is_empty() {
            return aspect_id.to_string();
        }

        format!("{}/{}", branch_path.join("/"), aspect_id)
    }

    pub fn context_path(&self) -> String {
        let aspect_id = &self.current_aspect().id;
        let branch_path = self.branch_path();
        if branch_path.is_empty() {
            return aspect_id.to_string();
        }

        format!("{aspect_id}/{}", branch_path.join("/"))
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
        let parent_index = self.frame().current_index;
        let parent_corpus = self.frame().corpus.clone();
        self.frames.push(ExecutionFrame {
            scope: FrameScope::Fork { id, prompt },
            completed_forks: Vec::new(),
            current_index: parent_index,
            corpus: parent_corpus,
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
        let (id, prompt) = match &self.frame().scope {
            FrameScope::Root => anyhow::bail!("child frame cannot have root scope"),
            FrameScope::Fork { id, prompt } => (id.clone(), prompt.clone()),
        };
        let child = self.frames.pop().expect("non-root frame exists");
        let parent = self.frame_mut();
        let corpus = child
            .corpus
            .into_iter()
            .filter(|(key, value)| parent.corpus.get(key) != Some(value))
            .collect();
        parent
            .completed_forks
            .push(CompletedFork { id, prompt, corpus });
        Ok(())
    }

    fn branch_path(&self) -> Vec<&str> {
        self.frames
            .iter()
            .filter_map(ExecutionFrame::fork_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Aspect, AspectType};

    fn pipeline_id() -> PipelineId {
        PipelineId::new("test-pipeline").expect("test pipeline ID should be valid")
    }

    #[test]
    fn nested_frames_return_and_join_without_recursion() {
        let mut state = PipelineState::new(
            pipeline_id(),
            "Request".to_string(),
            vec![Aspect {
                id: "research".to_string(),
                name: "Research".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Find evidence".to_string(),
                model: None,
            }],
            NonZeroUsize::new(8).expect("turn limit should be non-zero"),
            2,
        );
        state
            .push_fork("market".to_string(), "Study the market".to_string())
            .expect("outer fork should start");
        state
            .push_fork("buyers".to_string(), "Study buyers".to_string())
            .expect("nested fork should start");
        state.frame_mut().corpus.insert(
            "market/buyers/research".to_string(),
            "Buyer evidence".to_string(),
        );

        state
            .return_current_fork()
            .expect("nested fork should return");
        assert_eq!(state.frames.len(), 2);
        assert_eq!(state.frame().completed_forks[0].id, "buyers");
        state.join_completed_forks();
        state
            .frame_mut()
            .corpus
            .insert("market/research".to_string(), "Market evidence".to_string());
        state
            .return_current_fork()
            .expect("outer fork should return");

        assert_eq!(state.frames.len(), 1);
        let returned = &state.frame().completed_forks[0].corpus;
        assert_eq!(returned.len(), 2);
        assert_eq!(returned["market/buyers/research"], "Buyer evidence");
        assert_eq!(returned["market/research"], "Market evidence");
    }

    #[test]
    fn resumed_state_from_json_returns_and_joins_like_the_original() {
        let mut state = PipelineState::new(
            pipeline_id(),
            "Request".to_string(),
            vec![Aspect {
                id: "research".to_string(),
                name: "Research".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Find evidence".to_string(),
                model: None,
            }],
            NonZeroUsize::new(8).expect("turn limit should be non-zero"),
            2,
        );
        state
            .push_fork("market".to_string(), "Study the market".to_string())
            .expect("outer fork should start");
        state
            .push_fork("buyers".to_string(), "Study buyers".to_string())
            .expect("nested fork should start");
        state.frame_mut().corpus.insert(
            "market/buyers/research".to_string(),
            "Buyer evidence".to_string(),
        );

        let snapshot = serde_json::to_string(&state).expect("state should serialize to JSON");
        let mut resumed: PipelineState =
            serde_json::from_str(&snapshot).expect("snapshot should deserialize");

        resumed
            .validate()
            .expect("resumed state must satisfy frame invariants");
        assert_eq!(resumed.frames.len(), 3);

        resumed
            .return_current_fork()
            .expect("nested fork should return after resume");
        resumed.join_completed_forks();
        resumed
            .frame_mut()
            .corpus
            .insert("market/research".to_string(), "Market evidence".to_string());
        resumed
            .return_current_fork()
            .expect("outer fork should return after resume");

        assert_eq!(resumed.frames.len(), 1);
        let returned = &resumed.frame().completed_forks[0].corpus;
        assert_eq!(returned.len(), 2);
        assert_eq!(returned["market/buyers/research"], "Buyer evidence");
        assert_eq!(returned["market/research"], "Market evidence");
    }

    #[test]
    fn pipeline_owns_consumed_budget_counts() {
        let mut state = PipelineState::new(
            pipeline_id(),
            "Request".to_string(),
            vec![Aspect {
                id: "worker".to_string(),
                name: "Worker".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Work".to_string(),
                model: None,
            }],
            NonZeroUsize::new(1).expect("turn limit should be non-zero"),
            1,
        );

        state.consume_turn().expect("first turn should fit");
        state.consume_recovery().expect("first recovery should fit");
        assert_eq!(state.turns_taken, 1);
        assert_eq!(state.recoveries_taken, 1);
        assert!(state.consume_turn().is_err());
        assert!(state.consume_recovery().is_err());
    }

    #[test]
    fn validation_rejects_inconsistent_resumed_frames() {
        let mut state = PipelineState::new(
            pipeline_id(),
            "Request".to_string(),
            vec![Aspect {
                id: "worker".to_string(),
                name: "Worker".to_string(),
                aspect_type: AspectType::Worker,
                prompt: "Work".to_string(),
                model: None,
            }],
            NonZeroUsize::new(1).expect("turn limit should be non-zero"),
            0,
        );
        state.frames.push(ExecutionFrame::root());

        let error = state
            .validate()
            .expect_err("a root-scoped child frame should fail validation");

        assert!(error.to_string().contains("non-root"));
    }
}
