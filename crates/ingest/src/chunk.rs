use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub doc_id: String,
    pub chunk_id: String,
    pub text: String,
    pub source: String,
    pub offset: (usize, usize), // [start, end] character positions
}

impl Chunk {
    pub fn new(
        doc_id: String,
        text: String,
        source: String,
        offset: (usize, usize),
    ) -> Self {
        // Generate stable chunk_id from content
        let chunk_id = Self::generate_chunk_id(&doc_id, &text, offset);
        
        Self {
            doc_id,
            chunk_id,
            text,
            source,
            offset,
        }
    }

    fn generate_chunk_id(doc_id: &str, text: &str, offset: (usize, usize)) -> String {
        let mut hasher = Sha256::new();
        hasher.update(doc_id.as_bytes());
        hasher.update(text.as_bytes());
        hasher.update(offset.0.to_string().as_bytes());
        hasher.update(offset.1.to_string().as_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..16]) // Use first 16 bytes (32 hex chars)
    }

    /// Estimate token count (rough: 1.3 tokens per word)
    pub fn estimated_tokens(&self) -> usize {
        let word_count = self.text.split_whitespace().count();
        (word_count as f64 * 1.3) as usize
    }
}