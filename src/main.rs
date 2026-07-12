#![deny(warnings)]

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod driver;
mod llm;
mod models;
mod state;
mod config;
mod actions;

use driver::Driver;
use state::PipelineState;
use actions::{AppContext, WriteConfig};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Scaffold basic TOML settings
    Init,
    /// Run a pipeline
    Run {
        /// Name of the pipeline to run (from config.toml)
        pipeline_name: String,

        /// The original goal/objective
        #[arg(short, long)]
        goal: String,

        /// The initial draft of the content
        #[arg(short, long)]
        draft: String,

        /// Directory containing prompt templates
        #[arg(short, long, default_value = "skills/piped_piper/prompts")]
        prompts: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ctx = AppContext;

    match cli.command {
        Commands::Init => {
            let mut config = config::Config::default();
            config.llm.invocation = "gpt-4o".to_string();

            let mut pipelines = std::collections::HashMap::new();
            pipelines.insert(
                "default".to_string(),
                config::Pipeline {
                    aspects: vec![config::AspectStep::Inline(crate::models::Aspect {
                        id: "refiner-1".to_string(),
                        name: "Initial Refiner".to_string(),
                        aspect_type: crate::models::AspectType::Refiner,
                        description: "A basic refiner to get started".to_string(),
                    })],
                },
            );
            config.pipelines = pipelines;

            let toml_string = toml::to_string_pretty(&config)?;
            let action = WriteConfig {
                path: PathBuf::from("config.toml"),
                content: toml_string,
            };
            action.run(&ctx)?;
            println!("Created config.toml with default settings.");
        }
        Commands::Run {
            pipeline_name,
            goal,
            draft,
            prompts,
        } => {
            let config = config::Config::load(&ctx, PathBuf::from("config.toml"))?;
            let aspects = config.resolve_pipeline(&ctx, &pipeline_name)?;

            let state = PipelineState::new(draft, goal, aspects);

            let client = llm::ConfigurableLlmClient {
                invocation: config.llm.invocation,
            };

            let driver = Driver::new(client, prompts);
            let result = driver.run(state)?;

            println!("\n--- FINAL OUTPUT ---\n");
            println!("{}", result);
        }
    }

    Ok(())
}
