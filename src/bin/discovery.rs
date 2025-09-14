use dcap::{
    discovery::{DiscoveryServer, RegisterRequest, SearchRequest},
    error::NegotiationError,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use clap::Parser;
use tokio::net::TcpListener;

#[derive(Parser)]
#[command(name = "discovery")]
#[command(about = "Discovery service for agent registration and search")]
struct Args {
    #[arg(short, long, default_value = "sqlite://discovery.db")]
    database_url: String,

    #[arg(short, long, default_value = "8000")]
    port: u16,
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = Args::parse();

    let discovery_server = DiscoveryServer::new(&args.database_url).await?;
    let app_state = AppState { discovery_server };

    let app = Router::new()
        .route("/register", post(register_agent))
        .route("/search", post(search_agents))
        .route("/agents/:agent_id", get(get_agent))
        .route("/health", get(health_check))
        .with_state(app_state);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", args.port)).await?;
    println!("Discovery service listening on {}", args.port);

    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Clone)]
struct AppState {
    discovery_server: DiscoveryServer,
}

async fn register_agent(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Json<serde_json::Value> {
    match state.discovery_server.handle_register(request).await {
        Ok(agent) => Json(serde_json::json!({
            "status": "success",
            "agent_id": agent.id,
            "message": "Agent registered successfully"
        })),
        Err(e) => {
            tracing::error!("Failed to register agent: {}", e);
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string()
            }))
        }
    }
}

async fn search_agents(
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Json<serde_json::Value> {
    match state.discovery_server.handle_search(request).await {
        Ok(response) => Json(serde_json::json!(response)),
        Err(e) => {
            tracing::error!("Failed to search agents: {}", e);
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string()
            }))
        }
    }
}

async fn get_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<uuid::Uuid>,
) -> Json<serde_json::Value> {
    match state.discovery_server.get_agent_info(agent_id).await {
        Ok(Some(agent)) => Json(serde_json::json!(agent)),
        Ok(None) => Json(serde_json::json!({
            "status": "error",
            "message": "Agent not found"
        })),
        Err(e) => {
            tracing::error!("Failed to get agent: {}", e);
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string()
            }))
        }
    }
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "healthy"}))
}