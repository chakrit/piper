use super::AppContext;
use anyhow::Result;
use std::path::PathBuf;

pub struct ReadPromptTemplate {
    pub path: PathBuf,
}

impl ReadPromptTemplate {
    pub fn run(&self, _ctx: &AppContext) -> Result<String> {
        Ok(std::fs::read_to_string(&self.path)?)
    }
}
