use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::test_set::{QAPair, QuestionType, score_answer};
use crate::vanilla_rag::VanillaRAG;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub vanilla_rag: MethodResults,
    pub graphrag_local: MethodResults,
    pub graphrag_global: MethodResults,
    pub comparison: Comparison,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodResults {
    pub method: String,
    pub total_queries: usize,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub avg_quality_score: f64,
    pub by_category: Vec<CategoryScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryScore {
    pub category: String,
    pub avg_quality: f64,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comparison {
    pub local_vs_vanilla_quality_improvement: f64,
    pub global_vs_vanilla_quality_improvement: f64,
    pub local_vs_vanilla_latency_ratio: f64,
}

pub struct Benchmarker {
    vanilla_rag: VanillaRAG,
    api_base_url: String,
}

impl Benchmarker {
    pub fn new(vanilla_rag: VanillaRAG, api_base_url: String) -> Self {
        Self {
            vanilla_rag,
            api_base_url,
        }
    }

    pub async fn run_benchmark(&self, test_set: &[QAPair]) -> Result<BenchmarkResults> {
        println!("Running benchmark with {} questions...", test_set.len());

        // Run vanilla RAG
        println!("Testing Vanilla RAG...");
        let vanilla_results = self.test_vanilla(test_set).await?;

        // Run GraphRAG Local
        println!("Testing GraphRAG Local...");
        let local_results = self.test_graphrag_local(test_set).await?;

        // Run GraphRAG Global  
        println!("Testing GraphRAG Global...");
        let global_results = self.test_graphrag_global(test_set).await?;

        // Calculate comparison
        let comparison = Comparison {
            local_vs_vanilla_quality_improvement: 
                (local_results.avg_quality_score - vanilla_results.avg_quality_score) / vanilla_results.avg_quality_score * 100.0,
            global_vs_vanilla_quality_improvement:
                (global_results.avg_quality_score - vanilla_results.avg_quality_score) / vanilla_results.avg_quality_score * 100.0,
            local_vs_vanilla_latency_ratio:
                local_results.avg_latency_ms / vanilla_results.avg_latency_ms,
        };

        Ok(BenchmarkResults {
            vanilla_rag: vanilla_results,
            graphrag_local: local_results,
            graphrag_global: global_results,
            comparison,
        })
    }

    async fn test_vanilla(&self, test_set: &[QAPair]) -> Result<MethodResults> {
        let mut latencies = Vec::new();
        let mut scores = Vec::new();
        let mut category_scores: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();

        for qa in test_set {
            let result = self.vanilla_rag.search(&qa.question, 5).await?;
            
            latencies.push(result.query_time_ms as f64);
            let score = score_answer(&result.answer, &qa.expected_answer_contains);
            scores.push(score);

            let cat_name = format!("{:?}", qa.category);
            category_scores.entry(cat_name).or_insert_with(Vec::new).push(score);
        }

        Ok(self.compute_results("Vanilla RAG".to_string(), latencies, scores, category_scores))
    }

    async fn test_graphrag_local(&self, test_set: &[QAPair]) -> Result<MethodResults> {
        let mut latencies = Vec::new();
        let mut scores = Vec::new();
        let mut category_scores: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();

        let client = reqwest::Client::new();

        for qa in test_set {
            let start = Instant::now();
            
            let response = client
                .post(&format!("{}/query/local", self.api_base_url))
                .json(&serde_json::json!({
                    "query": qa.question,
                    "top_k": 5
                }))
                .send()
                .await?;

            let result: serde_json::Value = response.json().await?;
            let latency = start.elapsed().as_millis() as f64;
            
            latencies.push(latency);
            
            let answer = result["answer"].as_str().unwrap_or("");
            let score = score_answer(answer, &qa.expected_answer_contains);
            scores.push(score);

            let cat_name = format!("{:?}", qa.category);
            category_scores.entry(cat_name).or_insert_with(Vec::new).push(score);
        }

        Ok(self.compute_results("GraphRAG Local".to_string(), latencies, scores, category_scores))
    }

    async fn test_graphrag_global(&self, test_set: &[QAPair]) -> Result<MethodResults> {
        let mut latencies = Vec::new();
        let mut scores = Vec::new();
        let mut category_scores: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();

        let client = reqwest::Client::new();

        for qa in test_set {
            // Only test global on thematic questions
            if qa.category != QuestionType::Thematic {
                continue;
            }

            let start = Instant::now();
            
            let response = client
                .post(&format!("{}/query/global", self.api_base_url))
                .json(&serde_json::json!({
                    "query": qa.question,
                    "top_k": 3
                }))
                .send()
                .await?;

            let result: serde_json::Value = response.json().await?;
            let latency = start.elapsed().as_millis() as f64;
            
            latencies.push(latency);
            
            let answer = result["answer"].as_str().unwrap_or("");
            let score = score_answer(answer, &qa.expected_answer_contains);
            scores.push(score);

            let cat_name = format!("{:?}", qa.category);
            category_scores.entry(cat_name).or_insert_with(Vec::new).push(score);
        }

        Ok(self.compute_results("GraphRAG Global".to_string(), latencies, scores, category_scores))
    }

    fn compute_results(
        &self,
        method: String,
        mut latencies: Vec<f64>,
        scores: Vec<f64>,
        category_scores: std::collections::HashMap<String, Vec<f64>>,
    ) -> MethodResults {
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let avg_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let p50_latency = percentile(&latencies, 50);
        let p95_latency = percentile(&latencies, 95);
        let avg_quality = scores.iter().sum::<f64>() / scores.len() as f64;

        let by_category = category_scores.into_iter()
            .map(|(cat, scores)| CategoryScore {
                category: cat,
                avg_quality: scores.iter().sum::<f64>() / scores.len() as f64,
                count: scores.len(),
            })
            .collect();

        MethodResults {
            method,
            total_queries: latencies.len(),
            avg_latency_ms: avg_latency,
            p50_latency_ms: p50_latency,
            p95_latency_ms: p95_latency,
            avg_quality_score: avg_quality,
            by_category,
        }
    }
}

fn percentile(sorted_data: &[f64], p: usize) -> f64 {
    let index = (p as f64 / 100.0 * sorted_data.len() as f64) as usize;
    sorted_data[index.min(sorted_data.len() - 1)]
}