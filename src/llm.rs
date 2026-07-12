use crate::models::LlmResponse;
use anyhow::{Context, Result};
use std::process::Command;

pub trait LlmClient {
    fn call(&self, prompt: &str) -> Result<LlmResponse>;
}

pub struct ConfigurableLlmClient {
    pub invocation: String,
}

impl LlmClient for ConfigurableLlmClient {
    fn call(&self, prompt: &str) -> Result<LlmResponse> {
        let parts: Vec<&str> = self.invocation.split_whitespace().collect();
        if parts.is_empty() {
            anyhow::bail!("LLM invocation command is empty");
        }

        let output = Command::new(parts[0])
            .args(&parts[1..])
            .arg(prompt)
            .output()
            .with_context(|| format!("Failed to execute LLM command: {}", self.invocation))?;

        if !output.status.success() {
            anyhow::bail!(
                "LLM command exited with error: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let response_text = String::from_utf8_lossy(&output.stdout);

        let json_str = if let Some(start) = response_text.find("```json") {
            let content = &response_text[start + 7..];
            if let Some(end) = content.find("```") {
                content[..end].trim()
            } else {
                content.trim()
            }
        } else {
            response_text.trim()
        };

        serde_json::from_str(json_str).context("Failed to parse LLM response as JSON")
    }
}
