use crate::actions::{
    AcquirePipelineLock, ActivePointerError, AppContext, EnsureProject, ListProjectRuns,
    LoadRunSnapshot, ProjectPaths, ReadActivePointer, ResolveProject, WriteActivePointer,
    WriteConfig,
};
use crate::checkpointer::DiskCheckpointer;
use crate::config::Config;
use crate::driver::{
    Driver, GateDecision, log_budget_used, log_completed, log_run_started, one_line,
};
use crate::identifier::PipelineId;
use crate::llm::ConfigurableLlmClient;
use crate::prompt_templates::PromptTemplates;
use crate::run_id::{RunId, resolve_prefix, shortest_unique_prefix};
use crate::run_snapshot::{CURRENT_SCHEMA_VERSION, RunSnapshot, RunStatus, TerminalOutcome};
use crate::state::PipelineState;
use anyhow::{Context, Result, bail};
use dialoguer::Select;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// How a lifecycle command chooses which run to act on. Mirrors the spec: a pipeline id
/// selects that pipeline's active run, `--run` selects a specific run by unique prefix, and
/// neither opens an interactive selector or fails closed when non-interactive.
pub struct Selection {
    pub pipeline: Option<String>,
    pub run: Option<String>,
}

pub fn init(config_file: Option<PathBuf>) -> Result<()> {
    let config_file = config_file.unwrap_or_else(|| PathBuf::from("piper.toml"));
    let mut config = Config::default();
    config.llm.invocation = "gpt-4o {{prompt}}".to_string();

    let toml_string = toml::to_string_pretty(&config)?;
    WriteConfig {
        path: config_file.clone(),
        content: toml_string,
    }
    .run(&AppContext)?;

    println!("✓ Created configuration · {}", config_file.display());
    Ok(())
}

pub fn config(config_file: Option<PathBuf>, pipeline_file: PathBuf) -> Result<()> {
    let ctx = AppContext;
    let project = ResolveProject {
        config: config_file,
        cwd: std::env::current_dir()?,
    }
    .run(&ctx)?;

    let config = Config::load(&ctx, project.config)?;
    let pipeline = config.load_pipeline(&ctx, pipeline_file)?;
    let effective = config.into_effective(&ctx, &pipeline)?;

    print!("{}", toml::to_string_pretty(&effective)?);
    Ok(())
}

pub fn start(
    config_file: Option<PathBuf>,
    pipeline_file: PathBuf,
    request: String,
    prompts: Option<PathBuf>,
) -> Result<()> {
    let ctx = AppContext;
    let project = ensure_project(config_file)?;
    let (client, templates, snapshot) =
        build_initial_run(&project, pipeline_file, request, prompts)?;
    let pipeline_id = snapshot.pipeline_id().clone();
    let run_id = snapshot.run_id;

    let _lock = AcquirePipelineLock {
        project_state_dir: project.state_dir.clone(),
        pipeline_id: pipeline_id.clone(),
    }
    .run(&ctx)?;
    ensure_replaceable(&project, &pipeline_id)?;
    write_active(&project, &pipeline_id, run_id)?;

    log_run_started(&snapshot.state);
    let final_snapshot = disk_driver(client, templates, &project).advance(snapshot)?;
    report_outcome(&project, &final_snapshot)
}

/// The attended wrapper over the same durable engine: it starts a run, then presents each
/// gate in the terminal and applies the operator's Continue or Abort until the run pauses,
/// terminates, or the prompt is cancelled. Cancelling leaves the run waiting at that gate.
pub fn run(
    config_file: Option<PathBuf>,
    pipeline_file: PathBuf,
    request: String,
    prompts: Option<PathBuf>,
) -> Result<()> {
    let ctx = AppContext;
    let project = ensure_project(config_file)?;
    let (client, templates, snapshot) =
        build_initial_run(&project, pipeline_file, request, prompts)?;
    let pipeline_id = snapshot.pipeline_id().clone();
    let run_id = snapshot.run_id;

    let _lock = AcquirePipelineLock {
        project_state_dir: project.state_dir.clone(),
        pipeline_id: pipeline_id.clone(),
    }
    .run(&ctx)?;
    ensure_replaceable(&project, &pipeline_id)?;
    write_active(&project, &pipeline_id, run_id)?;

    log_run_started(&snapshot.state);
    let driver = disk_driver(client, templates, &project);
    let mut snapshot = driver.advance(snapshot)?;
    loop {
        let RunStatus::WaitingAtGate = snapshot.status else {
            return report_outcome(&project, &snapshot);
        };

        let gate_id = snapshot.state.current_aspect().id.clone();
        let Some(decision) = prompt_gate(&snapshot.state)? else {
            println!("╰─ Cancelled · run left waiting at gate {gate_id}");
            return Ok(());
        };
        match decision {
            GateDecision::Continue => println!("✓ Gate approved · continuing after {gate_id}"),
            GateDecision::Abort => println!("■ Gate aborted · ending run at {gate_id}"),
        }
        snapshot = driver.resume(snapshot, decision)?;
    }
}

pub fn continue_run(config_file: Option<PathBuf>, selection: Selection) -> Result<()> {
    let ctx = AppContext;
    let project = ensure_project(config_file)?;
    let Selected::Run(pipeline_id, run_id) =
        select_run(&project, &selection, RunFilter::WaitingAtGate)?
    else {
        println!("Cancelled · no run continued");
        return Ok(());
    };

    let _lock = AcquirePipelineLock {
        project_state_dir: project.state_dir.clone(),
        pipeline_id: pipeline_id.clone(),
    }
    .run(&ctx)?;
    let snapshot = load(&project, &pipeline_id, run_id)?;
    if snapshot.status != RunStatus::WaitingAtGate {
        bail!(
            "continue requires a run waiting at a gate; run {} is {}",
            short_id(&project, &snapshot)?,
            snapshot.observable_status().label()
        );
    }

    let (client, templates) = reload_runtime(&project, &snapshot)?;
    write_active(&project, &pipeline_id, run_id)?;
    let final_snapshot =
        disk_driver(client, templates, &project).resume(snapshot, GateDecision::Continue)?;
    report_outcome(&project, &final_snapshot)
}

pub fn abort(config_file: Option<PathBuf>, selection: Selection) -> Result<()> {
    let ctx = AppContext;
    let project = ensure_project(config_file)?;
    let Selected::Run(pipeline_id, run_id) =
        select_run(&project, &selection, RunFilter::NonTerminal)?
    else {
        println!("Cancelled · no run aborted");
        return Ok(());
    };

    let _lock = AcquirePipelineLock {
        project_state_dir: project.state_dir.clone(),
        pipeline_id: pipeline_id.clone(),
    }
    .run(&ctx)?;
    let snapshot = load(&project, &pipeline_id, run_id)?;
    if snapshot.status.is_terminal() {
        bail!(
            "run {} is already terminal ({})",
            short_id(&project, &snapshot)?,
            snapshot.status.label()
        );
    }

    let (client, templates) = reload_runtime(&project, &snapshot)?;
    write_active(&project, &pipeline_id, run_id)?;
    let final_snapshot = disk_driver(client, templates, &project).abort(snapshot)?;
    report_outcome(&project, &final_snapshot)
}

pub fn status(config_file: Option<PathBuf>, selection: Selection) -> Result<()> {
    let ctx = AppContext;
    let project = ResolveProject {
        config: config_file,
        cwd: std::env::current_dir()?,
    }
    .run(&ctx)?;

    let Selected::Run(pipeline_id, run_id) = select_run(&project, &selection, RunFilter::Any)?
    else {
        println!("Cancelled");
        return Ok(());
    };

    let snapshot = load(&project, &pipeline_id, run_id)?;
    print_status(&project, &snapshot)
}

// project + engine assembly

fn ensure_project(config_file: Option<PathBuf>) -> Result<ProjectPaths> {
    let ctx = AppContext;
    let project = ResolveProject {
        config: config_file,
        cwd: std::env::current_dir()?,
    }
    .run(&ctx)?;
    EnsureProject {
        directory: project.state_dir.clone(),
        canonical_root: project.root.clone(),
        configuration_path: project.config.clone(),
    }
    .run(&ctx)?;
    Ok(project)
}

fn build_initial_run(
    project: &ProjectPaths,
    pipeline_file: PathBuf,
    request: String,
    prompts: Option<PathBuf>,
) -> Result<(ConfigurableLlmClient, PromptTemplates, RunSnapshot)> {
    let ctx = AppContext;
    let config = Config::load(&ctx, project.config.clone())?;
    let pipeline = config.load_pipeline(&ctx, pipeline_file.clone())?;
    let resolved = config.resolve_pipeline(&ctx, &pipeline)?;

    let state = PipelineState::new(
        resolved.id,
        request,
        resolved.aspects,
        resolved.max_turns,
        resolved.max_recoveries,
    );
    let pipeline_path = pipeline_file
        .canonicalize()
        .with_context(|| format!("Could not resolve pipeline path {pipeline_file:?}"))?;
    let prompt_path = match &prompts {
        Some(directory) => directory
            .canonicalize()
            .with_context(|| format!("Could not resolve prompt directory {directory:?}"))?,
        None => project.root.clone(),
    };
    let templates = prompt_templates_for(&prompt_path, &project.root);
    let client = configurable_client(config);

    let snapshot = RunSnapshot {
        schema_version: CURRENT_SCHEMA_VERSION,
        run_id: RunId::generate(),
        project_root: project.root.clone(),
        config_path: project.config.clone(),
        pipeline_path,
        prompt_path,
        state,
        turns: Vec::new(),
        status: RunStatus::Ready,
        terminal: None,
    };
    Ok((client, templates, snapshot))
}

/// Reload the LLM invocation and runtime secrets from the run's stored canonical
/// configuration path for continuation. Secrets are read fresh from disk, never from the
/// snapshot; the prompt source is reconstructed from the stored prompt path.
fn reload_runtime(
    project: &ProjectPaths,
    snapshot: &RunSnapshot,
) -> Result<(ConfigurableLlmClient, PromptTemplates)> {
    let config = Config::load(&AppContext, snapshot.config_path.clone())?;
    let client = configurable_client(config);
    let templates = prompt_templates_for(&snapshot.prompt_path, &project.root);
    Ok((client, templates))
}

fn configurable_client(config: Config) -> ConfigurableLlmClient {
    ConfigurableLlmClient {
        invocation: config.llm.invocation,
        system_prompt: config.llm.system_prompt,
        default_model: config.llm.default_model,
    }
}

/// Embedded templates are recorded with the project root as their prompt path; a directory
/// source records that directory. Reconstructing by comparing against the root inverts that
/// encoding, so `start` and `continue` derive identical templates from the same inputs.
fn prompt_templates_for(prompt_path: &Path, project_root: &Path) -> PromptTemplates {
    if prompt_path == project_root {
        PromptTemplates::embedded()
    } else {
        PromptTemplates::directory(prompt_path.to_path_buf())
    }
}

fn disk_driver(
    client: ConfigurableLlmClient,
    templates: PromptTemplates,
    project: &ProjectPaths,
) -> Driver<ConfigurableLlmClient, DiskCheckpointer> {
    Driver::new(
        client,
        templates,
        DiskCheckpointer {
            project_state_dir: project.state_dir.clone(),
        },
    )
}

fn write_active(project: &ProjectPaths, pipeline_id: &PipelineId, run_id: RunId) -> Result<()> {
    WriteActivePointer {
        project_state_dir: project.state_dir.clone(),
        pipeline_id: pipeline_id.clone(),
        run_id,
    }
    .run(&AppContext)
}

fn ensure_replaceable(project: &ProjectPaths, pipeline_id: &PipelineId) -> Result<()> {
    let active = ReadActivePointer {
        project_state_dir: project.state_dir.clone(),
        pipeline_id: pipeline_id.clone(),
    }
    .run(&AppContext);

    let run_id = match active {
        Ok(run_id) => run_id,
        Err(ActivePointerError::Missing { .. }) => return Ok(()),
        Err(other) => return Err(other.into()),
    };

    let snapshot = load(project, pipeline_id, run_id)?;
    if !snapshot.status.is_terminal() {
        bail!(
            "pipeline {:?} has a non-terminal active run {} ({}); continue or abort it before starting another",
            pipeline_id.as_str(),
            short_id(project, &snapshot)?,
            snapshot.status.label()
        );
    }
    Ok(())
}

fn load(project: &ProjectPaths, pipeline_id: &PipelineId, run_id: RunId) -> Result<RunSnapshot> {
    LoadRunSnapshot {
        project_state_dir: project.state_dir.clone(),
        pipeline_id: pipeline_id.clone(),
        run_id,
        project_root: project.root.clone(),
        config_path: project.config.clone(),
    }
    .run(&AppContext)
    .map_err(Into::into)
}

// run selection

enum Selected {
    Run(PipelineId, RunId),
    Cancelled,
}

enum RunFilter {
    WaitingAtGate,
    NonTerminal,
    Any,
}

impl RunFilter {
    fn admits(&self, status: RunStatus) -> bool {
        match self {
            Self::WaitingAtGate => status == RunStatus::WaitingAtGate,
            Self::NonTerminal => !status.is_terminal(),
            Self::Any => true,
        }
    }
}

fn select_run(
    project: &ProjectPaths,
    selection: &Selection,
    filter: RunFilter,
) -> Result<Selected> {
    if let Some(prefix) = &selection.run {
        let pairs = ListProjectRuns {
            project_state_dir: project.state_dir.clone(),
        }
        .run(&AppContext)?;
        let ids: Vec<RunId> = pairs.iter().map(|(_, run_id)| *run_id).collect();
        let resolved = resolve_prefix(prefix, &ids)?;
        let (pipeline_id, run_id) = pairs
            .into_iter()
            .find(|(_, run_id)| *run_id == resolved)
            .expect("resolved run id came from the enumerated list");
        return Ok(Selected::Run(pipeline_id, run_id));
    }

    if let Some(pipeline) = &selection.pipeline {
        let pipeline_id = PipelineId::new(pipeline)?;
        let run_id = ReadActivePointer {
            project_state_dir: project.state_dir.clone(),
            pipeline_id: pipeline_id.clone(),
        }
        .run(&AppContext)?;
        return Ok(Selected::Run(pipeline_id, run_id));
    }

    select_interactively(project, filter)
}

fn select_interactively(project: &ProjectPaths, filter: RunFilter) -> Result<Selected> {
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        bail!("no run selected: pass a pipeline id or --run <ulid-or-prefix>");
    }

    let pairs = ListProjectRuns {
        project_state_dir: project.state_dir.clone(),
    }
    .run(&AppContext)?;
    let all_ids: Vec<RunId> = pairs.iter().map(|(_, run_id)| *run_id).collect();

    let mut eligible = Vec::new();
    for (pipeline_id, run_id) in pairs {
        let Ok(snapshot) = load(project, &pipeline_id, run_id) else {
            continue;
        };
        if filter.admits(snapshot.status) {
            eligible.push((pipeline_id, run_id, snapshot));
        }
    }
    if eligible.is_empty() {
        bail!("no eligible runs to select");
    }

    let items: Vec<String> = eligible
        .iter()
        .map(|(pipeline_id, run_id, snapshot)| {
            format!(
                "{} · {} · {} · {}",
                shortest_unique_prefix(run_id, &all_ids),
                pipeline_id.as_str(),
                snapshot.status.label(),
                one_line(&snapshot.state.request)
            )
        })
        .collect();

    let selection = Select::new()
        .with_prompt("Select a run")
        .items(&items)
        .interact_opt()?;
    match selection {
        Some(index) => {
            let (pipeline_id, run_id, _) = &eligible[index];
            Ok(Selected::Run(pipeline_id.clone(), *run_id))
        }
        None => Ok(Selected::Cancelled),
    }
}

// terminal reporting

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

fn report_outcome(project: &ProjectPaths, snapshot: &RunSnapshot) -> Result<()> {
    match snapshot.status {
        RunStatus::WaitingAtGate => {
            println!(
                "\n… Waiting at gate {} · run {}",
                snapshot.state.current_aspect().id,
                short_id(project, snapshot)?
            );
            println!("  Continue or abort to proceed");
        }
        RunStatus::Done => {
            let results = match &snapshot.terminal {
                Some(TerminalOutcome::Done { result }) => result.clone(),
                _ => Vec::new(),
            };
            log_completed(&snapshot.state, results.len());
            print_final_outputs(&results);
        }
        RunStatus::Stopped => {
            if let Some(TerminalOutcome::Stopped { reason }) = &snapshot.terminal {
                println!("\n■ Stopped · {}", one_line(reason));
            }
            log_budget_used(&snapshot.state);
        }
        RunStatus::Aborted => {
            println!("\n■ Aborted · run {}", short_id(project, snapshot)?);
            log_budget_used(&snapshot.state);
        }
        RunStatus::InterruptedCall => {
            println!(
                "\n! Interrupted call · run {} · resolve or abort it",
                short_id(project, snapshot)?
            );
        }
        other => bail!("run reached an unexpected status: {}", other.label()),
    }
    Ok(())
}

fn print_status(project: &ProjectPaths, snapshot: &RunSnapshot) -> Result<()> {
    println!(
        "Run       {} · {}",
        short_id(project, snapshot)?,
        snapshot.run_id
    );
    println!("Pipeline  {}", snapshot.pipeline_id().as_str());
    println!("Status    {}", snapshot.observable_status().label());
    println!("Request   {}", one_line(&snapshot.state.request));
    println!(
        "Budget    {}/{} turns · {}/{} recoveries",
        snapshot.state.turns_taken,
        snapshot.state.max_turns,
        snapshot.state.recoveries_taken,
        snapshot.state.max_recoveries
    );

    match &snapshot.terminal {
        Some(TerminalOutcome::Stopped { reason }) => {
            println!("Outcome   stopped · {}", one_line(reason))
        }
        Some(TerminalOutcome::Failed { error }) => {
            println!("Outcome   failed · {}", one_line(error))
        }
        _ => {}
    }
    if snapshot.status == RunStatus::WaitingAtGate {
        println!("Gate      {}", snapshot.state.current_aspect().id);
    }
    Ok(())
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

fn short_id(project: &ProjectPaths, snapshot: &RunSnapshot) -> Result<String> {
    let siblings: Vec<RunId> = ListProjectRuns {
        project_state_dir: project.state_dir.clone(),
    }
    .run(&AppContext)?
    .into_iter()
    .filter(|(pipeline_id, _)| pipeline_id == snapshot.pipeline_id())
    .map(|(_, run_id)| run_id)
    .collect();
    Ok(shortest_unique_prefix(&snapshot.run_id, &siblings))
}
