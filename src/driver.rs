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
                    let prompt = template
                        .replace("{{aspect_name}}", &aspect.name)
                        .replace("{{aspect_description}}", &aspect.description)
                        .replace("{{current_draft}}", &state.working_draft);

                    let response = self.client.call(&prompt)?;
                    state.working_draft = response.refined_draft;

                    println!("[Driver] Violations found: {}", response.violations_found);

                    if let Some(next_id) = response.next_aspect_id {
                        println!("[Driver] LLM signaled jump to: {}", next_id);
                        state.jump_to_id(&next_id)?;
                    } else {
                        state.current_index += 1;
                    }
                }
            }
        }

        println!("\n>>> Performing Final Synthesis...");
        let synth_template = self.load_prompt("final-alignment.md")?;
        let aspect_list: Vec<String> = state.aspects.iter().map(|a| a.name.clone()).collect();
        let final_prompt = synth_template
            .replace("{{original_goal}}", &state.original_goal)
            .replace("{{final_draft}}", &state.working_draft)
            .replace("{{aspect_list}}", &aspect_list.join(", "));

        let final_resp = self.client.call(&final_prompt)?;
        Ok(final_resp.refined_draft)
    }
}
