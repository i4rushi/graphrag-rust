use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::embeddings::EmbeddingClient;

pub struct QdrantIndexer {
    base_url: String,
    client: reqwest::Client,
    embedding_client: EmbeddingClient,
    collection_name: String,
}

#[derive(Serialize)]
struct CreateCollection {
    vectors: VectorParams,
}

#[derive(Serialize)]
struct VectorParams {
    size: usize,
    distance: String,
}

#[derive(Serialize)]
struct UpsertPoints {
    points: Vec<Point>,
}

#[derive(Serialize)]
struct Point {
    id: u64,
    vector: Vec<f32>,
    payload: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct CollectionInfo {
    result: CollectionResult,
}

#[derive(Deserialize)]
struct CollectionResult {
    collections: Vec<Collection>,
}

#[derive(Deserialize)]
struct Collection {
    name: String,
}

impl QdrantIndexer {
    pub fn new(
        base_url: String,
        embedding_client: EmbeddingClient,
        collection_name: String,
    ) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
            embedding_client,
            collection_name,
        }
    }

    /// Initialize collection with proper schema
    pub async fn init_collection(&self) -> Result<()> {
        // Check if collection exists
        let url = format!("{}/collections", self.base_url);
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to list collections: {}", response.status());
        }

        let info: CollectionInfo = response.json().await?;
        let exists = info.result.collections.iter()
            .any(|c| c.name == self.collection_name);

        if exists {
            println!("Collection '{}' already exists", self.collection_name);
            return Ok(());
        }

        // Get embedding dimension
        let dimension = self.embedding_client.get_dimension().await?;
        println!("Creating collection with dimension: {}", dimension);

        // Create collection
        let url = format!("{}/collections/{}", self.base_url, self.collection_name);
        let create_req = CreateCollection {
            vectors: VectorParams {
                size: dimension,
                distance: "Cosine".to_string(),
            },
        };

        let response = self.client
            .put(&url)
            .json(&create_req)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to create collection: {}", error_text);
        }

        println!("Collection '{}' created successfully", self.collection_name);
        Ok(())
    }

    /// Index a chunk with its embedding
    pub async fn index_chunk(
        &self,
        chunk: &ingest::Chunk,
        entity_ids: Vec<String>,
    ) -> Result<()> {
        // Generate embedding
        let embedding = self.embedding_client
            .embed(&chunk.text)
            .await
            .context("Failed to generate embedding")?;

        // Build payload
        let mut payload = HashMap::new();
        payload.insert(
            "chunk_id".to_string(),
            serde_json::json!(chunk.chunk_id.clone()),
        );
        payload.insert(
            "doc_id".to_string(),
            serde_json::json!(chunk.doc_id.clone()),
        );
        payload.insert(
            "text".to_string(),
            serde_json::json!(chunk.text.clone()),
        );
        payload.insert(
            "source".to_string(),
            serde_json::json!(chunk.source.clone()),
        );
        payload.insert(
            "entity_ids".to_string(),
            serde_json::json!(entity_ids.join(",")),
        );

        // Use chunk_id as point ID (hash to u64)
        let point_id = self.hash_to_u64(&chunk.chunk_id);

        // Create point
        let point = Point {
            id: point_id,
            vector: embedding,
            payload,
        };

        // Upsert point
        let url = format!(
            "{}/collections/{}/points",
            self.base_url, self.collection_name
        );
        
        let upsert_req = UpsertPoints {
            points: vec![point],
        };

        let response = self.client
            .put(&url)
            .json(&upsert_req)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to upsert point: {}", error_text);
        }

        Ok(())
    }

    /// Simple hash function to convert string ID to u64
    fn hash_to_u64(&self, s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    /// Get collection info
    pub async fn collection_info(&self) -> Result<()> {
        let url = format!(
            "{}/collections/{}",
            self.base_url, self.collection_name
        );
        
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to get collection info: {}", response.status());
        }

        let info: serde_json::Value = response.json().await?;
        println!("Collection info: {}", serde_json::to_string_pretty(&info)?);
        Ok(())
    }
}