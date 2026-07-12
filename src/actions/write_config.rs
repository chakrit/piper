use crate::actions::AppContext;
use anyhow::Result;
use std::path::PathBuf;

pub struct WriteConfig {
    pub path: PathBuf,
    pub content: String,
}

impl WriteConfig {
    pub fn run(&self, _ctx: &AppContext) -> Result<()> {
        std::fs::write(&self.path, &self.content)?;
        Ok(())
    }
}
