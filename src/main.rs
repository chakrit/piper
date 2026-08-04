#![deny(warnings)]

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod actions;
mod checkpointer;
mod config;
mod driver;
mod dsl;
mod identifier;
mod llm;
mod models;
mod prompt_templates;
// `RunId` is consumed here and by the checkpointer; prefix resolution awaits the CLI slice.
#[allow(dead_code)]
mod run_id;
mod run_snapshot;
mod state;

use actions::{AppContext, EnsureProject, ResolveProject, WriteConfig};
use checkpointer::{Checkpointer, NoopCheckpointer};
use dialoguer::Select;
use driver::{Driver, GateDecision};
use llm::LlmClient;
use run_id::RunId;
use run_snapshot::{CURRENT_SCHEMA_VERSION, RunSnapshot, RunStatus, TerminalOutcome};
use state::PipelineState;

use driver::{log_budget_used, log_completed, log_run_started, one_line};

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
    /// Print the effective configuration for a pipeline
    Config {
        /// TOML file containing the pipeline to resolve
        pipeline_file: PathBuf,
    },
    /// Run a pipeline
    Run {
        /// TOML file containing the pipeline to run
        pipeline_file: PathBuf,

        /// The immutable request to analyze, including its desired outcome
        request: String,

        /// Directory containing prompt templates
        #[arg(short, long)]
        prompts: Option<PathBuf>,
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
            println!("✓ Created configuration · {}", config_file.display());
        }
        Commands::Config { pipeline_file } => {
            let cwd = std::env::current_dir()?;
            let project = ResolveProject {
                config: cli.config_file,
                cwd,
            }
            .run(&ctx)?;
            let config = config::Config::load(&ctx, project.config)?;
            let pipeline = config.load_pipeline(&ctx, pipeline_file)?;
            let effective = config.into_effective(&ctx, &pipeline)?;

            print!("{}", toml::to_string_pretty(&effective)?);
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
            let project_root = project.root.clone();
            let config_path = project.config.clone();
            EnsureProject {
                directory: project.state_dir,
                canonical_root: project.root,
                configuration_path: project.config.clone(),
            }
            .run(&ctx)?;
            let config = config::Config::load(&ctx, project.config)?;
            let pipeline = config.load_pipeline(&ctx, pipeline_file.clone())?;
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
            let prompt_path = prompts.clone().unwrap_or_else(|| project_root.clone());
            let prompt_templates = match prompts {
                Some(directory) => prompt_templates::PromptTemplates::directory(directory),
                None => prompt_templates::PromptTemplates::embedded(),
            };

            let snapshot = RunSnapshot {
                schema_version: CURRENT_SCHEMA_VERSION,
                run_id: RunId::generate(),
                project_root,
                config_path,
                pipeline_path: pipeline_file,
                prompt_path,
                state,
                turns: Vec::new(),
                status: RunStatus::Ready,
                terminal: None,
            };

            // The attended wrapper drops checkpoints this slice; the CLI-lifecycle slice
            // resolves the run identity and swaps in the disk-backed sink.
            let driver = Driver::new(client, prompt_templates, NoopCheckpointer);
            run_attended(&driver, snapshot)?;
        }
    }

    Ok(())
}

/// Attended wrapper over the headless engine: drives to each pause, presents any gate
/// in the terminal, and applies the operator's Continue or Abort. Cancelling the prompt
/// leaves the run waiting at that gate.
fn run_attended<C: LlmClient, K: Checkpointer>(
    driver: &Driver<C, K>,
    snapshot: RunSnapshot,
) -> Result<()> {
    log_run_started(&snapshot.state);

    let mut snapshot = driver.advance(snapshot)?;
    loop {
        match snapshot.status {
            RunStatus::WaitingAtGate => {
                let gate_id = snapshot.state.current_aspect().id.clone();
                let Some(decision) = prompt_gate(&snapshot.state)? else {
                    println!("╰─ Cancelled · run left waiting at gate {gate_id}");
                    return Ok(());
                };
                match decision {
                    GateDecision::Continue => {
                        println!("✓ Gate approved · continuing after {gate_id}")
                    }
                    GateDecision::Abort => println!("■ Gate aborted · ending run at {gate_id}"),
                }
                snapshot = driver.resume(snapshot, decision)?;
            }
            RunStatus::Done => {
                let results = match &snapshot.terminal {
                    Some(TerminalOutcome::Done { result }) => result.clone(),
                    _ => Vec::new(),
                };
                log_completed(&snapshot.state, results.len());
                print_final_outputs(&results);
                return Ok(());
            }
            RunStatus::Stopped => {
                if let Some(TerminalOutcome::Stopped { reason }) = &snapshot.terminal {
                    println!("\n■ Stopped · {}", one_line(reason));
                }
                log_budget_used(&snapshot.state);
                return Ok(());
            }
            RunStatus::Aborted => {
                log_budget_used(&snapshot.state);
                return Ok(());
            }
            other => anyhow::bail!("run reached an unexpected status: {}", other.label()),
        }
    }
}

fn prompt_gate(state: &PipelineState) -> Result<Option<GateDecision>> {
    println!("│ Request   {}", one_line(&state.request));
    println!("╰─ Waiting for approval");

    let options = ["Continue", "Abort"];
    let selection = Select::new()
        .with_prompt("Choose an action")
        .items(&options)
        .interact_opt()?;

    Ok(match selection {
        Some(0) => Some(GateDecision::Continue),
        Some(_) => Some(GateDecision::Abort),
        None => None,
    })
}

fn print_final_outputs(results: &[String]) {
    if results.is_empty() {
        println!("\nFinal outputs · none");
        return;
    }

    println!("\n╭─ Final outputs");
    for (index, result) in results.iter().enumerate() {
        println!("│ Output {}", index + 1);
        println!("│");
        for line in result.lines() {
            println!("│ {line}");
        }
    }
    println!("╰─ End");
}
