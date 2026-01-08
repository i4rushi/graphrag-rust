use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

use serde::Serialize;
use std::sync::Arc;
use tracing_subscriber;

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct IngestRequest {
    path: String,
}

#[derive(Serialize)]
struct IngestResponse {
    chunks_created: usize,
    doc_ids: Vec<String>,
}

#[derive(Clone)]
struct AppState {
    neo4j_graph: neo4rs::Graph,
}

#[derive(Serialize)]
struct HealthResponse {
    qdrant: String,
    neo4j: String,
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

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Connect to Neo4j
    let neo4j_graph = neo4rs::Graph::new(
        "bolt://localhost:7687",
        "neo4j",
        "yourpassword", // Use the password from your docker-compose.yml
    )
    .await
    .expect("Failed to connect to Neo4j");

    let state = Arc::new(AppState {
        neo4j_graph,
    });

    // Build router
    let app = Router::new()
        .route("/health", post(health_check))
        .route("/health", get(health_check))
        .route("/ingest", post(ingest_document))
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