use crate::actions::AppContext;
use anyhow::Result;
use std::path::PathBuf;

pub struct ReadAspectFile {
    pub path: PathBuf,
}

impl ReadAspectFile {
    pub fn run(&self, _ctx: &AppContext) -> Result<String> {
        Ok(std::fs::read_to_string(&self.path)?)
    }
}
