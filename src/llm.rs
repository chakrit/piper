use anyhow::{Context, Result};
use std::process::Command;

pub trait LlmClient {
    fn call(&self, prompt: &str, model: Option<&str>) -> Result<String>;

    fn resolve_model<'a>(&'a self, model: Option<&'a str>) -> &'a str {
        model.unwrap_or("unspecified")
    }
}

pub struct ConfigurableLlmClient {
    pub invocation: String,
    pub system_prompt: String,
    pub default_model: String,
}

impl LlmClient for ConfigurableLlmClient {
    fn resolve_model<'a>(&'a self, model: Option<&'a str>) -> &'a str {
        model.unwrap_or(&self.default_model)
    }

    fn call(&self, prompt: &str, model: Option<&str>) -> Result<String> {
        let full_prompt = format!("{}\n\n{}", self.system_prompt, prompt);
        if !self.invocation.contains("{{prompt}}") {
            anyhow::bail!(
                "LLM invocation command must contain '{{prompt}}' placeholder. \
                 Example: 'gpt-4o {{prompt}}' or 'claude -p {{prompt}}'"
            );
        }

        let template_parts: Vec<&str> = self.invocation.split_whitespace().collect();
        if template_parts.is_empty() {
            anyhow::bail!("LLM invocation command is empty");
        }

        let active_model = self.resolve_model(model);
        let mut args = Vec::new();
        for part in template_parts {
            let mut replaced = part.replace("{{prompt}}", &full_prompt);
            replaced = replaced.replace("{{model}}", active_model);
            args.push(replaced);
        }

        let output = Command::new(&args[0])
            .args(&args[1..])
            .output()
            .with_context(|| format!("Failed to execute LLM command: {}", self.invocation))?;

        if !output.status.success() {
            anyhow::bail!(
                "LLM command exited with error: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}
