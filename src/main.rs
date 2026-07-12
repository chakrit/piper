use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Aspect {
    name: String,
    description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
enum RunStatus {
    Idle,
    AwaitingInput,
    Completed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RunState {
    run_id: String,
    pipeline_path: PathBuf,
    goal: String,
    current_draft: String,
    current_aspect_index: usize,
    status: RunStatus,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct HistoryEntry {
    aspect_index: usize,
    prompt: String,
    response: String,
    timestamp: DateTime<Utc>,
}

#[derive(Parser)]
#[command(name = "piped-piper")]
#[command(about = "Aspect-Oriented Prompting Engine", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new run
    Start {
        #[arg(short, long)]
        pipeline: PathBuf,
        #[arg(short, long)]
        goal: String,
        #[arg(short, long)]
        draft: String,
    },
    /// Advance to the next aspect and get the prompt
    Step {
        run_id: String,
    },
    /// Apply the response to the current aspect
    Apply {
        run_id: String,
        response: String,
    },
    /// Show current run status
    Status {
        run_id: String,
    },
    /// Run the engine interactively for humans
    Interactive {
        run_id: String,
    },
}

struct Engine {
    state_dir: PathBuf,
}

impl Engine {
    fn new() -> Result<Self> {
        let state_dir = PathBuf::from(".piped_piper/runs");
        if !state_dir.exists() {
            fs::create_dir_all(&state_dir)?;
        }
        Ok(Self { state_dir })
    }

    fn get_run_path(&self, run_id: &str) -> PathBuf {
        self.state_dir.join(run_id)
    }

    fn load_state(&self, run_id: &str) -> Result<RunState> {
        let path = self.get_run_path(run_id).join("state.json");
        let content = fs::read_to_string(path).context("Run state not found. Use 'start' first.")?;
        Ok(serde_json::from_str(&content)?)
    }

    fn save_state(&self, state: &RunState) -> Result<()> {
        let path = self.get_run_path(&state.run_id).join("state.json");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(state)?;
        fs::write(path, content)?;
        Ok(())
    }

    fn load_history(&self, run_id: &str) -> Result<Vec<HistoryEntry>> {
        let path = self.get_run_path(run_id).join("history.json");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    fn save_history(&self, run_id: &str, history: &[HistoryEntry]) -> Result<()> {
        let path = self.get_run_path(run_id).join("history.json");
        let content = serde_json::to_string_pretty(history)?;
        fs::write(path, content)?;
        Ok(())
    }

    fn start(&self, pipeline_path: PathBuf, goal: String, draft: String) -> Result<String> {
        let run_id = Uuid::new_v4().to_string();
        let state = RunState {
            run_id: run_id.clone(),
            pipeline_path,
            goal,
            current_draft: draft,
            current_aspect_index: 0,
            status: RunStatus::Idle,
            created_at: Utc::now(),
        };
        self.save_state(&state)?;
        Ok(run_id)
    }

    fn step(&self, run_id: &str) -> Result<String> {
        let mut state = self.load_state(run_id)?;
        
        if let RunStatus::AwaitingInput = state.status {
            return Err(anyhow::anyhow!("Run is already awaiting input. Use 'apply' to proceed."));
        }

        let pipeline: Vec<Aspect> = {
            let content = fs::read_to_string(&state.pipeline_path)
                .context("Could not read pipeline file")?;
            serde_json::from_str(&content)?
        };

        if state.current_aspect_index >= pipeline.len() {
            // Final Synthesis
            state.status = RunStatus::Completed;
            self.save_state(&state)?;
            
            let synthesis_template = fs::read_to_string("skills/piped_piper/prompts/final-alignment.md")
                .context("Could not find final-alignment.md template")?;
            
            let aspect_names: Vec<String> = pipeline.iter().map(|a| a.name.clone()).collect();
            
            Ok(synthesis_template
                .replace("{{original_goal}}", &state.goal)
                .replace("{{final_draft}}", &state.current_draft)
                .replace("{{aspect_list}}", &aspect_names.join(", ")))
        } else {
            let aspect = &pipeline[state.current_aspect_index];
            let refiner_template = fs::read_to_string("skills/piped_piper/prompts/aspect-refiner.md")
                .context("Could not find aspect-refiner.md template")?;
            
            let prompt = refiner_template
                .replace("{{aspect_name}}", &aspect.name)
                .replace("{{aspect_description}}", &aspect.description)
                .replace("{{current_draft}}", &state.current_draft);
            
            state.status = RunStatus::AwaitingInput;
            self.save_state(&state)?;
            
            Ok(prompt)
        }
    }

    fn apply(&self, run_id: &str, response: String) -> Result<String> {
        let mut state = self.load_state(run_id)?;
        
        if let RunStatus::AwaitingInput = state.status {
            // In a real-world scenario, we'd parse JSON from the response.
            // For now, as per the user's "dumb" requirement, we basically 
            // trust the driver to provide the refined draft or we do basic extraction.
            
            // Mimic the Python driver's basic JSON extraction
            let refined_draft = if response.contains("```json") {
                response.split("```json").nth(1).unwrap_or(&response)
                       .split("```").next().unwrap_or(&response).trim().to_string()
            } else {
                response.trim().to_string()
            };

            // Try to parse as JSON to get 'refined_draft' field specifically
            let final_draft = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&refined_draft) {
                json["refined_draft"].as_str().unwrap_or(&refined_draft).to_string()
            } else {
                refined_draft
            };

            let mut history = self.load_history(run_id)?;
            
            // Get the prompt that was sent (we'd need to store it in state or re-generate it)
            // For simplicity, let's store the last prompt in metadata or just record the index.
            history.push(HistoryEntry {
                aspect_index: state.current_aspect_index,
                prompt: "".to_string(), // Simplification: in a real impl, store this in RunState
                response: response.clone(),
                timestamp: Utc::now(),
            });
            self.save_history(run_id, &history)?;

            state.current_draft = final_draft;
            state.current_aspect_index += 1;
            state.status = RunStatus::Idle;
            self.save_state(&state)?;

            Ok(format!("Applied response. Next aspect index: {}", state.current_aspect_index))
        } else {
            Err(anyhow::anyhow!("Run is not awaiting input."))
        }
    }

    fn status(&self, run_id: &str) -> Result<String> {
        let state = self.load_state(run_id)?;
        let pipeline: Vec<Aspect> = {
            let content = fs::read_to_string(&state.pipeline_path)?;
            serde_json::from_str(&content)?
        };
        
        Ok(format!(
            "Run ID: {}\nGoal: {}\nProgress: {}/{} aspects\nStatus: {:?}\nDraft Length: {} chars",
            state.run_id, state.goal, state.current_aspect_index, pipeline.len(), state.status, state.current_draft.len()
        ))
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let engine = Engine::new()?;

    match cli.command {
        Commands::Start { pipeline, goal, draft } => {
            let id = engine.start(pipeline, goal, draft)?;
            println!("Run started. ID: {}", id);
        }
        Commands::Step { run_id } => {
            let prompt = engine.step(&run_id)?;
            println!("{}", prompt);
        }
        Commands::Apply { run_id, response } => {
            let res = engine.apply(&run_id, response)?;
            println!("{}", res);
        }
        Commands::Status { run_id } => {
            println!("{}", engine.status(&run_id)?);
        }
        Commands::Interactive { run_id } => {
            loop {
                let prompt = engine.step(&run_id)?;
                println!("\n--- CURRENT ASPECT PROMPT ---\n{}", prompt);
                
                println!("\n[C]ontinue (input response), [S]tatus, [Q]uit?");
                let input: String = dialoguer::Input::new().with_prompt("Action").interact_text()?;
                
                match input.to_uppercase().as_str() {
                    "C" => {
                        let response: String = dialoguer::Input::new().with_prompt("Enter LLM Response").interact_text()?;
                        engine.apply(&run_id, response)?;
                    }
                    "S" => {
                        println!("{}", engine.status(&run_id)?);
                    }
                    "Q" => break,
                    _ => println!("Unknown action."),
                }
                
                if let RunStatus::Completed = engine.load_state(&run_id)?.status {
                    println!("Run completed.");
                    break;
                }
            }
        }
    }
    Ok(())
}
