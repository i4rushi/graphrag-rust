pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

pub mod schema;
pub mod normalizer;
pub mod llm;
pub mod prompt;

pub use schema::{Entity, Relation, ExtractionResult, ExtractedChunk};
pub use normalizer::EntityNormalizer;
pub use llm::OllamaClient;
use std::collections::HashMap;

use anyhow::{Context, Result};
use serde_json;

pub struct Extractor {
    llm_client: OllamaClient,
    normalizer: EntityNormalizer,
}

impl Extractor {
    pub fn new(llm_client: OllamaClient) -> Self {
        Self {
            llm_client,
            normalizer: EntityNormalizer::new(),
        }
    }

    pub fn default() -> Self {
        Self::new(OllamaClient::default())
    }

    /// Extract entities and relations from a chunk of text
    pub async fn extract_from_text(&mut self, text: &str) -> Result<ExtractionResult> {
        // Build prompt
        let prompt = prompt::build_extraction_prompt(text);
        
        // Get JSON response with retry
        let json_str = self.llm_client
            .generate_json_with_retry(&prompt, 3)
            .await
            .context("Failed to extract entities after retries")?;
        
        // Parse JSON
        let mut result: ExtractionResult = serde_json::from_str(&json_str)
            .context("Failed to parse extraction result")?;
        
        // Build a mapping from old IDs (E1, E2) to normalized names
        let mut id_to_normalized: HashMap<String, String> = HashMap::new();
        
        // Normalize entity names and build mapping
        for entity in &mut result.entities {
            let normalized = self.normalizer.normalize(&entity.name);
            id_to_normalized.insert(entity.id.clone(), normalized.clone());
            entity.id = normalized.clone();
            entity.name = normalized;
        }
        
        // Update relation source/target using the mapping
        for relation in &mut result.relations {
            if let Some(normalized_source) = id_to_normalized.get(&relation.source) {
                relation.source = normalized_source.clone();
            }
            if let Some(normalized_target) = id_to_normalized.get(&relation.target) {
                relation.target = normalized_target.clone();
            }
        }
        
        Ok(result)
    }

    /// Extract from a chunk with metadata
    pub async fn extract_chunk(
        &mut self,
        chunk_id: String,
        doc_id: String,
        text: &str,
    ) -> Result<ExtractedChunk> {
        let extraction = self.extract_from_text(text).await?;
        
        Ok(ExtractedChunk {
            chunk_id,
            doc_id,
            extraction,
        })
    }

    pub fn get_normalizer(&self) -> &EntityNormalizer {
        &self.normalizer
    }
}