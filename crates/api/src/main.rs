use axum::{
    extract::{State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber;
use qdrant_client::Qdrant;

// Import from your other crates
//use query::local_search::LocalSearchEngine;
//use query::llm::QueryLLM;

mod config;
mod cache;
mod retry;
mod metrics;
mod request_id;

use config::AppConfig;
use cache::Cache;
use retry::RetryPolicy;
use metrics::{Metrics, TimedOperation};
use request_id::{RequestId, request_id_middleware};

#[derive(Clone)]
struct AppState {
    neo4j_graph: neo4rs::Graph,
    extractor: Arc<Mutex<extract::Extractor>>,
    indexer: std::sync::Arc<index::Indexer>,
    community_detector: std::sync::Arc<communities::CommunityDetector>,
    local_search: std::sync::Arc<query::LocalSearchEngine>,
    global_search: std::sync::Arc<query::GlobalSearchEngine>,
    config: AppConfig,
    cache: Arc<Cache>,
    metrics: Arc<Metrics>,
    retry_policy: Arc<RetryPolicy>,
    llm_semaphore: Arc<tokio::sync::Semaphore>,
}

#[derive(Serialize)]
struct HealthResponse {
    qdrant: String,
    neo4j: String,
}

#[derive(Deserialize)]
struct IngestRequest {
    path: String,
}

#[derive(Serialize)]
struct IngestResponse {
    chunks_created: usize,
    doc_ids: Vec<String>,
}

#[derive(Deserialize)]
struct ExtractRequest {
    /// Optional: extract from specific chunk file
    chunk_file: Option<String>,
}

#[derive(Serialize)]
struct ExtractResponse {
    chunks_processed: usize,
    entities_extracted: usize,
    relations_extracted: usize,
}

// Added Search Request/Response structs
// #[derive(Deserialize)]
// struct SearchRequest {
//     q: String,
// }

// #[derive(Serialize)]
// struct SearchResponse {
//     answer: String,
//     sources: Vec<query::local_search::Source>,
// }

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .json()
        .init();

    // Load config
    let config = AppConfig::default(); // Or load from file/env
    
    tracing::info!(mode = ?config.mode, "Starting GraphRAG API");

    // Initialize cache
    let cache = Arc::new(Cache::new(config.cache.max_entries));

    // Initialize metrics
    let metrics = Metrics::new();

    // Initialize retry policy
    let retry_policy = Arc::new(RetryPolicy::new(
        config.retry.max_retries,
        config.retry.initial_backoff_ms,
        config.retry.max_backoff_ms,
    ));

    // Concurrency control
    let llm_semaphore = Arc::new(tokio::sync::Semaphore::new(
        config.concurrency.max_concurrent_llm_calls
    ));

    // Connect to Neo4j
    let neo4j_graph = neo4rs::Graph::new(
        "bolt://localhost:7687",
        "neo4j",
        "yourpassword",
    )
    .await
    .expect("Failed to connect to Neo4j");

    // Create extractor
    let extractor = extract::Extractor::default();

    // Create embedding client
    let embedding_client = index::EmbeddingClient::default();

    // Create Qdrant indexer (using REST API)
    let _qdrant_client = Qdrant::from_url("http://localhost:6333")
        .build()
        .expect("Failed to create Qdrant client");

    let qdrant_indexer = index::QdrantIndexer::new(
        "http://localhost:6333".to_string(),
        embedding_client,
        "graphrag_chunks".to_string(),
    );
    
    let neo4j_indexer = index::Neo4jIndexer::new(neo4j_graph.clone());

    // Create unified indexer
    let indexer = index::Indexer::new(qdrant_indexer, neo4j_indexer);
    
    // Initialize stores
    indexer.init().await.expect("Failed to initialize indexer");

    let community_summarizer = communities::CommunitySummarizer::default();
    let community_detector = communities::CommunityDetector::new(
        neo4j_graph.clone(),
        community_summarizer,
    );

    let query_llm = query::QueryLLM::default();
    let query_embedding_client = index::EmbeddingClient::default();

    let local_search = query::LocalSearchEngine::new(
        //qdrant_client.clone(),
        neo4j_graph.clone(),
        query_embedding_client.clone(),
        query_llm.clone(),
        "http://localhost:6333".to_string(), 
        "graphrag_chunks".to_string(),
    );

    let global_search = query::GlobalSearchEngine::new(
        query_embedding_client,
        query_llm,
    );

    let state = Arc::new(AppState {
        neo4j_graph,
        extractor: Arc::new(Mutex::new(extractor)),
        indexer: Arc::new(indexer),
        community_detector: Arc::new(community_detector),
        local_search: Arc::new(local_search),
        global_search: Arc::new(global_search),
        config,
        cache,
        metrics,
        retry_policy,
        llm_semaphore,
    });

    // Build router
    let app = Router::new()
        .route("/health", post(health_check))
        .route("/health", get(health_check))
        .route("/ingest", post(ingest_document))
        .route("/extract", post(extract_chunks))
        .route("/index", post(index_data))
        //.route("/stats", get(get_stats))
        .route("/communities", post(detect_communities))
        .route("/query/local", post(query_local))
        .route("/query/global", post(query_global))
        .route("/stats", get(get_stats))
        .route("/metrics", get(get_metrics))
        .route("/cache/stats", get(get_cache_stats))
        .route("/cache/clear", post(clear_cache))
        .route("/config", get(get_config))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    
    tracing::info!("Server listening on http://localhost:3000");
    
    axum::serve(listener, app).await.unwrap();
}

async fn get_metrics(
    State(state): State<Arc<AppState>>,
) -> Json<metrics::MetricsSnapshot> {
    Json(state.metrics.snapshot())
}

async fn get_cache_stats(
    State(state): State<Arc<AppState>>,
) -> Json<cache::CacheStats> {
    Json(state.cache.stats())
}

async fn clear_cache(
    State(state): State<Arc<AppState>>,
) -> StatusCode {
    state.cache.clear();
    StatusCode::OK
}

async fn get_config(
    State(state): State<Arc<AppState>>,
) -> Json<AppConfig> {
    Json(state.config.clone())
}

async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Result<Json<HealthResponse>, StatusCode> {
    // Check Qdrant with REST API
    let qdrant_status = match reqwest::get("http://localhost:6333/").await {
        Ok(resp) if resp.status().is_success() => "ok".to_string(),
        Ok(resp) => format!("error: status {}", resp.status()),
        Err(e) => format!("error: {}", e),
    };

    // Check Neo4j with a simple query
    let neo4j_status = match state.neo4j_graph.run(neo4rs::query("RETURN 1")).await {
        Ok(_) => "ok".to_string(),
        Err(e) => format!("error: {}", e),
    };

    Ok(Json(HealthResponse {
        qdrant: qdrant_status,
        neo4j: neo4j_status,
    }))
}

async fn ingest_document(
    Json(req): Json<IngestRequest>,
) -> Result<Json<IngestResponse>, StatusCode> {
    let path = PathBuf::from(&req.path);
    
    if !path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }
    
    let chunks = if path.is_file() {
        ingest::ingest_file(&path)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else if path.is_dir() {
        ingest::ingest_directory(&path)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };
    
    // Save chunks to disk (data/chunks/)
    let output_dir = PathBuf::from("data/chunks");
    tokio::fs::create_dir_all(&output_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let mut doc_ids = std::collections::HashSet::new();
    
    for chunk in &chunks {
        doc_ids.insert(chunk.doc_id.clone());
        
        let chunk_file = output_dir.join(format!("{}.json", chunk.chunk_id));
        let json = serde_json::to_string_pretty(chunk)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        tokio::fs::write(chunk_file, json)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    
    Ok(Json(IngestResponse {
        chunks_created: chunks.len(),
        doc_ids: doc_ids.into_iter().collect(),
    }))
}

async fn extract_chunks(
    State(state): State<Arc<AppState>>,
    req: Option<Json<ExtractRequest>>,
) -> Result<Json<ExtractResponse>, StatusCode> {
    let chunks_dir = PathBuf::from("data/chunks");
    
    // Read chunk files
    let chunk_files: Vec<PathBuf> = if let Some(Json(req)) = req {
        if let Some(chunk_file) = req.chunk_file {
            vec![chunks_dir.join(chunk_file)]
        } else {
            read_chunk_files(&chunks_dir).await?
        }
    } else {
        read_chunk_files(&chunks_dir).await?
    };

    let mut total_entities = 0;
    let mut total_relations = 0;
    let output_dir = PathBuf::from("data/extracted");
    tokio::fs::create_dir_all(&output_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for chunk_path in &chunk_files {
        // Read chunk
        let chunk_json = tokio::fs::read_to_string(chunk_path)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        let chunk: ingest::Chunk = serde_json::from_str(&chunk_json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Extract entities and relations
        let mut extractor = state.extractor.lock().await;
        
        let extracted = extractor
            .extract_chunk(chunk.chunk_id.clone(), chunk.doc_id.clone(), &chunk.text)
            .await
            .map_err(|e| {
                eprintln!("Extraction error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        total_entities += extracted.extraction.entities.len();
        total_relations += extracted.extraction.relations.len();

        // Save extracted data
        let output_file = output_dir.join(format!("{}.json", chunk.chunk_id));
        let json = serde_json::to_string_pretty(&extracted)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        tokio::fs::write(output_file, json)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(ExtractResponse {
        chunks_processed: chunk_files.len(),
        entities_extracted: total_entities,
        relations_extracted: total_relations,
    }))
}

// Helper function to read chunk files from directory
async fn read_chunk_files(chunks_dir: &PathBuf) -> Result<Vec<PathBuf>, StatusCode> {
    let mut entries = tokio::fs::read_dir(&chunks_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let mut files = Vec::new();
    while let Some(entry) = entries.next_entry()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? 
    {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "json" {
                    files.push(path);
                }
            }
        }
    }
    Ok(files)
}

#[derive(Serialize)]
struct IndexResponse {
    chunks_indexed: usize,
    entities_indexed: usize,
    relations_indexed: usize,
}

async fn index_data(
    State(state): State<Arc<AppState>>,
) -> Result<Json<IndexResponse>, StatusCode> {
    let chunks_dir = PathBuf::from("data/chunks");
    let extracted_dir = PathBuf::from("data/extracted");

    // Read all extracted files
    let mut entries = tokio::fs::read_dir(&extracted_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut chunks_indexed = 0;
    let mut total_entities = 0;
    let mut total_relations = 0;

    while let Some(entry) = entries.next_entry()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        // Read extracted data
        let extracted_json = tokio::fs::read_to_string(&path)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        let extracted: extract::ExtractedChunk = serde_json::from_str(&extracted_json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Read corresponding chunk
        let chunk_file = chunks_dir.join(format!("{}.json", extracted.chunk_id));
        let chunk_json = tokio::fs::read_to_string(&chunk_file)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        let chunk: ingest::Chunk = serde_json::from_str(&chunk_json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Index both
        state.indexer
            .index_extracted_chunk(&chunk, &extracted)
            .await
            .map_err(|e| {
                eprintln!("Indexing error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        chunks_indexed += 1;
        total_entities += extracted.extraction.entities.len();
        total_relations += extracted.extraction.relations.len();
    }

    Ok(Json(IndexResponse {
        chunks_indexed,
        entities_indexed: total_entities,
        relations_indexed: total_relations,
    }))
}

async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<index::IndexStats>, StatusCode> {
    let stats = state.indexer
        .get_stats()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(stats))
}

#[derive(Serialize)]
struct CommunitiesResponse {
    communities_detected: usize,
    summaries: Vec<communities::CommunitySummary>,
}

async fn detect_communities(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CommunitiesResponse>, StatusCode> {
    let summaries = state.community_detector
        .detect_and_summarize()
        .await
        .map_err(|e| {
            eprintln!("Community detection error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Save summaries to disk
    let output_dir = PathBuf::from("data/communities");
    tokio::fs::create_dir_all(&output_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for summary in &summaries {
        let file_path = output_dir.join(format!("community_{}.json", summary.community_id));
        let json = serde_json::to_string_pretty(summary)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        tokio::fs::write(file_path, json)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(CommunitiesResponse {
        communities_detected: summaries.len(),
        summaries,
    }))
}

#[derive(Deserialize)]
struct QueryRequest {
    query: String,
    #[serde(default = "default_top_k")]
    top_k: usize,
}

fn default_top_k() -> usize {
    5
}

async fn query_local(
    State(state): State<Arc<AppState>>,
    axum::Extension(request_id): axum::Extension<RequestId>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<query::LocalSearchResult>, StatusCode> {
    tracing::info!(
        request_id = %request_id.0,
        query = %req.query,
        "Local search request"
    );

    let timer = TimedOperation::start();
    
    let result = state.local_search
        .search(&req.query, req.top_k)
        .await
        .map_err(|e| {
            tracing::error!(
                request_id = %request_id.0,
                error = %e,
                "Local search failed"
            );
            state.metrics.record_request(false);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    state.metrics.record_query(timer.elapsed());
    state.metrics.record_request(true);

    tracing::info!(
        request_id = %request_id.0,
        duration_ms = timer.elapsed().as_millis(),
        chunks_retrieved = result.trace.chunks_retrieved,
        "Local search completed"
    );

    Ok(Json(result))
}

async fn query_global(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<query::GlobalSearchResult>, StatusCode> {
    let result = state.global_search
        .search(&req.query, req.top_k)
        .await
        .map_err(|e| {
            eprintln!("Global search error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(result))
}