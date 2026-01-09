use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct EmbeddingClient {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

impl EmbeddingClient {
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

    /// Generate embedding for text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);
        
        let request = EmbeddingRequest {
            model: self.model.clone(),
            prompt: text.to_string(),
        };

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send embedding request")?;

        if !response.status().is_success() {
            anyhow::bail!("Embedding request failed: {}", response.status());
        }

        let embedding_response: EmbeddingResponse = response
            .json()
            .await
            .context("Failed to parse embedding response")?;

        Ok(embedding_response.embedding)
    }

    /// Get embedding dimension
    pub async fn get_dimension(&self) -> Result<usize> {
        let test_embedding = self.embed("test").await?;
        Ok(test_embedding.len())
    }
}