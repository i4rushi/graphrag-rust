use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct OllamaClient {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    format: String, // "json" for structured output
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

impl OllamaClient {
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
            format: "json".to_string(), // Force JSON output
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

    /// Generate with retry for invalid JSON
    pub async fn generate_json_with_retry(
        &self,
        prompt: &str,
        max_retries: usize,
    ) -> Result<String> {
        for attempt in 0..max_retries {
            let response = self.generate(prompt).await?;
            
            // Try to parse as JSON
            if serde_json::from_str::<serde_json::Value>(&response).is_ok() {
                return Ok(response);
            }
            
            // If invalid, retry with correction prompt
            if attempt < max_retries - 1 {
                let retry_prompt = format!(
                    "The following JSON is invalid:\n{}\n\nFix this JSON. Output only valid JSON.",
                    response
                );
                
                let corrected = self.generate(&retry_prompt).await?;
                if serde_json::from_str::<serde_json::Value>(&corrected).is_ok() {
                    return Ok(corrected);
                }
            }
        }
        
        anyhow::bail!("Failed to get valid JSON after {} retries", max_retries)
    }
}