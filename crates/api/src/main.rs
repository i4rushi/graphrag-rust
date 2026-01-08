use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber;

#[derive(Clone)]
struct AppState {
    neo4j_graph: neo4rs::Graph,
    extractor: Arc<Mutex<extract::Extractor>>,
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

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

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

    let state = Arc::new(AppState {
        neo4j_graph,
        extractor: Arc::new(Mutex::new(extractor)),
    });

    // Build router
    let app = Router::new()
        .route("/health", post(health_check))
        .route("/health", get(health_check))
        .route("/ingest", post(ingest_document))
        .route("/extract", post(extract_chunks))
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    
    tracing::info!("Server listening on http://localhost:3000");
    
    axum::serve(listener, app).await.unwrap();
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