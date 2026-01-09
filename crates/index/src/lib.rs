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

pub mod embeddings;
pub mod qdrant_index;
pub mod neo4j_index;

pub use embeddings::EmbeddingClient;
pub use qdrant_index::QdrantIndexer;
pub use neo4j_index::{Neo4jIndexer, GraphStats};

use anyhow::Result;
//use std::path::Path;

/// Unified indexer that handles both Qdrant and Neo4j
pub struct Indexer {
    qdrant: QdrantIndexer,
    neo4j: Neo4jIndexer,
}

impl Indexer {
    pub fn new(qdrant: QdrantIndexer, neo4j: Neo4jIndexer) -> Self {
        Self { qdrant, neo4j }
    }

    /// Initialize both stores
    pub async fn init(&self) -> Result<()> {
        println!("Initializing Qdrant...");
        self.qdrant.init_collection().await?;
        
        println!("Initializing Neo4j...");
        self.neo4j.init_schema().await?;
        
        println!("Indexer initialized successfully");
        Ok(())
    }

    /// Index a single extracted chunk
    pub async fn index_extracted_chunk(
        &self,
        chunk: &ingest::Chunk,
        extracted: &extract::ExtractedChunk,
    ) -> Result<()> {
        // Extract entity IDs
        let entity_ids: Vec<String> = extracted.extraction.entities
            .iter()
            .map(|e| e.id.clone())
            .collect();

        // Index in Qdrant (vector store)
        self.qdrant.index_chunk(chunk, entity_ids).await?;

        // Index in Neo4j (graph store)
        self.neo4j.index_extraction(&extracted.extraction).await?;

        Ok(())
    }

    /// Get overall stats
    pub async fn get_stats(&self) -> Result<IndexStats> {
        let graph_stats = self.neo4j.get_stats().await?;
        
        Ok(IndexStats {
            entities: graph_stats.entity_count,
            relations: graph_stats.relation_count,
        })
    }
}

#[derive(Debug, serde::Serialize)]
pub struct IndexStats {
    pub entities: usize,
    pub relations: usize,
}