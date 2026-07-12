use anyhow::{Context, Result};
use std::process::Command;
use crate::models::LlmResponse;

pub trait LlmClient {
    fn call(&self, prompt: &str) -> Result<LlmResponse>;
}

pub struct ClaudeCliClient;

impl LlmClient for ClaudeCliClient {
    fn call(&self, prompt: &str) -> Result<LlmResponse> {
        let output = Command::new("claude")
            .arg("-p")
            .arg(prompt)
            .output()
            .context("Failed to execute claude CLI")?;

        if !output.status.success() {
            anyhow::bail!(
                "Claude CLI exited with error: {}",
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
