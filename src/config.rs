use crate::models::Aspect;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub llm: LlmConfig,
    pub pipelines: HashMap<String, Pipeline>,
}

pub const DEFAULT_SYSTEM_PROMPT: &str = include_str!("system_prompt.txt");

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    pub invocation: String,
    pub system_prompt: String,
    pub default_model: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            invocation: String::new(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            default_model: String::new(),
        }
    }
}

pub const DEFAULT_MAX_TURNS: usize = 100;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Pipeline {
    pub max_turns: usize,
    pub aspects: Vec<AspectStep>,
}

impl Default for Pipeline {
    fn default() -> Self {
        Self {
            max_turns: DEFAULT_MAX_TURNS,
            aspects: Vec::new(),
        }
    }
}

pub struct ResolvedPipeline {
    pub aspects: Vec<Aspect>,
    pub max_turns: NonZeroUsize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AspectStep {
    Inline(Aspect),
    Reference { file: PathBuf },
}

impl Config {
    pub fn load(ctx: &crate::actions::AppContext, path: PathBuf) -> Result<Self> {
        let action = crate::actions::ReadConfig { path };
        let content = action
            .run(ctx)
            .with_context(|| "Could not read config file".to_string())?;
        let mut env = Self::load_env_file(Path::new(".env"))?;
        env.extend(Self::load_env_file(Path::new(".env.local"))?);
        let content = interpolate_env(&content, |name| {
            env.get(name).cloned().or_else(|| env::var(name).ok())
        })?;
        Self::parse(&content)
    }

    fn parse(content: &str) -> Result<Self> {
        toml::from_str(content).context("Failed to parse TOML config")
    }

    fn load_env_file(path: &Path) -> Result<HashMap<String, String>> {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
            Err(error) => return Err(error).with_context(|| format!("Could not read {:?}", path)),
        };

        parse_env_file(&content).with_context(|| format!("Failed to parse {:?}", path))
    }

    pub fn resolve_pipeline(
        &self,
        ctx: &crate::actions::AppContext,
        name: &str,
    ) -> Result<ResolvedPipeline> {
        let pipeline = self
            .pipelines
            .get(name)
            .with_context(|| format!("Pipeline '{}' not found in config", name))?;

        let max_turns = NonZeroUsize::new(pipeline.max_turns)
            .with_context(|| format!("Pipeline '{}' must set max_turns to at least 1", name))?;

        let mut resolved = Vec::new();
        for step in &pipeline.aspects {
            match step {
                AspectStep::Inline(aspect) => {
                    resolved.push(aspect.clone());
                }
                AspectStep::Reference { file } => {
                    let action = crate::actions::ReadAspectFile { path: file.clone() };
                    let content = action
                        .run(ctx)
                        .with_context(|| format!("Could not read aspect file {:?}", file))?;
                    let aspect: Aspect = toml::from_str(&content)
                        .with_context(|| format!("Failed to parse aspect TOML in {:?}", file))?;
                    resolved.push(aspect);
                }
            }
        }
        Ok(ResolvedPipeline {
            aspects: resolved,
            max_turns,
        })
    }
}

fn interpolate_env(
    content: &str,
    mut lookup: impl FnMut(&str) -> Option<String>,
) -> Result<String> {
    let mut output = String::with_capacity(content.len());
    let mut rest = content;

    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find('}') else {
            anyhow::bail!("unterminated environment interpolation")
        };
        let name = &after_start[..end];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            anyhow::bail!("invalid environment variable name '{name}'")
        }
        let value =
            lookup(name).with_context(|| format!("environment variable '{name}' is not set"))?;
        output.push_str(&value);
        rest = &after_start[end + 1..];
    }

    output.push_str(rest);
    Ok(output)
}

fn parse_env_file(content: &str) -> Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    for (index, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((name, value)) = line.split_once('=') else {
            anyhow::bail!(".env line {} must be KEY=VALUE", index + 1)
        };
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            anyhow::bail!(".env line {} has invalid key '{name}'", index + 1)
        }

        let value = value.trim();
        let value = if let Some(value) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
            value
                .replace("\\n", "\n")
                .replace("\\\"", "\"")
                .replace("\\\\", "\\")
        } else if let Some(value) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
            value.to_string()
        } else {
            value.to_string()
        };

        env.insert(name.to_string(), value);
    }

    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_pipeline_inline() {
        let mut pipelines = HashMap::new();
        pipelines.insert(
            "test".to_string(),
            Pipeline {
                aspects: vec![AspectStep::Inline(Aspect {
                    id: "1".to_string(),
                    name: "N1".to_string(),
                    aspect_type: crate::models::AspectType::Worker,
                    prompt: "D1".to_string(),
                    model: None,
                })],
                ..Default::default()
            },
        );

        let config = Config {
            pipelines,
            ..Default::default()
        };
        let ctx = crate::actions::AppContext;

        let resolved = config
            .resolve_pipeline(&ctx, "test")
            .expect("pipeline should resolve");
        assert_eq!(resolved.aspects.len(), 1);
        assert_eq!(resolved.aspects[0].id, "1");
        assert_eq!(resolved.max_turns.get(), DEFAULT_MAX_TURNS);
    }

    #[test]
    fn test_resolve_pipeline_reference() {
        let dir = tempdir().unwrap();
        let aspect_path = dir.path().join("aspect.toml");
        let aspect_toml = r#"
            id = "ref-1"
            name = "Refined"
            type = "worker"
            prompt = "Desc"
        "#;
        std::fs::write(&aspect_path, aspect_toml).unwrap();

        let mut pipelines = HashMap::new();
        pipelines.insert(
            "test".to_string(),
            Pipeline {
                aspects: vec![AspectStep::Reference { file: aspect_path }],
                ..Default::default()
            },
        );

        let config = Config {
            pipelines,
            ..Default::default()
        };
        let ctx = crate::actions::AppContext;

        let resolved = config
            .resolve_pipeline(&ctx, "test")
            .expect("pipeline should resolve");
        assert_eq!(resolved.aspects.len(), 1);
        assert_eq!(resolved.aspects[0].id, "ref-1");
    }

    #[test]
    fn test_resolve_pipeline_missing() {
        let config = Config::default();
        let ctx = crate::actions::AppContext;
        let result = config.resolve_pipeline(&ctx, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_pipeline_with_model_overrides() {
        let mut pipelines = HashMap::new();
        pipelines.insert(
            "test".to_string(),
            Pipeline {
                aspects: vec![AspectStep::Inline(Aspect {
                    id: "1".to_string(),
                    name: "N1".to_string(),
                    aspect_type: crate::models::AspectType::Worker,
                    prompt: "D1".to_string(),
                    model: Some("custom-model".to_string()),
                })],
                ..Default::default()
            },
        );

        let config = Config {
            pipelines,
            ..Default::default()
        };
        let ctx = crate::actions::AppContext;

        let resolved = config
            .resolve_pipeline(&ctx, "test")
            .expect("pipeline should resolve");
        assert_eq!(resolved.aspects[0].model, Some("custom-model".to_string()));
    }

    #[test]
    fn test_aspect_defaults_to_worker_type() {
        let aspect: Aspect = toml::from_str(
            r#"
                id = "default-worker"
                name = "Default Worker"
                prompt = "Prompt"
            "#,
        )
        .expect("aspect without type should parse");

        assert_eq!(aspect.aspect_type, crate::models::AspectType::Worker);
    }

    #[test]
    fn test_resolve_pipeline_rejects_zero_turn_limit() {
        let mut pipelines = HashMap::new();
        pipelines.insert(
            "test".to_string(),
            Pipeline {
                max_turns: 0,
                aspects: Vec::new(),
            },
        );

        let config = Config {
            pipelines,
            ..Default::default()
        };
        let ctx = crate::actions::AppContext;

        let result = config.resolve_pipeline(&ctx, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_interpolates_environment_values() {
        let content = r#"
            [llm]
            invocation = "${PIPER_LLM_INVOCATION}"
            default_model = "${PIPER_DEFAULT_MODEL}"
        "#;
        let expanded = interpolate_env(content, |name| match name {
            "PIPER_LLM_INVOCATION" => Some("opencode run -m {{model}} {{prompt}}".to_string()),
            "PIPER_DEFAULT_MODEL" => Some("cerebras/gemma-4-31b".to_string()),
            _ => None,
        })
        .expect("env interpolation should succeed");

        let config = Config::parse(&expanded).expect("expanded config should parse");
        assert_eq!(
            config.llm.invocation,
            "opencode run -m {{model}} {{prompt}}"
        );
        assert_eq!(config.llm.default_model, "cerebras/gemma-4-31b");
    }

    #[test]
    fn test_parse_env_file() {
        let env = parse_env_file(
            r#"
                # comment
                export PIPER_DEFAULT_MODEL=cerebras/gemma-4-31b
                PIPER_LLM_INVOCATION="opencode run -m {{model}} {{prompt}}"
            "#,
        )
        .expect(".env content should parse");

        assert_eq!(
            env.get("PIPER_LLM_INVOCATION"),
            Some(&"opencode run -m {{model}} {{prompt}}".to_string())
        );
        assert_eq!(
            env.get("PIPER_DEFAULT_MODEL"),
            Some(&"cerebras/gemma-4-31b".to_string())
        );
    }

    #[test]
    fn test_env_local_overrides_env_values() {
        let mut env =
            parse_env_file("PIPER_DEFAULT_MODEL=default-model").expect(".env content should parse");
        env.extend(
            parse_env_file("PIPER_DEFAULT_MODEL=local-model")
                .expect(".env.local content should parse"),
        );

        assert_eq!(
            env.get("PIPER_DEFAULT_MODEL"),
            Some(&"local-model".to_string())
        );
    }
}
