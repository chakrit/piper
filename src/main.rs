#![deny(warnings)]

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod actions;
mod config;
mod driver;
mod dsl;
mod identifier;
mod llm;
mod models;
mod state;

use actions::{AppContext, EnsureProject, ResolveProject, WriteConfig};
use driver::Driver;
use state::PipelineState;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short = 'f', long = "file")]
    config_file: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Scaffold basic TOML settings
    Init,
    /// Run a pipeline
    Run {
        /// TOML file containing the pipeline to run
        pipeline_file: PathBuf,

        /// The immutable request to analyze, including its desired outcome
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
            let config_file = cli
                .config_file
                .unwrap_or_else(|| PathBuf::from("piper.toml"));
            let mut config = config::Config::default();
            config.llm.invocation = "gpt-4o {{prompt}}".to_string();

            let toml_string = toml::to_string_pretty(&config)?;
            let action = WriteConfig {
                path: config_file.clone(),
                content: toml_string,
            };
            action.run(&ctx)?;
            println!("Created {:?} with default settings.", config_file);
        }
        Commands::Run {
            pipeline_file,
            request,
            prompts,
        } => {
            let cwd = std::env::current_dir()?;
            let project = ResolveProject {
                config: cli.config_file,
                cwd,
            }
            .run(&ctx)?;
            EnsureProject {
                directory: project.state_dir,
                canonical_root: project.root,
                configuration_path: project.config.clone(),
            }
            .run(&ctx)?;
            let config = config::Config::load(&ctx, project.config)?;
            let pipeline = config.load_pipeline(&ctx, pipeline_file)?;
            let pipeline = config.resolve_pipeline(&ctx, &pipeline)?;

            let state = PipelineState::new(
                pipeline.id,
                request,
                pipeline.aspects,
                pipeline.max_turns,
                pipeline.max_recoveries,
            );
            let client = llm::ConfigurableLlmClient {
                invocation: config.llm.invocation,
                system_prompt: config.llm.system_prompt,
                default_model: config.llm.default_model,
            };

            let driver = Driver::new(client, prompts);
            let results = driver.run(state)?;

            println!("\n--- FINAL OUTPUTS ---");
            for (i, res) in results.iter().enumerate() {
                println!("\n--- Output {} ---\n", i + 1);
                println!("{}", res);
            }
        }
    }

    Ok(())
}
