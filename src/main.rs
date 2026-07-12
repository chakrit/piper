use anyhow::{Context, Result};
use clap::Parser;
use dialoguer::{Input, Select};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the pipeline JSON file
    #[arg(short, long)]
    pipeline: PathBuf,

    /// The original goal/objective
    #[arg(short, long)]
    goal: String,

    /// The initial draft of the content
    #[arg(short, long)]
    draft: String,

    /// Directory containing prompt templates
    #[arg(short, long, default_value = "skills/piped_piper/prompts")]
    prompts: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
enum AspectType {
    Refiner,
    Gate,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Aspect {
    id: String,
    name: String,
    #[serde(rename = "type")]
    aspect_type: AspectType,
    description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct LlmResponse {
    refined_draft: String,
    violations_found: u32,
    next_aspect_id: Option<String>,
}

trait LlmClient {
    fn call(&self, prompt: &str) -> Result<LlmResponse>;
}

struct ClaudeCliClient;

impl LlmClient for ClaudeCliClient {
    fn call(&self, prompt: &str) -> Result<LlmResponse> {
        let output = Command::new("claude")
            .arg("-p")
            .arg(prompt)
            .output()
            .context("Failed to execute claude CLI")?;

        if !output.status.success() {
            anyhow::bail!(
                "Claude CLI exited with error: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let response_text = String::from_utf8_lossy(&output.stdout);
        
        // Extract JSON from potentially wrapped markdown blocks
        let json_str = if let Some(start) = response_text.find("```json") {
            let content = &response_text[start + 7..];
            if let Some(end) = content.find("```") {
                content[..end].trim()
            } else {
                content.trim()
            }
        } else {
            response_text.trim()
        };

        serde_json::from_str(json_str).context("Failed to parse LLM response as JSON")
    }
}

struct PipelineState {
    working_draft: String,
    original_goal: String,
    current_index: usize,
    aspects: Vec<Aspect>,
}

impl PipelineState {
    fn current_aspect(&self) -> &Aspect {
        &self.aspects[self.current_index]
    }

    fn jump_to_id(&mut self, id: &str) -> Result<()> {
        if let Some(pos) = self.aspects.iter().position(|a| a.id == id) {
            self.current_index = pos;
            Ok(())
        } else {
            anyhow::bail!("Aspect ID {} not found in pipeline", id)
        }
    }
}

struct Driver<C: LlmClient> {
    client: C,
    prompt_dir: PathBuf,
}

impl<C: LlmClient> Driver<C> {
    fn new(client: C, prompt_dir: PathBuf) -> Self {
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

    fn run(&self, mut state: PipelineState) -> Result<String> {
        while state.current_index < state.aspects.len() {
            let aspect = state.current_aspect().clone();
            println!("\n>>> Processing: {} ({})", aspect.name, aspect.id);

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

        // Final synthesis (similar to the original driver)
        println!("\n>>> Performing Final Synthesis...");
        let synth_template = self.load_prompt("final-alignment.md")?;
        let aspect_list: Vec<String> = state.aspects.iter().map(|a| a.name.clone()).collect();
        let final_prompt = synth_template
            .replace("{{original_goal}}", &state.original_goal)
            .replace("{{final_draft}}", &state.working_draft)
            .replace("{{aspect_list}}", &aspect_list.join(", "));

        // For the final synthesis, we manually trigger the LLM and return the result as a string
        // since the Synthesis pass doesn't return a JSON LlmResponse but a final result.
        // I'll add a special method to the trait or just handle it.
        
        // For now, let's just call the client and use the refined_draft part of the response
        // assuming the final-alignment template is also structured to return a JSON object.
        let final_resp = self.client.call(&final_prompt)?;
        Ok(final_resp.refined_draft)
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    let pipeline_content = fs::read_to_string(&args.pipeline)
        .with_context(|| format!("Could not read pipeline file {:?}", args.pipeline))?;
    let aspects: Vec<Aspect> = serde_json::from_str(&pipeline_content)
        .context("Failed to parse pipeline JSON")?;

    let state = PipelineState {
        working_draft: args.draft,
        original_goal: args.goal,
        current_index: 0,
        aspects,
    };

    let driver = Driver::new(ClaudeCliClient, args.prompts);
    let result = driver.run(state)?;

    println!("\n--- FINAL OUTPUT ---\n");
    println!("{}", result);

    Ok(())
}
