use anyhow::Result;
use serde::{Deserialize, Serialize};

use index::EmbeddingClient;
use query::QueryLLM;

/// Vanilla RAG: just vector search + LLM, no graph
pub struct VanillaRAG {
    embedding_client: EmbeddingClient,
    llm: QueryLLM,
    qdrant_url: String,
    collection_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VanillaSearchResult {
    pub answer: String,
    pub sources: Vec<Source>,
    pub query_time_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub chunk_id: String,
    pub text: String,
    pub score: f32,
}

impl VanillaRAG {
    pub fn new(
        embedding_client: EmbeddingClient,
        llm: QueryLLM,
        qdrant_url: String,
        collection_name: String,
    ) -> Self {
        Self {
            embedding_client,
            llm,
            qdrant_url,
            collection_name,
        }
    }

    pub async fn search(&self, query: &str, top_k: usize) -> Result<VanillaSearchResult> {
        let start = std::time::Instant::now();

        // Step 1: Embed query
        let query_embedding = self.embedding_client.embed(query).await?;

        // Step 2: Vector search
        let sources = self.vector_search(query_embedding, top_k).await?;

        // Step 3: Build simple context (just chunks, no graph)
        let context = self.build_context(&sources);

        // Step 4: Generate answer
        let answer = self.generate_answer(query, &context).await?;

        Ok(VanillaSearchResult {
            answer,
            sources,
            query_time_ms: start.elapsed().as_millis(),
        })
    }

    async fn vector_search(&self, embedding: Vec<f32>, top_k: usize) -> Result<Vec<Source>> {
        use serde_json::json;

        let client = reqwest::Client::new();
        let url = format!("{}/collections/{}/points/search", self.qdrant_url, self.collection_name);

        let body = json!({
            "vector": embedding,
            "limit": top_k,
            "with_payload": true
        });

        let response = client.post(&url).json(&body).send().await?;
        let result: serde_json::Value = response.json().await?;

        let points = result["result"].as_array().unwrap();
        let mut sources = Vec::new();

        for point in points {
            let score = point["score"].as_f64().unwrap_or(0.0) as f32;
            let payload = point["payload"].as_object().unwrap();

            sources.push(Source {
                chunk_id: payload.get("chunk_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                text: payload.get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                score,
            });
        }

        Ok(sources)
    }

    fn build_context(&self, sources: &[Source]) -> String {
        let mut context = String::new();
        context.push_str("RELEVANT CHUNKS:\n\n");

        for (i, source) in sources.iter().enumerate() {
            context.push_str(&format!("[Chunk {}] {}\n\n", i + 1, source.text));
        }

        context
    }

    async fn generate_answer(&self, query: &str, context: &str) -> Result<String> {
        let prompt = format!(
            r#"Answer the question based on the provided context.

CONTEXT:
{}

QUESTION: {}

ANSWER:"#,
            context, query
        );

        self.llm.generate(&prompt).await
    }
}