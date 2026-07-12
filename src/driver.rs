use crate::llm::LlmClient;
use crate::models::AspectType;
use crate::state::PipelineState;
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

    pub fn run(&self, mut state: PipelineState) -> Result<String> {
        while state.current_index < state.aspects.len() {
            let aspect = state.current_aspect().clone();
            println!("\n>>> Processing: {} ({} - {})", aspect.name, aspect.aspect_type.label(), aspect.id);

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
                        .replace("{{corpus}}", &corpus_text.join("\n"));

                    let response = self.client.call(&prompt, aspect.model.as_deref())?;
                    
                    // Update corpus with the knowledge layer added by this aspect
                    state.corpus.extend(response.knowledge.clone());
                    
                    let final_draft = response.refined_draft.clone();
                    state.working_draft = response.refined_draft;
                    
                    println!("[Driver] Violations found: {}", response.violations_found);
                    
                    if response.terminate {
                        println!("[Driver] LLM signaled termination.");
                        return Ok(final_draft);
                    }

                    if let Some(next_id) = response.next_aspect_id {
                        println!("[Driver] LLM signaled jump to: {}", next_id);
                        state.jump_to_id(&next_id)?;
                    } else {
                        state.current_index += 1;
                    }
                }
            }
        }

        Ok(state.working_draft)
    }
}
