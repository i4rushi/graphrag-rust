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

pub mod chunk;
pub mod chunker;
pub mod reader;

pub use chunk::Chunk;
pub use chunker::{Chunker, ChunkerConfig};
pub use reader::FileReader;

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Generate a stable document ID from file path
pub fn generate_doc_id(path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..16])
}

/// Main ingestion pipeline
pub async fn ingest_file(file_path: &Path) -> Result<Vec<Chunk>> {
    let content = FileReader::read_file(file_path).await?;
    let path_str = file_path.to_string_lossy().to_string();
    let doc_id = generate_doc_id(&path_str);
    
    let chunker = Chunker::new(ChunkerConfig::default());
    let chunks = chunker.chunk_text(&doc_id, &content, &path_str);
    
    Ok(chunks)
}

/// Ingest entire directory
pub async fn ingest_directory(dir_path: &Path) -> Result<Vec<Chunk>> {
    let files = FileReader::read_directory(dir_path).await?;
    let chunker = Chunker::new(ChunkerConfig::default());
    
    let mut all_chunks = Vec::new();
    
    for (path, content) in files {
        let doc_id = generate_doc_id(&path);
        let chunks = chunker.chunk_text(&doc_id, &content, &path);
        all_chunks.extend(chunks);
    }
    
    Ok(all_chunks)
}
