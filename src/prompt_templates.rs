use crate::actions::{AppContext, ReadPromptTemplate};
use anyhow::{Context, Result};
use std::path::PathBuf;

const ASPECT_WORKER: &str = include_str!("../prompts/aspect-worker.md");
const ASPECT_RECOVER: &str = include_str!("../prompts/aspect-recover.md");

pub enum PromptTemplates {
    Embedded,
    Directory(PathBuf),
}

impl PromptTemplates {
    pub fn embedded() -> Self {
        Self::Embedded
    }

    pub fn directory(path: PathBuf) -> Self {
        Self::Directory(path)
    }

    pub fn worker(&self, ctx: &AppContext) -> Result<String> {
        self.load(ctx, "aspect-worker.md", ASPECT_WORKER)
    }

    pub fn recover(&self, ctx: &AppContext) -> Result<String> {
        self.load(ctx, "aspect-recover.md", ASPECT_RECOVER)
    }

    fn load(&self, ctx: &AppContext, filename: &str, embedded: &str) -> Result<String> {
        match self {
            Self::Embedded => Ok(embedded.to_string()),
            Self::Directory(directory) => ReadPromptTemplate {
                path: directory.join(filename),
            }
            .run(ctx)
            .with_context(|| format!("Failed to read prompt template {filename}")),
        }
    }
}
