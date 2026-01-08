use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing_subscriber;

#[derive(Clone)]
struct AppState {
    qdrant_client: qdrant_client::client::QdrantClient,
    neo4j_graph: neo4rs::Graph,
}

#[derive(Serialize)]
struct HealthResponse {
    qdrant: String,
    neo4j: String,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Connect to Qdrant
    let qdrant_client = qdrant_client::client::QdrantClient::from_url("http://localhost:6333")
        .build()
        .expect("Failed to create Qdrant client");

    // Connect to Neo4j
    let neo4j_graph = neo4rs::Graph::new(
        "bolt://localhost:7687",
        "neo4j",
        "yourpassword", // Use the password from your docker-compose.yml
    )
    .await
    .expect("Failed to connect to Neo4j");

    let state = Arc::new(AppState {
        qdrant_client,
        neo4j_graph,
    });

    // Build router
    let app = Router::new()
        .route("/health", post(health_check))
        .route("/health", get(health_check)) // Also support GET
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
    // Check Qdrant
    let qdrant_status = match state.qdrant_client.health_check().await {
        Ok(_) => "ok".to_string(),
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