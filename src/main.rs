use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;

mod driver;
mod llm;
mod models;
mod state;

use driver::Driver;
use llm::ClaudeCliClient;
use models::Aspect;
use state::PipelineState;

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

fn main() -> Result<()> {
    let args = Args::parse();

    let pipeline_content = fs::read_to_string(&args.pipeline)
        .with_context(|| format!("Could not read pipeline file {:?}", args.pipeline))?;
    let aspects: Vec<Aspect> =
        serde_json::from_str(&pipeline_content).context("Failed to parse pipeline JSON")?;

    let state = PipelineState::new(args.draft, args.goal, aspects);

    let driver = Driver::new(ClaudeCliClient, args.prompts);
    let result = driver.run(state)?;

    println!("\n--- FINAL OUTPUT ---\n");
    println!("{}", result);

    Ok(())
}
