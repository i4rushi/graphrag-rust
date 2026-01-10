use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::graph_export::{EntityInfo, RelationInfo};

#[derive(Clone)]
pub struct CommunitySummarizer {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunitySummary {
    pub community_id: usize,
    pub entity_count: usize,
    pub summary: String,
    pub key_entities: Vec<String>,
}

impl CommunitySummarizer {
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

    /// Generate summary for a community
    pub async fn summarize_community(
        &self,
        community_id: usize,
        entities: &[EntityInfo],
        relations: &[RelationInfo],
    ) -> Result<CommunitySummary> {
        let prompt = self.build_summary_prompt(entities, relations);

        let summary_text = self.generate(&prompt).await
            .context("Failed to generate community summary")?;

        // Extract key entities (top 5 by degree or importance)
        let key_entities: Vec<String> = entities.iter()
            .take(5)
            .map(|e| e.name.clone())
            .collect();

        Ok(CommunitySummary {
            community_id,
            entity_count: entities.len(),
            summary: summary_text.trim().to_string(),
            key_entities,
        })
    }

    fn build_summary_prompt(
        &self,
        entities: &[EntityInfo],
        relations: &[RelationInfo],
    ) -> String {
        let mut prompt = String::from(
            "You are analyzing a community of related entities from a knowledge graph.\n\n"
        );

        prompt.push_str("ENTITIES IN THIS COMMUNITY:\n");
        for entity in entities.iter().take(10) {
            prompt.push_str(&format!(
                "- {} ({}): {}\n",
                entity.name,
                entity.entity_type,
                entity.description
            ));
        }

        if relations.len() > 0 {
            prompt.push_str("\nKEY RELATIONSHIPS:\n");
            for relation in relations.iter().take(10) {
                prompt.push_str(&format!(
                    "- {} {} {}\n",
                    relation.source,
                    relation.relation,
                    relation.target
                ));
            }
        }

        prompt.push_str(
            "\nTASK: Write a 2-3 paragraph summary describing:\n\
            1. The main theme or topic of this community\n\
            2. Key entities and their roles\n\
            3. Important relationships and patterns\n\n\
            Keep it concise and factual. Do NOT use markdown formatting.\n\n\
            SUMMARY:"
        );

        prompt
    }

    async fn generate(&self, prompt: &str) -> Result<String> {
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