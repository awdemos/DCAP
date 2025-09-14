use negotiation_agents::{
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

async fn setup_test_services() -> Result<(Database, DiscoveryService, TrustSystem, SettlementService)> {
    // Create temporary database
    let temp_file = NamedTempFile::new().unwrap();
    let db_url = format!("sqlite://{}", temp_file.path().to_string_lossy());
    let database = Database::new(&db_url).await?;

    // Create services
    let discovery = DiscoveryService::new("http://localhost:8000".to_string());
    let trust = TrustSystem::new(database.clone()).await?;
    let settlement = SettlementService::new(negotiation_agents::settlement::SettlementConfig {
        stripe_secret_key: None,
        solana_rpc_url: None,
        escrow_service_url: None,
    }).await?;

    Ok((database, discovery, trust, settlement))
}

#[tokio::test]
async fn test_agent_registration() -> Result<()> {
    let (database, discovery, trust, settlement) = setup_test_services().await?;

    // Create seller agent
    let seller_config = SellerAgentConfig {
        agent_id: uuid::Uuid::new_v4(),
        name: "Test Seller".to_string(),
        endpoint: "http://localhost:8001".to_string(),
        products: vec![Product {
            id: "test-product".to_string(),
            name: "Test Product".to_string(),
            description: "A test product".to_string(),
            category: "Test".to_string(),
            base_price: 100.0,
            currency: "USD".to_string(),
            stock_quantity: 10,
            metadata: HashMap::new(),
        }],
        payment_methods: vec![PaymentMethod::Stripe],
        llm_config: LLMConfig {
            model: "test-model".to_string(),
            api_key: "test-key".to_string(),
            max_tokens: 100,
            temperature: 0.7,
        },
    };

    let seller_agent = SellerAgent::new(seller_config, discovery.clone(), trust, database.clone()).await?;
    seller_agent.register().await?;

    // Verify agent exists in database
    let agents = database.get_agents_by_type(AgentType::Seller).await?;
    assert!(!agents.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_negotiation_flow() -> Result<()> {
    let (database, discovery, trust, settlement) = setup_test_services().await?;

    // Setup seller
    let seller_config = SellerAgentConfig {
        agent_id: uuid::Uuid::new_v4(),
        name: "Test Seller".to_string(),
        endpoint: "http://localhost:8001".to_string(),
        products: vec![Product {
            id: "laptop-001".to_string(),
            name: "Test Laptop".to_string(),
            description: "A test laptop".to_string(),
            category: "Electronics".to_string(),
            base_price: 1000.0,
            currency: "USD".to_string(),
            stock_quantity: 5,
            metadata: HashMap::new(),
        }],
        payment_methods: vec![PaymentMethod::Stripe],
        llm_config: LLMConfig {
            model: "test-model".to_string(),
            api_key: "test-key".to_string(),
            max_tokens: 100,
            temperature: 0.7,
        },
    };

    let seller_agent = SellerAgent::new(seller_config, discovery.clone(), trust.clone(), database.clone()).await?;
    seller_agent.register().await?;

    // Setup buyer
    let buyer_config = BuyerAgentConfig {
        agent_id: uuid::Uuid::new_v4(),
        name: "Test Buyer".to_string(),
        endpoint: "http://localhost:8002".to_string(),
        max_concurrent_negotiations: 5,
        default_ttl_hours: 24,
        llm_config: LLMConfig {
            model: "test-model".to_string(),
            api_key: "test-key".to_string(),
            max_tokens: 100,
            temperature: 0.7,
        },
    };

    let mut buyer_agent = BuyerAgent::new(
        buyer_config,
        discovery.clone(),
        trust.clone(),
        settlement.clone(),
        database.clone(),
    ).await?;

    // Test RFQ creation
    let rfq = RFQ::new(
        buyer_agent.config.agent_id,
        "laptop-001".to_string(),
        1,
        1200.0,
        "USD".to_string(),
        chrono::Utc::now() + chrono::Duration::hours(24),
    );

    rfq.validate()?;
    assert_eq!(rfq.quantity, 1);
    assert_eq!(rfq.max_price, 1200.0);

    Ok(())
}

#[tokio::test]
async fn test_trust_system() -> Result<()> {
    let (database, _, trust, _) = setup_test_services().await?;

    let agent_id = uuid::Uuid::new_v4();

    // Test initial reputation
    let initial_score = trust.get_reputation(agent_id).await?;
    assert_eq!(initial_score, 0);

    // Test reputation update
    trust.update_reputation(agent_id, 10).await?;
    let updated_score = trust.get_reputation(agent_id).await?;
    assert_eq!(updated_score, 10);

    // Test trust level calculation
    let trust_level = trust.get_trust_level(agent_id).await?;
    assert_eq!(format!("{:?}", trust_level), "Neutral");

    // Test JWT generation
    let jwt = trust.generate_jwt(agent_id).await?;
    assert!(!jwt.is_empty());

    // Test JWT validation
    let claims = trust.validate_jwt(&jwt).await?;
    assert_eq!(claims.sub, agent_id.to_string());
    assert_eq!(claims.reputation_score, 10);

    Ok(())
}

#[tokio::test]
async fn test_settlement_service() -> Result<()> {
    let settlement = SettlementService::new(negotiation_agents::settlement::SettlementConfig {
        stripe_secret_key: None,
        solana_rpc_url: None,
        escrow_service_url: None,
    }).await?;

    let buyer_id = uuid::Uuid::new_v4();
    let seller_id = uuid::Uuid::new_v4();

    // Test escrow payment (mock)
    let result = settlement.create_payment(buyer_id, seller_id, 100.0, "USD".to_string()).await?;
    assert!(result.success);
    assert_eq!(result.amount, 100.0);

    // Test payment status
    let status = settlement.get_payment_status(&result.payment_id).await?;
    assert!(!format!("{:?}", status).is_empty());

    Ok(())
}

#[tokio::test]
async fn test_discovery_service() -> Result<()> {
    let (database, discovery, _, _) = setup_test_services().await?;

    // Test seller search
    let search_request = negotiation_agents::discovery::SearchRequest {
        category: Some("Electronics".to_string()),
        min_reputation: Some(50),
        payment_methods: Some(vec![PaymentMethod::Stripe]),
    };

    let sellers = discovery.search_sellers(search_request).await?;
    assert!(sellers.is_empty()); // No sellers registered yet

    // Test agent lookup
    let agent_id = uuid::Uuid::new_v4();
    let result = discovery.get_agent(agent_id).await;
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_database_operations() -> Result<()> {
    let temp_file = NamedTempFile::new().unwrap();
    let db_url = format!("sqlite://{}", temp_file.path().to_string_lossy());
    let database = Database::new(&db_url).await?;

    // Test agent creation
    let agent_id = uuid::Uuid::new_v4();
    let agent_info = negotiation_agents::model::AgentInfo {
        id: agent_id,
        agent_type: AgentType::Seller,
        name: "Test Agent".to_string(),
        endpoint: "http://localhost:8001".to_string(),
        public_key: "test-key".to_string(),
        reputation_score: 100,
        products: vec![],
        payment_methods: vec![],
        created_at: chrono::Utc::now(),
        last_active: chrono::Utc::now(),
    };

    database.create_agent(&agent_info).await?;

    // Test agent retrieval
    let retrieved_agent = database.get_agent(agent_id).await?;
    assert!(retrieved_agent.is_some());
    assert_eq!(retrieved_agent.unwrap().name, "Test Agent");

    // Test reputation update
    database.update_agent_reputation(agent_id, 5).await?;
    let reputation = database.get_agent_reputation(agent_id).await?;
    assert_eq!(reputation, 105);

    Ok(())
}

#[tokio::test]
async fn test_negotiation_model() -> Result<()> {
    let buyer_id = uuid::Uuid::new_v4();
    let seller_id = uuid::Uuid::new_v4();

    // Test RFQ validation
    let mut rfq = RFQ::new(
        buyer_id,
        "test-product".to_string(),
        0, // Invalid quantity
        100.0,
        "USD".to_string(),
        chrono::Utc::now() + chrono::Duration::hours(24),
    );

    assert!(rfq.validate().is_err());

    // Fix RFQ
    rfq.quantity = 1;
    assert!(rfq.validate().is_ok());

    // Test Quote validation
    let quote = Quote::new(
        rfq.id,
        seller_id,
        90.0,
        "USD".to_string(),
        1,
        3600,
    );

    assert!(quote.validate().is_ok());
    assert!(!quote.is_expired());

    // Test negotiation workflow
    let mut negotiation = negotiation_agents::model::Negotiation::new(rfq, seller_id);
    assert_eq!(negotiation.status, negotiation_agents::model::NegotiationStatus::Pending);

    negotiation.add_quote(&quote)?;
    assert_eq!(negotiation.status, negotiation_agents::model::NegotiationStatus::Quoted);

    negotiation.accept(quote.price)?;
    assert_eq!(negotiation.status, negotiation_agents::model::NegotiationStatus::Accepted);

    let record = negotiation.to_record();
    assert!(record.is_some());

    Ok(())
}

#[tokio::test]
async fn test_configuration_loading() -> Result<()> {
    let temp_file = NamedTempFile::new().unwrap();
    let config_path = temp_file.path();

    // Create test configuration
    let test_config = r#"
[server]
host = "127.0.0.1"
port = 8080

[database]
url = "sqlite://test.db"

[llm]
model = "gpt-4"
max_tokens = 2000
"#;

    std::fs::write(config_path, test_config)?;

    let config = AppConfig::load(config_path)?;
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.llm.max_tokens, Some(2000));

    // Test validation
    assert!(config.validate().is_ok());

    Ok(())
}