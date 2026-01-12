use chrono::Utc;
use dcap::{
    agent::{LLMConfig, SellerAgent, SellerAgentConfig},
    error::NegotiationError,
    model::{AgentInfo, AgentType, PaymentMethod, Product, Quote},
};
use std::collections::HashMap;
use uuid::Uuid;

fn create_test_seller_config() -> SellerAgentConfig {
    SellerAgentConfig {
        agent_id: Uuid::new_v4(),
        name: "Test Seller".to_string(),
        endpoint: "http://localhost:8001".to_string(),
        products: vec![Product {
            id: "test-product".to_string(),
            name: "Test Product".to_string(),
            description: "A test product".to_string(),
            category: "Electronics".to_string(),
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
    }
}

#[test]
fn test_seller_config_creation() {
    let config = create_test_seller_config();
    assert_eq!(config.products.len(), 1);
    assert_eq!(config.products[0].base_price, 100.0);
    assert_eq!(config.payment_methods.len(), 1);
}

#[test]
fn test_seller_config_validation() {
    let config = create_test_seller_config();

    let rfq = dcap::model::RFQ::new(
        Uuid::new_v4(),
        "test-product".to_string(),
        5,
        100.0,
        "USD".to_string(),
        Utc::now() + chrono::Duration::hours(24),
    );

    assert!(rfq.validate().is_ok());
}

#[test]
fn test_product_stock_validation_sufficient() {
    let config = create_test_seller_config();
    let product = &config.products[0];
    let rfq = dcap::model::RFQ::new(
        Uuid::new_v4(),
        "test-product".to_string(),
        product.stock_quantity,
        100.0,
        "USD".to_string(),
        Utc::now() + chrono::Duration::hours(24),
    );

    assert!(rfq.validate().is_ok());
}

#[test]
fn test_product_stock_validation_insufficient() {
    let config = create_test_seller_config();
    let product = &config.products[0];

    let rfq = dcap::model::RFQ::new(
        Uuid::new_v4(),
        "test-product".to_string(),
        product.stock_quantity + 1,
        100.0,
        "USD".to_string(),
        Utc::now() + chrono::Duration::hours(24),
    );

    assert!(rfq.validate().is_err());
}

#[test]
fn test_seller_agent_creation() {
    let config = create_test_seller_config();
    let discovery = dcap::DiscoveryService::new("".to_string());
    let trust = dcap::TrustSystem::new().unwrap();

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { SellerAgent::new(config, discovery, trust).await });

    assert!(result.is_ok());
}

#[test]
fn test_seller_products_listing() {
    let config = create_test_seller_config();
    let discovery = dcap::DiscoveryService::new("".to_string());
    let trust = dcap::TrustSystem::new().unwrap();

    let seller = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { SellerAgent::new(config, discovery, trust).await.unwrap() });

    let products = seller.get_products();
    assert_eq!(products.len(), 1);
    assert_eq!(products[0].id, "test-product");
}
