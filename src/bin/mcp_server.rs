use dcap::mcp::NegotiationMcpServer;
use tokio::net::TcpListener;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "negotiation_mcp=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting DCAP MCP Server");

    // Create MCP server
    let server = NegotiationMcpServer::new().await?;

    // Start TCP listener
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    info!("MCP server listening on {}", listener.local_addr()?);

    // Run server
    if let Err(e) = server.run(listener).await {
        error!("Server error: {}", e);
        return Err(e.into());
    }

    Ok(())
}