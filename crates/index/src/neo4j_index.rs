use anyhow::{Context, Result};
use neo4rs::{Graph, Query};

pub struct Neo4jIndexer {
    graph: Graph,
}

impl Neo4jIndexer {
    pub fn new(graph: Graph) -> Self {
        Self { graph }
    }

    /// Initialize schema: create indexes
    pub async fn init_schema(&self) -> Result<()> {
        println!("Creating Neo4j indexes...");

        // Create index on Entity.id
        let query = Query::new(
            "CREATE INDEX entity_id_index IF NOT EXISTS FOR (e:Entity) ON (e.id)".to_string()
        );
        self.graph.run(query).await
            .context("Failed to create index on Entity.id")?;

        // Create index on Entity.name
        let query = Query::new(
            "CREATE INDEX entity_name_index IF NOT EXISTS FOR (e:Entity) ON (e.name)".to_string()
        );
        self.graph.run(query).await
            .context("Failed to create index on Entity.name")?;

        println!("Neo4j indexes created successfully");
        Ok(())
    }

    /// Index an entity (MERGE to avoid duplicates)
    pub async fn index_entity(&self, entity: &extract::Entity) -> Result<()> {
        let query = Query::new(
            r#"
            MERGE (e:Entity {id: $id})
            SET e.name = $name,
                e.type = $type,
                e.description = $description
            "#.to_string()
        )
        .param("id", entity.id.clone())
        .param("name", entity.name.clone())
        .param("type", entity.entity_type.clone())
        .param("description", entity.description.clone());

        self.graph.run(query).await
            .context("Failed to index entity")?;

        Ok(())
    }

    /// Index a relation
    pub async fn index_relation(&self, relation: &extract::Relation) -> Result<()> {
        // First ensure both entities exist
        self.ensure_entity_exists(&relation.source).await?;
        self.ensure_entity_exists(&relation.target).await?;

        // Create relationship (using MERGE to avoid duplicates)
        let query = Query::new(
            r#"
            MATCH (source:Entity {id: $source_id})
            MATCH (target:Entity {id: $target_id})
            MERGE (source)-[r:RELATION {type: $relation_type}]->(target)
            SET r.evidence = $evidence
            "#.to_string()
        )
        .param("source_id", relation.source.clone())
        .param("target_id", relation.target.clone())
        .param("relation_type", relation.relation.clone())
        .param("evidence", relation.evidence.clone());

        self.graph.run(query).await
            .context("Failed to index relation")?;

        Ok(())
    }

    /// Ensure an entity exists (minimal placeholder if not)
    async fn ensure_entity_exists(&self, entity_id: &str) -> Result<()> {
        let query = Query::new(
            r#"
            MERGE (e:Entity {id: $id})
            ON CREATE SET e.name = $id, e.type = 'UNKNOWN', e.description = 'Auto-created'
            "#.to_string()
        )
        .param("id", entity_id.to_string());

        self.graph.run(query).await
            .context("Failed to ensure entity exists")?;

        Ok(())
    }

    /// Batch index extracted data
    pub async fn index_extraction(
        &self,
        extraction: &extract::ExtractionResult,
    ) -> Result<()> {
        // Index all entities
        for entity in &extraction.entities {
            self.index_entity(entity).await?;
        }

        // Index all relations
        for relation in &extraction.relations {
            self.index_relation(relation).await?;
        }

        Ok(())
    }

    /// Get graph statistics
    pub async fn get_stats(&self) -> Result<GraphStats> {
        // Count entities
        let entity_query = Query::new("MATCH (e:Entity) RETURN count(e) as count".to_string());
        let mut result = self.graph.execute(entity_query).await?;
        let entity_count = if let Some(row) = result.next().await? {
            row.get::<i64>("count").unwrap_or(0) as usize
        } else {
            0
        };

        // Count relations
        let relation_query = Query::new("MATCH ()-[r:RELATION]->() RETURN count(r) as count".to_string());
        let mut result = self.graph.execute(relation_query).await?;
        let relation_count = if let Some(row) = result.next().await? {
            row.get::<i64>("count").unwrap_or(0) as usize
        } else {
            0
        };

        Ok(GraphStats {
            entity_count,
            relation_count,
        })
    }
}

#[derive(Debug)]
pub struct GraphStats {
    pub entity_count: usize,
    pub relation_count: usize,
}