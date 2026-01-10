#![allow(dead_code)]
use dashmap::DashMap;
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub struct Cache {
    embeddings: Arc<DashMap<String, Vec<f32>>>,
    llm_responses: Arc<DashMap<String, String>>,
    max_entries: usize,
}

impl Cache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            embeddings: Arc::new(DashMap::new()),
            llm_responses: Arc::new(DashMap::new()),
            max_entries,
        }
    }

    /// Cache an embedding
    pub fn set_embedding(&self, text: &str, embedding: Vec<f32>) {
        if self.embeddings.len() >= self.max_entries {
            // Simple eviction: clear 25% when full
            let to_remove: Vec<_> = self.embeddings.iter()
                .take(self.max_entries / 4)
                .map(|r| r.key().clone())
                .collect();
            for key in to_remove {
                self.embeddings.remove(&key);
            }
        }
        let key = self.hash_text(text);
        self.embeddings.insert(key, embedding);
    }

    pub fn get_embedding(&self, text: &str) -> Option<Vec<f32>> {
        let key = self.hash_text(text);
        self.embeddings.get(&key).map(|r| r.value().clone())
    }

    /// Cache an LLM response
    pub fn set_llm_response(&self, prompt: &str, response: String) {
        if self.llm_responses.len() >= self.max_entries {
            let to_remove: Vec<_> = self.llm_responses.iter()
                .take(self.max_entries / 4)
                .map(|r| r.key().clone())
                .collect();
            for key in to_remove {
                self.llm_responses.remove(&key);
            }
        }
        let key = self.hash_text(prompt);
        self.llm_responses.insert(key, response);
    }

    pub fn get_llm_response(&self, prompt: &str) -> Option<String> {
        let key = self.hash_text(prompt);
        self.llm_responses.get(&key).map(|r| r.value().clone())
    }

    fn hash_text(&self, text: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            embeddings_cached: self.embeddings.len(),
            llm_responses_cached: self.llm_responses.len(),
        }
    }

    pub fn clear(&self) {
        self.embeddings.clear();
        self.llm_responses.clear();
    }
}

#[derive(Debug, serde::Serialize)]
pub struct CacheStats {
    pub embeddings_cached: usize,
    pub llm_responses_cached: usize,
}