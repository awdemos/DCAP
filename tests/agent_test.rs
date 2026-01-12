use chrono::{Duration, Utc};
use dcap::{
    agent::{BuyerAgent, BuyerAgentConfig, LLMConfig},
    config::AppConfig,
    discovery::DiscoveryService,
    error::NegotiationError,
    model::{AgentInfo, AgentType, Product, Quote, RFQ},
    settlement::SettlementService,
    trust::TrustSystem,
};
use std::collections::HashMap;
use uuid::Uuid;

fn create_test_buyer_config() -> BuyerAgentConfig {
    BuyerAgentConfig {
        agent_id: Uuid::new_v4(),
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
    }
}

fn create_test_discovery() -> DiscoveryService {
    DiscoveryService::new("".to_string())
}

fn create_test_trust() -> TrustSystem {
    TrustSystem::new().unwrap()
}

fn create_test_settlement() -> SettlementService {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        SettlementService::new(dcap::settlement::SettlementConfig {
            stripe_secret_key: None,
            solana_rpc_url: None,
            escrow_service_url: None,
        })
        .await
        .unwrap()
    })
}

fn create_mock_seller_agent_info() -> AgentInfo {
    AgentInfo {
        id: Uuid::new_v4(),
        agent_type: AgentType::Seller,
        name: "Mock Seller".to_string(),
        endpoint: "http://localhost:8001".to_string(),
        public_key: "mock-public-key".to_string(),
        reputation_score: 100,
        products: vec![Product {
            id: "test-product".to_string(),
            name: "Test Product".to_string(),
            description: "A test product for negotiation".to_string(),
            category: "Electronics".to_string(),
            base_price: 100.0,
            currency: "USD".to_string(),
            stock_quantity: 10,
            metadata: HashMap::new(),
        }],
        payment_methods: vec![],
        created_at: Utc::now(),
        last_active: Utc::now(),
    }
}

#[test]
fn test_buyer_agent_creation() {
    let config = create_test_buyer_config();
    let discovery = create_test_discovery();
    let trust = create_test_trust();
    let settlement = create_test_settlement();

    let buyer = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { BuyerAgent::new(config, discovery, trust, settlement).await });

    assert!(buyer.is_ok());
}

#[test]
fn test_buyer_browse_products_empty_discovery() {
    let config = create_test_buyer_config();
    let discovery = create_test_discovery();
    let trust = create_test_trust();
    let settlement = create_test_settlement();

    let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
        let buyer = BuyerAgent::new(config, discovery, trust, settlement)
            .await
            .unwrap();
        buyer.browse_products(None).await
    });

    assert!(result.is_ok());
    let products = result.unwrap();
    assert!(products.is_empty());
}

#[test]
fn test_rfq_validation_deadline_in_future() {
    let buyer_id = Uuid::new_v4();
    let rfq = RFQ::new(
        buyer_id,
        "test-product".to_string(),
        5,
        100.0,
        "USD".to_string(),
        Utc::now() + Duration::hours(24),
    );

    assert!(rfq.validate().is_ok());
    assert!(rfq.deadline > Utc::now());
}

#[test]
fn test_rfq_validation_zero_quantity_invalid() {
    let buyer_id = Uuid::new_v4();
    let rfq = RFQ::new(
        buyer_id,
        "test-product".to_string(),
        0,
        100.0,
        "USD".to_string(),
        Utc::now() + Duration::hours(24),
    );

    assert!(rfq.validate().is_err());
}

#[test]
fn test_rfq_validation_max_price_positive() {
    let buyer_id = Uuid::new_v4();
    let rfq = RFQ::new(
        buyer_id,
        "test-product".to_string(),
        5,
        -100.0,
        "USD".to_string(),
        Utc::now() + Duration::hours(24),
    );

    assert!(rfq.validate().is_err());
}

#[test]
fn test_buyer_max_concurrent_limit() {
    let config = create_test_buyer_config();
    assert_eq!(config.max_concurrent_negotiations, 5);
}

#[test]
fn test_buyer_default_ttl_hours() {
    let config = create_test_buyer_config();
    assert_eq!(config.default_ttl_hours, 24);
}

#[test]
fn test_buyer_endpoint_format() {
    let config = create_test_buyer_config();
    assert!(config.endpoint.starts_with("http://") || config.endpoint.starts_with("https://"));
}
