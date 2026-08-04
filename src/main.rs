#![deny(warnings)]

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod actions;
mod checkpointer;
mod commands;
mod config;
mod driver;
mod dsl;
mod identifier;
mod llm;
mod models;
mod prompt_templates;
mod run_id;
mod run_snapshot;
mod state;

use commands::Selection;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short = 'f', long = "file")]
    config_file: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

/// Selects which run a lifecycle command acts on: a pipeline id names that pipeline's
/// active run, `--run` names a specific run by ULID or unique prefix, and neither opens an
/// interactive selector or fails closed when the process is non-interactive.
#[derive(clap::Args, Debug)]
struct RunSelection {
    /// Pipeline id whose active run to select
    pipeline: Option<String>,

    /// Select a specific run by ULID or unique prefix
    #[arg(long)]
    run: Option<String>,
}

impl From<RunSelection> for Selection {
    fn from(selection: RunSelection) -> Self {
        Self {
            pipeline: selection.pipeline,
            run: selection.run,
        }
    }
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Scaffold basic TOML settings
    Init,
    /// Print the effective configuration for a pipeline
    Config {
        /// TOML file containing the pipeline to resolve
        pipeline_file: PathBuf,
    },
    /// Start a new durable run and execute it to a gate or terminal status
    Start {
        /// TOML file containing the pipeline to run
        pipeline_file: PathBuf,

        /// The immutable request to analyze, including its desired outcome
        request: String,

        /// Directory containing prompt templates
        #[arg(short, long)]
        prompts: Option<PathBuf>,
    },
    /// Run a pipeline attended, presenting each gate in the terminal
    Run {
        /// TOML file containing the pipeline to run
        pipeline_file: PathBuf,

        /// The immutable request to analyze, including its desired outcome
        request: String,

        /// Directory containing prompt templates
        #[arg(short, long)]
        prompts: Option<PathBuf>,
    },
    /// Approve the current gate of a run and execute to the next gate or terminal
    Continue {
        #[command(flatten)]
        selection: RunSelection,
    },
    /// Record an operator abort on a non-terminal run
    Abort {
        #[command(flatten)]
        selection: RunSelection,
    },
    /// Report a run's state without mutating it
    Status {
        #[command(flatten)]
        selection: RunSelection,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_file = cli.config_file;

    match cli.command {
        Commands::Init => commands::init(config_file),
        Commands::Config { pipeline_file } => commands::config(config_file, pipeline_file),
        Commands::Start {
            pipeline_file,
            request,
            prompts,
        } => commands::start(config_file, pipeline_file, request, prompts),
        Commands::Run {
            pipeline_file,
            request,
            prompts,
        } => commands::run(config_file, pipeline_file, request, prompts),
        Commands::Continue { selection } => commands::continue_run(config_file, selection.into()),
        Commands::Abort { selection } => commands::abort(config_file, selection.into()),
        Commands::Status { selection } => commands::status(config_file, selection.into()),
    }
}
