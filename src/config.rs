use crate::models::Aspect;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;

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
        toml::from_str(&content).context("Failed to parse TOML config")
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
                    aspect_type: crate::models::AspectType::Refiner,
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
            type = "refiner"
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
                    aspect_type: crate::models::AspectType::Refiner,
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
}
