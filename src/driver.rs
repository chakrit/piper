use crate::llm::LlmClient;
use crate::models::{AspectAction, AspectType, LlmResponse};
use crate::state::{PipelineState, SessionContext};
use anyhow::{Context, Result};
use dialoguer::{Input, Select};
use std::fs;
use std::path::PathBuf;

pub struct Driver<C: LlmClient> {
    client: C,
    prompt_dir: PathBuf,
}

impl<C: LlmClient> Driver<C> {
    pub fn new(client: C, prompt_dir: PathBuf) -> Self {
        Self { client, prompt_dir }
    }

    fn load_prompt(&self, filename: &str) -> Result<String> {
        fs::read_to_string(self.prompt_dir.join(filename))
            .with_context(|| format!("Failed to read prompt template {}", filename))
    }

    fn parse_response(&self, text: &str) -> Result<LlmResponse> {
        let json_str = if let Some(start) = text.find("```json") {
            let content = &text[start + 7..];
            if let Some(end) = content.find("```") {
                content[..end].trim()
            } else {
                content.trim()
            }
        } else {
            text.trim()
        };

        serde_json::from_str(json_str).context("Failed to parse LLM response as JSON")
    }

    fn handle_gate(&self, state: &mut PipelineState) -> Result<bool> {
        println!("\n--- 🛑 STOP-GATE: {} ---", state.current_aspect().name);
        println!("\nCURRENT DRAFT:\n\n{}\n", state.working_draft);
        println!("--------------------------------------------------");

        let options = vec!["Continue", "Edit Draft", "Abort"];
        let selection = Select::new()
            .with_prompt("Choose an action")
            .items(&options)
            .interact()?;

        match selection {
            0 => Ok(true),
            1 => {
                let new_draft: String = Input::new()
                    .with_prompt("Enter the edited draft (or leave empty to keep current)")
                    .allow_empty(true)
                    .interact_text()?;
                if !new_draft.is_empty() {
                    state.working_draft = new_draft;
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub fn run(&self, ctx: &mut SessionContext, mut state: PipelineState) -> Result<Vec<String>> {
        let mut all_results = Vec::new();
        let mut terminated = false;

        while state.current_index < state.aspects.len() {
            let aspect = state.current_aspect().clone();
            let fork_info = state.fork_id.as_ref()
                .map(|id| format!("[Fork: {}]", id))
                .unwrap_or_default();
            println!("\n{} >>> Processing: {} ({} - {})", fork_info, aspect.name, aspect.aspect_type.label(), aspect.id);

            match aspect.aspect_type {
                AspectType::Gate => {
                    if !self.handle_gate(&mut state)? {
                        anyhow::bail!("Run aborted by user at gate {}", aspect.id);
                    }
                    state.current_index += 1;
                }
                AspectType::Refiner => {
                    let template = self.load_prompt("aspect-refiner.md")?;
                    
                    let mut corpus_text = Vec::new();
                    for aspect_item in &state.aspects {
                        if let Some(k) = state.corpus.get(&aspect_item.id) {
                            corpus_text.push(format!("[{}]: {}", aspect_item.id, k));
                        }
                    }

                    let prompt = template
                        .replace("{{aspect_name}}", &aspect.name)
                        .replace("{{aspect_prompt}}", &aspect.prompt)
                        .replace("{{aspect_id}}", &aspect.id)
                        .replace("{{current_draft}}", &state.working_draft)
                        .replace("{{goal}}", &state.original_goal)
                        .replace("{{fork_prompt}}", state.fork_prompt.as_deref().unwrap_or(""))
                        .replace("{{corpus}}", &corpus_text.join("\n"));

                    let raw_response = self.client.call(&prompt, aspect.model.as_deref())?;
                    
                    let response = match self.parse_response(&raw_response) {
                        Ok(res) => res,
                        Err(_) => {
                            println!("[Driver] Parsing failed. Attempting recovery...");
                            let recover_template = self.load_prompt("aspect-recover.md")
                                .unwrap_or_else(|_| "Please reformat the following response into valid JSON based on the required schema.".to_string());
                            
                            let recover_prompt = format!("{}\n\nBad Response:\n{}", recover_template, raw_response);
                            let recovered_raw = self.client.call(&recover_prompt, aspect.model.as_deref())?;
                            self.parse_response(&recovered_raw)?
                        }
                    };
                    
                    println!("[Driver] Violations found: {}", response.violations_found);

                    let mut jumped = false;

                    for action in response.actions {
                        match action {
                            AspectAction::Refine { draft } => {
                                state.working_draft = draft;
                            }
                            AspectAction::AddKnowledge { key, value } => {
                                state.corpus.insert(key, value);
                            }
                            AspectAction::EditKnowledge { key, value } => {
                                state.corpus.insert(key, value);
                            }
                            AspectAction::Fork { id, prompt, goal, draft, start_aspect_id } => {
                                let mut fork_state = PipelineState {
                                    fork_id: Some(id.clone()),
                                    fork_prompt: Some(prompt),
                                    working_draft: draft,
                                    original_goal: goal,
                                    current_index: 0,
                                    aspects: state.aspects.clone(),
                                    corpus: state.corpus.clone(),
                                };
                                if let Some(sid) = start_aspect_id {
                                    fork_state.jump_to_id(&sid)?;
                                }
                                
                                let fork_results = self.run(ctx, fork_state)?;
                                if let Some(last_res) = fork_results.last().cloned() {
                                    ctx.fork_results.insert(id, last_res);
                                }
                                all_results.extend(fork_results);
                            }
                            AspectAction::Jump { id } => {
                                state.jump_to_id(&id)?;
                                jumped = true;
                            }
                            AspectAction::Merge { fork_ids } => {
                                for fid in fork_ids {
                                    if let Some(res) = ctx.fork_results.get(&fid) {
                                        state.working_draft.push_str("\n\n--- Merged from fork ");
                                        state.working_draft.push_str(&fid);
                                        state.working_draft.push_str(" ---\n");
                                        state.working_draft.push_str(res);
                                    }
                                }
                            }
                            AspectAction::Terminate => {
                                terminated = true;
                            }
                        }
                    }

                    if terminated {
                        println!("[Driver] LLM signaled termination.");
                        all_results.push(state.working_draft);
                        return Ok(all_results);
                    }

                    if !jumped {
                        state.current_index += 1;
                    }
                }
            }
        }

        all_results.push(state.working_draft);
        Ok(all_results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use std::sync::Mutex;

    struct MockLlmClient {
        responses: Mutex<Vec<String>>,
    }

    impl LlmClient for MockLlmClient {
        fn call(&self, _prompt: &str, _model: Option<&str>) -> Result<String> {
            let mut resps = self.responses.lock().unwrap();
            if resps.is_empty() {
                anyhow::bail!("No mock responses left");
            }
            Ok(resps.remove(0))
        }
    }

    #[test]
    fn test_run_with_dsl() {
        let temp_dir = std::env::temp_dir().join("piper_tests_dsl");
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("aspect-refiner.md"), "Mock template").unwrap();

        let parent_resp = serde_json::to_string(&LlmResponse {
            actions: vec![
                AspectAction::Refine { draft: "Parent Updated".to_string() },
                AspectAction::Fork {
                    id: "f1".to_string(),
                    prompt: "p1".to_string(),
                    goal: "g1".to_string(),
                    draft: "Draft 1".to_string(),
                    start_aspect_id: None,
                },
                AspectAction::Merge { fork_ids: vec!["f1".to_string()] },
            ],
            violations_found: 0,
        }).unwrap();

        let fork_resp = serde_json::to_string(&LlmResponse {
            actions: vec![
                AspectAction::Refine { draft: "Fork 1 Final".to_string() },
                AspectAction::Terminate,
            ],
            violations_found: 0,
        }).unwrap();

        let client = MockLlmClient {
            responses: Mutex::new(vec![parent_resp, fork_resp]),
        };

        let driver = Driver::new(client, temp_dir);
        let mut ctx = SessionContext::new();
        let state = PipelineState {
            fork_id: None,
            fork_prompt: None,
            working_draft: "Initial".to_string(),
            original_goal: "Goal".to_string(),
            current_index: 0,
            aspects: vec![Aspect {
                id: "a1".to_string(),
                name: "Aspect 1".to_string(),
                aspect_type: AspectType::Refiner,
                prompt: "Prompt".to_string(),
                model: None,
            }],
            corpus: std::collections::HashMap::new(),
        };

        let results = driver.run(&mut ctx, state).unwrap();
        
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"Fork 1 Final".to_string()));
        assert!(results.iter().any(|r| r.contains("Fork 1 Final")));
    }
}
