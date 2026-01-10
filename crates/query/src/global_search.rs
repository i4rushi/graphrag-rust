use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

use crate::llm::QueryLLM;
use index::EmbeddingClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSearchResult {
    pub answer: String,
    pub communities: Vec<CommunityReference>,
    pub trace: GlobalSearchTrace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityReference {
    pub community_id: usize,
    pub summary: String,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSearchTrace {
    pub communities_searched: usize,
    pub communities_used: usize,
}

pub struct GlobalSearchEngine {
    embedding_client: EmbeddingClient,
    llm: QueryLLM,
}

impl GlobalSearchEngine {
    pub fn new(embedding_client: EmbeddingClient, llm: QueryLLM) -> Self {
        Self {
            embedding_client,
            llm,
        }
    }

    pub async fn search(&self, query: &str, top_k: usize) -> Result<GlobalSearchResult> {
        // Step 1: Load all community summaries
        let summaries = self.load_community_summaries().await?;
        let total_communities = summaries.len();

        // Step 2: Embed the query
        let query_embedding = self.embedding_client.embed(query).await?;

        // Step 3: Score summaries by similarity
        let mut scored_summaries = Vec::new();
        
        for summary in &summaries {
            let summary_embedding = self.embedding_client.embed(&summary.summary).await?;
            let similarity = Self::cosine_similarity(&query_embedding, &summary_embedding);
            
            scored_summaries.push((summary.clone(), similarity));
        }

        // Sort by relevance
        scored_summaries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Step 4: Take top-k communities
        let top_communities: Vec<_> = scored_summaries.into_iter()
            .take(top_k)
            .collect();

        let communities_used = top_communities.len();

        // Step 5: Build global context
        let context = self.build_global_context(&top_communities);

        // Step 6: Generate synthesis
        let answer = self.generate_synthesis(query, &context).await?;

        // Build response
        let community_refs: Vec<CommunityReference> = top_communities.iter()
            .map(|(summary, score)| CommunityReference {
                community_id: summary.community_id,
                summary: summary.summary.clone(),
                relevance_score: *score,
            })
            .collect();

        Ok(GlobalSearchResult {
            answer,
            communities: community_refs,
            trace: GlobalSearchTrace {
                communities_searched: total_communities,
                communities_used,
            },
        })
    }

    async fn load_community_summaries(&self) -> Result<Vec<communities::CommunitySummary>> {
        let dir = PathBuf::from("data/communities");
        let mut summaries = Vec::new();

        let mut entries = fs::read_dir(&dir).await
            .context("Failed to read communities directory")?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content = fs::read_to_string(&path).await?;
                let summary: communities::CommunitySummary = serde_json::from_str(&content)?;
                summaries.push(summary);
            }
        }

        Ok(summaries)
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if mag_a == 0.0 || mag_b == 0.0 {
            0.0
        } else {
            dot / (mag_a * mag_b)
        }
    }

    fn build_global_context(&self, scored_summaries: &[(communities::CommunitySummary, f32)]) -> String {
        let mut context = String::new();

        context.push_str("THEMATIC COMMUNITIES:\n\n");

        for (i, (summary, score)) in scored_summaries.iter().enumerate() {
            context.push_str(&format!(
                "Community {} (relevance: {:.2}):\n{}\n\nKey entities: {}\n\n",
                i + 1,
                score,
                summary.summary,
                summary.key_entities.join(", ")
            ));
        }

        context
    }

    async fn generate_synthesis(&self, query: &str, context: &str) -> Result<String> {
        let prompt = format!(
            r#"You are a helpful assistant synthesizing information from multiple thematic communities.

COMMUNITY SUMMARIES:
{}

USER QUESTION: {}

INSTRUCTIONS:
- Synthesize a comprehensive answer drawing from the community summaries
- Identify overarching themes and patterns
- Provide a high-level overview rather than specific details
- Mention which communities are most relevant
- Be clear about the scope and limitations of your answer

SYNTHESIS:"#,
            context, query
        );

        self.llm.generate(&prompt).await
    }
}