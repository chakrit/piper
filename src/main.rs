#![deny(warnings)]

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod actions;
mod config;
mod driver;
mod dsl;
mod llm;
mod models;
mod state;

use actions::{AppContext, WriteConfig};
use driver::Driver;
use state::PipelineState;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short = 'f', long = "file", default_value = "piper.toml")]
    config_file: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Scaffold basic TOML settings
    Init,
    /// Run a pipeline
    Run {
        /// Name of the pipeline to run (from config file)
        pipeline_name: String,

        /// The original goal/objective
        #[arg(short, long)]
        goal: String,

        /// The immutable request to analyze
        #[arg(short, long)]
        request: String,

        /// Directory containing prompt templates
        #[arg(short, long, default_value = "prompts")]
        prompts: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ctx = AppContext;

    match cli.command {
        Commands::Init => {
            let mut config = config::Config::default();
            config.llm.invocation = "gpt-4o {{prompt}}".to_string();

            let mut pipelines = std::collections::HashMap::new();
            pipelines.insert(
                "default".to_string(),
                config::Pipeline {
                    aspects: vec![config::AspectStep::Inline(crate::models::Aspect {
                        id: "worker-1".to_string(),
                        name: "Initial Worker".to_string(),
                        aspect_type: crate::models::AspectType::Worker,
                        prompt: "A basic worker to get started".to_string(),
                        model: None,
                    })],
                    ..Default::default()
                },
            );
            config.pipelines = pipelines;

            let toml_string = toml::to_string_pretty(&config)?;
            let action = WriteConfig {
                path: cli.config_file.clone(),
                content: toml_string,
            };
            action.run(&ctx)?;
            println!("Created {:?} with default settings.", cli.config_file);
        }
        Commands::Run {
            pipeline_name,
            goal,
            request,
            prompts,
        } => {
            let config = config::Config::load(&ctx, cli.config_file)?;
            let pipeline = config.resolve_pipeline(&ctx, &pipeline_name)?;

            let state = PipelineState::new(request, goal, pipeline.aspects, pipeline.max_turns);
            let mut ctx = state::SessionContext::new();

            let client = llm::ConfigurableLlmClient {
                invocation: config.llm.invocation,
                system_prompt: config.llm.system_prompt,
                default_model: config.llm.default_model,
            };

            let driver = Driver::new(client, prompts);
            let results = driver.run(&mut ctx, state)?;

            println!("\n--- FINAL OUTPUTS ---");
            for (i, res) in results.iter().enumerate() {
                println!("\n--- Output {} ---\n", i + 1);
                println!("{}", res);
            }
        }
    }

    Ok(())
}
