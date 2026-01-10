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

pub mod graph_export;
pub mod louvain;
pub mod summarizer;

pub use graph_export::{GraphExporter, GraphData, EntityInfo, RelationInfo};
pub use louvain::LouvainDetector;
pub use summarizer::{CommunitySummarizer, CommunitySummary};

use anyhow::Result;
use neo4rs::{Graph, Query};
use std::collections::HashMap;

pub struct CommunityDetector {
    exporter: GraphExporter,
    summarizer: CommunitySummarizer,
    graph: Graph,
}

impl CommunityDetector {
    pub fn new(graph: Graph, summarizer: CommunitySummarizer) -> Self {
        let exporter = GraphExporter::new(graph.clone());
        Self {
            exporter,
            summarizer,
            graph,
        }
    }

    /// Full pipeline: detect communities and generate summaries
    pub async fn detect_and_summarize(&self) -> Result<Vec<CommunitySummary>> {
        // Step 1: Export graph
        println!("Exporting graph from Neo4j...");
        let graph_data = self.exporter.export_graph().await?;

        if graph_data.entities.is_empty() {
            println!("No entities found in graph");
            return Ok(Vec::new());
        }

        // Step 2: Run community detection
        println!("Running Louvain community detection...");
        let detector = LouvainDetector::new(graph_data.clone());
        let communities = detector.detect_communities();

        // Step 3: Assign communities in Neo4j
        println!("Assigning communities in Neo4j...");
        self.assign_communities(&communities).await?;

        // Step 4: Group entities by community
        let mut community_groups: HashMap<usize, Vec<String>> = HashMap::new();
        for (entity_id, &comm_id) in &communities {
            community_groups.entry(comm_id)
                .or_insert_with(Vec::new)
                .push(entity_id.clone());
        }

        // Step 5: Generate summaries for each community
        println!("Generating community summaries...");
        let mut summaries = Vec::new();

        for (&comm_id, entity_ids) in &community_groups {
            println!("Processing community {} ({} entities)...", comm_id, entity_ids.len());

            let entities = self.exporter.get_community_entities(entity_ids).await?;
            let relations = self.exporter.get_community_relations(entity_ids).await?;

            let summary = self.summarizer
                .summarize_community(comm_id, &entities, &relations)
                .await?;

            summaries.push(summary);
        }

        Ok(summaries)
    }

    /// Assign community IDs to entities in Neo4j
    async fn assign_communities(&self, communities: &HashMap<String, usize>) -> Result<()> {
        for (entity_id, &comm_id) in communities {
            let query = Query::new(
                "MATCH (e:Entity {id: $id}) SET e.community_id = $comm_id".to_string()
            )
            .param("id", entity_id.clone())
            .param("comm_id", comm_id as i64);

            self.graph.run(query).await?;
        }

        Ok(())
    }
}