use anyhow::{Context, Result};
use neo4rs::{Graph, Query};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct GraphData {
    pub entities: Vec<String>,
    pub edges: Vec<(usize, usize)>, // (source_idx, target_idx)
    pub entity_to_idx: HashMap<String, usize>,
}

impl GraphData {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            edges: Vec::new(),
            entity_to_idx: HashMap::new(),
        }
    }

    pub fn add_entity(&mut self, entity_id: String) -> usize {
        if let Some(&idx) = self.entity_to_idx.get(&entity_id) {
            return idx;
        }

        let idx = self.entities.len();
        self.entities.push(entity_id.clone());
        self.entity_to_idx.insert(entity_id, idx);
        idx
    }

    pub fn add_edge(&mut self, source: usize, target: usize) {
        self.edges.push((source, target));
    }
}

pub struct GraphExporter {
    graph: Graph,
}

impl GraphExporter {
    pub fn new(graph: Graph) -> Self {
        Self { graph }
    }

    /// Export all entities and relationships from Neo4j
    pub async fn export_graph(&self) -> Result<GraphData> {
        let mut graph_data = GraphData::new();

        // Get all relationships (this implicitly gets entities too)
        let query = Query::new(
            r#"
            MATCH (source:Entity)-[r:RELATION]->(target:Entity)
            RETURN source.id as source_id, target.id as target_id
            "#.to_string()
        );

        let mut result = self.graph.execute(query).await
            .context("Failed to export graph from Neo4j")?;

        while let Some(row) = result.next().await? {
            let source_id: String = row.get("source_id")
                .context("Missing source_id")?;
            let target_id: String = row.get("target_id")
                .context("Missing target_id")?;

            let source_idx = graph_data.add_entity(source_id);
            let target_idx = graph_data.add_entity(target_id);
            graph_data.add_edge(source_idx, target_idx);
        }

        println!("Exported graph: {} entities, {} edges", 
            graph_data.entities.len(), 
            graph_data.edges.len()
        );

        Ok(graph_data)
    }

    /// Get entity details for a community
    pub async fn get_community_entities(
        &self,
        entity_ids: &[String],
    ) -> Result<Vec<EntityInfo>> {
        let mut entities = Vec::new();

        for entity_id in entity_ids {
            let query = Query::new(
                "MATCH (e:Entity {id: $id}) RETURN e.name as name, e.type as type, e.description as description".to_string()
            ).param("id", entity_id.clone());

            let mut result = self.graph.execute(query).await?;
            
            if let Some(row) = result.next().await? {
                entities.push(EntityInfo {
                    id: entity_id.clone(),
                    name: row.get("name").unwrap_or_else(|_| entity_id.clone()),
                    entity_type: row.get("type").unwrap_or_else(|_| "UNKNOWN".to_string()),
                    description: row.get("description").unwrap_or_else(|_| String::new()),
                });
            }
        }

        Ok(entities)
    }

    /// Get key relationships within a community
    pub async fn get_community_relations(
        &self,
        entity_ids: &[String],
    ) -> Result<Vec<RelationInfo>> {
        let mut relations = Vec::new();

        // Build a Cypher query to get relations within the community
        let query = Query::new(
            r#"
            MATCH (source:Entity)-[r:RELATION]->(target:Entity)
            WHERE source.id IN $entity_ids AND target.id IN $entity_ids
            RETURN source.id as source_id, r.type as relation_type, 
                   target.id as target_id, r.evidence as evidence
            LIMIT 20
            "#.to_string()
        ).param("entity_ids", entity_ids.to_vec());

        let mut result = self.graph.execute(query).await?;

        while let Some(row) = result.next().await? {
            relations.push(RelationInfo {
                source: row.get("source_id")?,
                relation: row.get("relation_type")?,
                target: row.get("target_id")?,
                evidence: row.get("evidence").unwrap_or_else(|_| String::new()),
            });
        }

        Ok(relations)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EntityInfo {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RelationInfo {
    pub source: String,
    pub relation: String,
    pub target: String,
    pub evidence: String,
}