use anyhow::{Context, Result};
use neo4rs::{Graph, Query};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::llm::QueryLLM;
use index::EmbeddingClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSearchResult {
    pub answer: String,
    pub sources: Vec<Source>,
    pub trace: SearchTrace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub chunk_id: String,
    pub text: String,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchTrace {
    pub chunks_retrieved: usize,
    pub entities_found: usize,
    pub entities_expanded: usize,
    pub context_size: usize,
}

pub struct LocalSearchEngine {
    neo4j: Graph,
    embedding_client: EmbeddingClient,
    llm: QueryLLM,
    qdrant_url: String,
    collection_name: String,
}

impl LocalSearchEngine {
    pub fn new(
        neo4j: Graph,
        embedding_client: EmbeddingClient,
        llm: QueryLLM,
        qdrant_url: String,
        collection_name: String,
    ) -> Self {
        Self {
            neo4j,
            embedding_client,
            llm,
            qdrant_url,
            collection_name,
        }
    }

    pub async fn search(&self, query: &str, top_k: usize) -> Result<LocalSearchResult> {
        // Step 1: Embed the query
        let query_embedding = self.embedding_client.embed(query).await
            .context("Failed to embed query")?;

        // Step 2: Vector search via REST API
        let points = self.search_qdrant_rest(query_embedding, top_k).await
            .context("Failed to search Qdrant")?;

        let chunks_retrieved = points.len();

        // Step 3: Extract entity IDs and build sources
        let mut entity_ids = HashSet::new();
        let mut sources = Vec::new();

        for point in &points {
            // Extract entity IDs
            for entity_id in point.entity_ids.split(',') {
                if !entity_id.is_empty() {
                    entity_ids.insert(entity_id.trim().to_string());
                }
            }

            // Build sources
            if !point.chunk_id.is_empty() && !point.text.is_empty() {
                sources.push(Source {
                    chunk_id: point.chunk_id.clone(),
                    text: point.text.clone(),
                    relevance_score: point.score,
                });
            }
        }

        let entities_found = entity_ids.len();

        // Step 4: Expand graph (1-2 hops)
        let expanded_entities = if !entity_ids.is_empty() {
            self.expand_graph(&entity_ids, 2).await?
        } else {
            HashSet::new()
        };
        let entities_expanded = expanded_entities.len();

        // Step 5: Get entity and relation details
        let entity_details = if !expanded_entities.is_empty() {
            self.get_entity_details(&expanded_entities).await?
        } else {
            Vec::new()
        };
        
        let relations = if !expanded_entities.is_empty() {
            self.get_relations(&expanded_entities).await?
        } else {
            Vec::new()
        };

        // Step 6: Build context
        let context = self.build_context(&sources, &entity_details, &relations);

        // Step 7: Generate answer
        let answer = self.generate_answer(query, &context).await?;

        Ok(LocalSearchResult {
            answer,
            sources,
            trace: SearchTrace {
                chunks_retrieved,
                entities_found,
                entities_expanded,
                context_size: context.len(),
            },
        })
    }

    async fn search_qdrant_rest(&self, query_embedding: Vec<f32>, top_k: usize) -> Result<Vec<QdrantPoint>> {
        use serde_json::json;
        
        let client = reqwest::Client::new();
        let url = format!("{}/collections/{}/points/search", self.qdrant_url, self.collection_name);
        
        let body = json!({
            "vector": query_embedding,
            "limit": top_k,
            "with_payload": true
        });

        let response = client.post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to send search request to Qdrant")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Qdrant search failed: {}", error_text);
        }

        let result: serde_json::Value = response.json().await
            .context("Failed to parse Qdrant response")?;

        // Parse the response
        let points = result["result"].as_array()
            .context("Invalid Qdrant response format")?;

        let mut parsed_points = Vec::new();
        for point in points {
            let score = point["score"].as_f64().unwrap_or(0.0) as f32;
            let payload = point["payload"].as_object()
                .context("Missing payload")?;

            let chunk_id = payload.get("chunk_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            
            let text = payload.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            
            let entity_ids = payload.get("entity_ids")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            parsed_points.push(QdrantPoint {
                score,
                chunk_id,
                text,
                entity_ids,
            });
        }

        Ok(parsed_points)
    }

    async fn expand_graph(&self, seed_entities: &HashSet<String>, hops: usize) -> Result<HashSet<String>> {
        let mut expanded = seed_entities.clone();

        for _ in 0..hops {
            let current_entities: Vec<String> = expanded.iter().cloned().collect();
            
            if current_entities.is_empty() {
                break;
            }
            
            let query = Query::new(
                r#"
                MATCH (e:Entity)-[r:RELATION]-(neighbor:Entity)
                WHERE e.id IN $entity_ids
                RETURN DISTINCT neighbor.id as neighbor_id
                "#.to_string()
            ).param("entity_ids", current_entities);

            let mut result = self.neo4j.execute(query).await?;

            while let Some(row) = result.next().await? {
                if let Ok(neighbor_id) = row.get::<String>("neighbor_id") {
                    expanded.insert(neighbor_id);
                }
            }
        }

        Ok(expanded)
    }

    async fn get_entity_details(&self, entity_ids: &HashSet<String>) -> Result<Vec<EntityDetail>> {
        let mut entities = Vec::new();
        let entity_list: Vec<String> = entity_ids.iter().cloned().collect();

        if entity_list.is_empty() {
            return Ok(entities);
        }

        let query = Query::new(
            r#"
            MATCH (e:Entity)
            WHERE e.id IN $entity_ids
            RETURN e.id as id, e.name as name, e.type as type, e.description as description
            "#.to_string()
        ).param("entity_ids", entity_list);

        let mut result = self.neo4j.execute(query).await?;

        while let Some(row) = result.next().await? {
            entities.push(EntityDetail {
                name: row.get("name")?,
                entity_type: row.get("type").unwrap_or_else(|_| "UNKNOWN".to_string()),
                description: row.get("description").unwrap_or_else(|_| String::new()),
            });
        }

        Ok(entities)
    }

    async fn get_relations(&self, entity_ids: &HashSet<String>) -> Result<Vec<RelationDetail>> {
        let mut relations = Vec::new();
        let entity_list: Vec<String> = entity_ids.iter().cloned().collect();

        if entity_list.is_empty() {
            return Ok(relations);
        }

        let query = Query::new(
            r#"
            MATCH (source:Entity)-[r:RELATION]->(target:Entity)
            WHERE source.id IN $entity_ids AND target.id IN $entity_ids
            RETURN source.id as source, r.type as relation, target.id as target, r.evidence as evidence
            LIMIT 50
            "#.to_string()
        ).param("entity_ids", entity_list);

        let mut result = self.neo4j.execute(query).await?;

        while let Some(row) = result.next().await? {
            relations.push(RelationDetail {
                source: row.get("source")?,
                relation: row.get("relation")?,
                target: row.get("target")?,
                evidence: row.get("evidence").unwrap_or_else(|_| String::new()),
            });
        }

        Ok(relations)
    }

    fn build_context(
        &self,
        sources: &[Source],
        entities: &[EntityDetail],
        relations: &[RelationDetail],
    ) -> String {
        let mut context = String::new();

        context.push_str("RELEVANT TEXT CHUNKS:\n");
        for (i, source) in sources.iter().take(5).enumerate() {
            context.push_str(&format!("[Chunk {}] {}\n\n", i + 1, source.text));
        }

        if !entities.is_empty() {
            context.push_str("\nRELEVANT ENTITIES:\n");
            for entity in entities.iter().take(10) {
                context.push_str(&format!(
                    "- {} ({}): {}\n",
                    entity.name, entity.entity_type, entity.description
                ));
            }
        }

        if !relations.is_empty() {
            context.push_str("\nKEY RELATIONSHIPS:\n");
            for relation in relations.iter().take(10) {
                context.push_str(&format!(
                    "- {} {} {} (Evidence: {})\n",
                    relation.source, relation.relation, relation.target, relation.evidence
                ));
            }
        }

        context
    }

    async fn generate_answer(&self, query: &str, context: &str) -> Result<String> {
        let prompt = format!(
            r#"You are a helpful assistant answering questions based on the provided context.

CONTEXT:
{}

USER QUESTION: {}

INSTRUCTIONS:
- Answer the question using only information from the context above
- Be specific and cite relevant chunks, entities, or relationships
- If the context doesn't contain enough information, say so
- Keep your answer concise and factual

ANSWER:"#,
            context, query
        );

        self.llm.generate(&prompt).await
    }
}

#[derive(Debug, Clone)]
struct QdrantPoint {
    score: f32,
    chunk_id: String,
    text: String,
    entity_ids: String,
}

#[derive(Debug, Clone)]
struct EntityDetail {
    name: String,
    entity_type: String,
    description: String,
}

#[derive(Debug, Clone)]
struct RelationDetail {
    source: String,
    relation: String,
    target: String,
    evidence: String,
}