use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QAPair {
    pub question: String,
    pub expected_answer_contains: Vec<String>,
    pub category: QuestionType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QuestionType {
    Factual,        // Simple fact lookup
    Relational,     // Requires understanding relationships
    Thematic,       // Requires synthesis across documents
    MultiHop,       // Requires multiple reasoning steps
}

pub fn get_test_set() -> Vec<QAPair> {
    vec![
        // Factual questions
        QAPair {
            question: "What is GraphRAG?".to_string(),
            expected_answer_contains: vec!["graph".to_string(), "retrieval".to_string()],
            category: QuestionType::Factual,
        },
        QAPair {
            question: "What database does GraphRAG use for vector search?".to_string(),
            expected_answer_contains: vec!["qdrant".to_string()],
            category: QuestionType::Factual,
        },
        QAPair {
            question: "What is Qdrant used for?".to_string(),
            expected_answer_contains: vec!["vector".to_string(), "search".to_string()],
            category: QuestionType::Factual,
        },
        
        // Relational questions
        QAPair {
            question: "How does GraphRAG use Neo4j?".to_string(),
            expected_answer_contains: vec!["graph".to_string(), "entities".to_string()],
            category: QuestionType::Relational,
        },
        QAPair {
            question: "What is the relationship between vector embeddings and semantic similarity?".to_string(),
            expected_answer_contains: vec!["semantic".to_string(), "similar".to_string()],
            category: QuestionType::Relational,
        },
        
        // Multi-hop questions
        QAPair {
            question: "How does the system process documents from ingestion to query?".to_string(),
            expected_answer_contains: vec!["chunk".to_string(), "extract".to_string(), "index".to_string()],
            category: QuestionType::MultiHop,
        },
        QAPair {
            question: "What stages are involved in building the knowledge graph?".to_string(),
            expected_answer_contains: vec!["entity".to_string(), "extraction".to_string(), "graph".to_string()],
            category: QuestionType::MultiHop,
        },
        
        // Thematic questions
        QAPair {
            question: "What are the main components of GraphRAG?".to_string(),
            expected_answer_contains: vec!["vector".to_string(), "graph".to_string()],
            category: QuestionType::Thematic,
        },
        QAPair {
            question: "What technologies are used in this system?".to_string(),
            expected_answer_contains: vec!["qdrant".to_string(), "neo4j".to_string()],
            category: QuestionType::Thematic,
        },
        QAPair {
            question: "What are the key concepts in retrieval augmented generation?".to_string(),
            expected_answer_contains: vec!["retrieval".to_string(), "generation".to_string()],
            category: QuestionType::Thematic,
        },
    ]
}

/// Simple scoring: check if expected keywords appear in answer
// In test_set.rs, add:
pub async fn llm_score_answer(answer: &str, question: &str, expected: &str, llm: &QueryLLM) -> f64 {
    let prompt = format!(
        "Rate the quality of this answer from 0.0 to 1.0:\n\nQuestion: {}\nAnswer: {}\n\nExpected to contain: {}\n\nScore (just the number):",
        question, answer, expected
    );
    
    let response = llm.generate(&prompt).await.unwrap_or_default();
    response.trim().parse::<f64>().unwrap_or(0.5)
}

