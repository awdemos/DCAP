//! Example demonstrating a complete negotiation flow
//!
//! This example shows how to:
//! 1. Set up all services
//! 2. Register a seller agent
//! 3. Create a buyer agent
//! 4. Browse products
//! 5. Request quotes
//! 6. Negotiate prices
//! 7. Complete transactions

use dcap::{
    agent::{BuyerAgent, SellerAgent, BuyerAgentConfig, SellerAgentConfig, LLMConfig},
    config::AppConfig,
    database::Database,
    discovery::DiscoveryService,
    error::Result,
    model::{Product, RFQ, Quote, AgentType, PaymentMethod},
    settlement::SettlementService,
    trust::TrustSystem,
};
use std::collections::HashMap;
use tempfile::NamedTempFile;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🤖 Negotiation Agents Demo");
    println!("==========================\n");

    // Setup services
    let (database, discovery, trust, settlement) = setup_services().await?;

    // Register seller
    println!("1. Registering seller agent...");
    let seller_agent = setup_seller_agent(&database, &discovery, &trust).await?;
    println!("   ✅ Seller agent registered\n");

    // Setup buyer
    println!("2. Setting up buyer agent...");
    let mut buyer_agent = setup_buyer_agent(&database, &discovery, &trust, &settlement).await?;
    println!("   ✅ Buyer agent ready\n");

    // Browse products
    println!("3. Browsing available products...");
    let products = buyer_agent.browse_products(Some("Electronics".to_string())).await?;
    println!("   Found {} products:", products.len());
    for product in &products {
        println!("   - {}: ${}", product.name, product.base_price);
    }
    println!();

    if products.is_empty() {
        println!("❌ No products found. Exiting.");
        return Ok(());
    }

    // Select first product for negotiation
    let selected_product = &products[0];
    println!("4. Negotiating for: {}", selected_product.name);

    // Request quote
    let negotiation_id = buyer_agent
        .request_quote(
            selected_product.id.clone(),
            1,
            selected_product.base_price * 1.2, // Willing to pay 20% more
        )
        .await?;

    println!("   📝 Quote requested (ID: {})", negotiation_id);

    // Wait for processing
    sleep(Duration::from_millis(500)).await;

    // Check active negotiations
    let active_negotiations = buyer_agent.get_active_negotiations();
    println!("   Active negotiations: {}", active_negotiations.len());

    for negotiation in active_negotiations {
        println!("   Status: {:?}", negotiation.status);
    }

    // Accept the quote
    println!("\n5. Accepting quote...");
    buyer_agent.accept_quote(negotiation_id).await?;
    println!("   ✅ Quote accepted! Transaction completed.");

    // Check final reputation scores
    println!("\n6. Final reputation scores:");
    let buyer_rep = trust.get_reputation(buyer_agent.config.agent_id).await?;
    let seller_rep = trust.get_reputation(seller_agent.config.agent_id).await?;
    println!("   Buyer reputation: {}", buyer_rep);
    println!("   Seller reputation: {}", seller_rep);

    println!("\n🎉 Demo completed successfully!");
    Ok(())
}

async fn setup_services() -> Result<(Database, DiscoveryService, TrustSystem, SettlementService)> {
    // Create temporary database
    let temp_file = NamedTempFile::new().unwrap();
    let db_url = format!("sqlite://{}", temp_file.path().to_string_lossy());
    let database = Database::new(&db_url).await?;

    // Create services
    let discovery = DiscoveryService::new("http://localhost:8000".to_string());
    let trust = TrustSystem::new(database.clone()).await?;
    let settlement = SettlementService::new(dcap::settlement::SettlementConfig {
        stripe_secret_key: None,
        solana_rpc_url: None,
        escrow_service_url: None,
    }).await?;

    Ok((database, discovery, trust, settlement))
}

async fn setup_seller_agent(
    database: &Database,
    discovery: &DiscoveryService,
    trust: &TrustSystem,
) -> Result<SellerAgent> {
    let seller_config = SellerAgentConfig {
        agent_id: uuid::Uuid::new_v4(),
        name: "TechStore Pro".to_string(),
        endpoint: "http://localhost:8001".to_string(),
        products: vec![
            Product {
                id: "laptop-gaming-001".to_string(),
                name: "Gaming Laptop Pro".to_string(),
                description: "High-performance gaming laptop with RTX 4080, 32GB RAM, 1TB SSD".to_string(),
                category: "Electronics".to_string(),
                base_price: 2499.99,
                currency: "USD".to_string(),
                stock_quantity: 5,
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("brand".to_string(), "TechBrand".to_string());
                    meta.insert("warranty".to_string(), "2 years".to_string());
                    meta
                },
            },
            Product {
                id: "smartphone-pro-001".to_string(),
                name: "Smartphone Pro Max".to_string(),
                description: "Latest flagship smartphone with 5G, 256GB storage".to_string(),
                category: "Electronics".to_string(),
                base_price: 1299.99,
                currency: "USD".to_string(),
                stock_quantity: 15,
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("brand".to_string(), "TechPhone".to_string());
                    meta.insert("colors".to_string(), "Black, White, Blue".to_string());
                    meta
                },
            },
        ],
        payment_methods: vec![PaymentMethod::Stripe, PaymentMethod::Escrow],
        llm_config: LLMConfig {
            model: "gpt-4".to_string(),
            api_key: "demo-key".to_string(),
            max_tokens: 1000,
            temperature: 0.7,
        },
    };

    let seller_agent = SellerAgent::new(seller_config, discovery.clone(), trust.clone(), database.clone()).await?;
    seller_agent.register().await?;

    // Give seller some initial reputation
    trust.update_reputation(seller_agent.config.agent_id, 80).await?;

    Ok(seller_agent)
}

async fn setup_buyer_agent(
    database: &Database,
    discovery: &DiscoveryService,
    trust: &TrustSystem,
    settlement: &SettlementService,
) -> Result<BuyerAgent> {
    let buyer_config = BuyerAgentConfig {
        agent_id: uuid::Uuid::new_v4(),
        name: "Corporate Buyer".to_string(),
        endpoint: "http://localhost:8002".to_string(),
        max_concurrent_negotiations: 10,
        default_ttl_hours: 48,
        llm_config: LLMConfig {
            model: "gpt-4".to_string(),
            api_key: "demo-key".to_string(),
            max_tokens: 1000,
            temperature: 0.7,
        },
    };

    let buyer_agent = BuyerAgent::new(
        buyer_config,
        discovery.clone(),
        trust.clone(),
        settlement.clone(),
        database.clone(),
    ).await?;

    // Give buyer some initial reputation
    trust.update_reputation(buyer_agent.config.agent_id, 75).await?;

    Ok(buyer_agent)
}