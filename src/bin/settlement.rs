use dcap::{
    error::Result,
    model::PaymentMethod,
    settlement::{PaymentRequest, PaymentResult, SettlementConfig, SettlementService},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use clap::Parser;
use std::collections::HashMap;
use tokio::net::TcpListener;

#[derive(Parser)]
#[command(name = "settlement")]
#[command(about = "Settlement service for payment processing")]
struct Args {
    #[arg(short, long, default_value = "8002")]
    port: u16,

    #[arg(long, env = "STRIPE_SECRET_KEY")]
    stripe_secret_key: Option<String>,

    #[arg(long, env = "SOLANA_RPC_URL")]
    solana_rpc_url: Option<String>,

    #[arg(long, env = "ESCROW_SERVICE_URL")]
    escrow_service_url: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = Args::parse();

    let config = SettlementConfig {
        stripe_secret_key: args.stripe_secret_key,
        solana_rpc_url: args.solana_rpc_url,
        escrow_service_url: args.escrow_service_url,
    };

    let settlement_service = SettlementService::new(config).await?;
    let app_state = AppState { settlement_service };

    let app = Router::new()
        .route("/payment", post(create_payment))
        .route("/payment/:payment_id/status", get(get_payment_status))
        .route("/payment/:payment_id/refund", post(refund_payment))
        .route("/escrow/:escrow_id/release", post(release_escrow))
        .route("/webhook/stripe", post(handle_stripe_webhook))
        .route("/health", get(health_check))
        .with_state(app_state);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", args.port)).await?;
    println!("Settlement service listening on {}", args.port);

    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Clone)]
struct AppState {
    settlement_service: SettlementService,
}

async fn create_payment(
    State(state): State<AppState>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<PaymentResult>, StatusCode> {
    let payment_request = serde_json::from_value::<PaymentRequest>(request.clone())
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    match state.settlement_service.process_payment(payment_request).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            tracing::error!("Failed to create payment: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

async fn get_payment_status(
    State(state): State<AppState>,
    Path(payment_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.settlement_service.get_payment_status(&payment_id).await {
        Ok(status) => Ok(Json(serde_json::json!({
            "payment_id": payment_id,
            "status": status
        }))),
        Err(e) => {
            tracing::error!("Failed to get payment status: {}", e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

async fn refund_payment(
    State(state): State<AppState>,
    Path(payment_id): Path<String>,
) -> Result<Json<PaymentResult>, StatusCode> {
    match state.settlement_service.refund_payment(&payment_id).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            tracing::error!("Failed to refund payment: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

async fn release_escrow(
    State(state): State<AppState>,
    Path(escrow_id): Path<uuid::Uuid>,
) -> Result<Json<PaymentResult>, StatusCode> {
    match state.settlement_service.release_escrow(escrow_id).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            tracing::error!("Failed to release escrow: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

async fn handle_stripe_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let signature = headers
        .get("stripe-signature")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    match state.settlement_service.handle_webhook(&body, signature).await {
        Ok(_) => Ok(Json(serde_json::json!({"status": "received"}))),
        Err(e) => {
            tracing::error!("Failed to handle webhook: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "healthy"}))
}