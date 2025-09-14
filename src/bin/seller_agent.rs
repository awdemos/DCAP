use dcap::{
    agent::{SellerAgent, SellerAgentConfig, LLMConfig},
    config::AppConfig,
    discovery::DiscoveryService,
    error::NegotiationError,
    model::{Product, RFQ, Quote, PaymentMethod},
    settlement::SettlementService,
    trust::TrustSystem,
};
use chrono;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use clap::Parser;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[derive(Parser)]
#[command(name = "seller-agent")]
#[command(about = "LLM-powered seller agent for marketplace negotiations")]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    #[arg(short, long, default_value = "sqlite://negotiation.db")]
    database_url: String,

    #[arg(short, long, default_value = "http://localhost:8000")]
    discovery_endpoint: String,

    #[arg(short, long, default_value = "8001")]
    port: u16,
}

#[derive(Clone)]
struct AppState {
    seller_agent_config: SellerAgentConfig,
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = Args::parse();

    let config = AppConfig::load(&args.config)?;
    let discovery = DiscoveryService::new(args.discovery_endpoint.clone());
    let trust = TrustSystem::new()?;
    let settlement_config = dcap::settlement::SettlementConfig {
        stripe_secret_key: None,
        solana_rpc_url: None,
        escrow_service_url: None,
    };
    let settlement = SettlementService::new(settlement_config).await?;

    let products = vec![
        Product {
            id: "laptop-001".to_string(),
            name: "Gaming Laptop".to_string(),
            description: "High-performance gaming laptop with RTX 4080".to_string(),
            category: "Electronics".to_string(),
            base_price: 2499.99,
            currency: "USD".to_string(),
            stock_quantity: 10,
            metadata: HashMap::new(),
        },
        Product {
            id: "phone-001".to_string(),
            name: "Smartphone Pro".to_string(),
            description: "Latest flagship smartphone with 5G".to_string(),
            category: "Electronics".to_string(),
            base_price: 1299.99,
            currency: "USD".to_string(),
            stock_quantity: 25,
            metadata: HashMap::new(),
        },
    ];

    let seller_config = SellerAgentConfig {
        agent_id: uuid::Uuid::new_v4(),
        name: "TechSeller".to_string(),
        endpoint: format!("http://localhost:{}", args.port),
        products,
        payment_methods: vec![PaymentMethod::Stripe, PaymentMethod::Escrow],
        llm_config: LLMConfig {
            model: "gpt-4".to_string(),
            api_key: env::var("OPENAI_API_KEY").unwrap_or_else(|_| "mock_key".to_string()),
            max_tokens: 1000,
            temperature: 0.7,
        },
    };

    let seller_agent = SellerAgent::new(
        seller_config.clone(),
        discovery,
        trust,
    ).await?;

    // Register with discovery service
    seller_agent.register().await?;

    let app_state = AppState {
        seller_agent_config: seller_config.clone(),
    };

    let app = Router::new()
        .route("/quote", post(handle_quote))
        .route("/quote/:rfq_id", get(get_quote))
        .route("/negotiate/:negotiation_id", post(handle_negotiation))
        .route("/products", get(list_products))
        .route("/health", get(health_check))
        .with_state(app_state);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", args.port)).await?;
    println!("Seller agent listening on {}", args.port);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_quote(
    State(_state): State<AppState>,
    Json(rfq): Json<RFQ>,
) -> Json<serde_json::Value> {
    // Mock quote response
    Json(serde_json::json!({
        "id": uuid::Uuid::new_v4(),
        "rfq_id": rfq.id,
        "seller_id": uuid::Uuid::new_v4(),
        "price": rfq.max_price * 0.9,
        "currency": rfq.currency,
        "available_quantity": rfq.quantity,
        "ttl_seconds": 3600,
        "created_at": chrono::Utc::now(),
        "metadata": {}
    }))
}

async fn get_quote(
    State(state): State<AppState>,
    Path(rfq_id): Path<uuid::Uuid>,
) -> Json<serde_json::Value> {
    // This would typically fetch the quote from the database
    // For now, we'll return a mock response
    Json(serde_json::json!({
        "status": "error",
        "message": "Quote not found"
    }))
}

async fn handle_negotiation(
    State(_state): State<AppState>,
    Path(_negotiation_id): Path<uuid::Uuid>,
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let counter_offer = payload.get("counter_offer")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    // Mock negotiation response
    Json(serde_json::json!({
        "id": uuid::Uuid::new_v4(),
        "rfq_id": uuid::Uuid::new_v4(),
        "seller_id": uuid::Uuid::new_v4(),
        "price": counter_offer * 0.95,
        "currency": "USD",
        "available_quantity": 1,
        "ttl_seconds": 1800,
        "created_at": chrono::Utc::now(),
        "metadata": {}
    }))
}

async fn list_products(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    // In a real implementation, this would query the database
    // For now, return mock products
    Json(serde_json::json!([
        {
            "id": "laptop-001",
            "name": "Gaming Laptop",
            "description": "High-performance gaming laptop with RTX 4080",
            "category": "Electronics",
            "base_price": 2499.99,
            "currency": "USD",
            "stock_quantity": 10
        },
        {
            "id": "phone-001",
            "name": "Smartphone Pro",
            "description": "Latest flagship smartphone with 5G",
            "category": "Electronics",
            "base_price": 1299.99,
            "currency": "USD",
            "stock_quantity": 25
        }
    ]))
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "healthy"}))
}