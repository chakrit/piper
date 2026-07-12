use crate::models::Aspect;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub llm: LlmConfig,
    pub pipelines: HashMap<String, Pipeline>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LlmConfig {
    pub invocation: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Pipeline {
    pub aspects: Vec<AspectStep>,
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
        let content = action.run(ctx).with_context(|| "Could not read config file".to_string())?;
        toml::from_str(&content).context("Failed to parse TOML config")
    }

    pub fn resolve_pipeline(&self, ctx: &crate::actions::AppContext, name: &str) -> Result<Vec<Aspect>> {
        let pipeline = self.pipelines.get(name)
            .with_context(|| format!("Pipeline '{}' not found in config", name))?;

        let mut resolved = Vec::new();
        for step in &pipeline.aspects {
            match step {
                AspectStep::Inline(aspect) => {
                    resolved.push(aspect.clone());
                }
                AspectStep::Reference { file } => {
                    let action = crate::actions::ReadAspectFile { path: file.clone() };
                    let content = action.run(ctx)
                        .with_context(|| format!("Could not read aspect file {:?}", file))?;
                    let aspect: Aspect = toml::from_str(&content)
                        .with_context(|| format!("Failed to parse aspect TOML in {:?}", file))?;
                    resolved.push(aspect);
                }
            }
        }
        Ok(resolved)
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
                    description: "D1".to_string(),
                })],
            },
        );

        let config = Config {
            pipelines,
            ..Default::default()
        };
        let ctx = crate::actions::AppContext;

        let resolved = config.resolve_pipeline(&ctx, "test").unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, "1");
    }

    #[test]
    fn test_resolve_pipeline_reference() {
        let dir = tempdir().unwrap();
        let aspect_path = dir.path().join("aspect.toml");
        let aspect_toml = r#"
            id = "ref-1"
            name = "Refined"
            aspect_type = "refiner"
            description = "Desc"
        "#;
        std::fs::write(&aspect_path, aspect_toml).unwrap();

        let mut pipelines = HashMap::new();
        pipelines.insert(
            "test".to_string(),
            Pipeline {
                aspects: vec![AspectStep::Reference {
                    file: aspect_path,
                }],
            },
        );

        let config = Config {
            pipelines,
            ..Default::default()
        };
        let ctx = crate::actions::AppContext;

        let resolved = config.resolve_pipeline(&ctx, "test").unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, "ref-1");
    }

    #[test]
    fn test_resolve_pipeline_missing() {
        let config = Config::default();
        let ctx = crate::actions::AppContext;
        let result = config.resolve_pipeline(&ctx, "nonexistent");
        assert!(result.is_err());
    }
}
