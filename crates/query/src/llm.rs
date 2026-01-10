use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct QueryLLM {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

impl QueryLLM {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url,
            model,
            client: reqwest::Client::new(),
        }
    }

    pub fn default() -> Self {
        Self::new(
            "http://localhost:11434".to_string(),
            "llama3".to_string(),
        )
    }

    pub async fn generate(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);

        let request = OllamaRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
        };

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Ollama")?;

        if !response.status().is_success() {
            anyhow::bail!("Ollama request failed: {}", response.status());
        }

        let ollama_response: OllamaResponse = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        Ok(ollama_response.response)
    }
}