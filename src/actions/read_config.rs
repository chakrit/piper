use crate::actions::AppContext;
use anyhow::Result;
use std::path::PathBuf;

pub struct ReadConfig {
    pub path: PathBuf,
}

impl ReadConfig {
    pub fn run(&self, _ctx: &AppContext) -> Result<String> {
        Ok(std::fs::read_to_string(&self.path)?)
    }
}
