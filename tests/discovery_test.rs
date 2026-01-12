use dcap::{
    discovery::DiscoveryService,
    error::NegotiationError,
    model::{AgentInfo, AgentType, PaymentMethod},
};
use std::collections::HashMap;
use uuid::Uuid;

#[test]
fn test_discovery_service_creation() {
    let discovery = DiscoveryService::new("http://localhost:8000".to_string());
    assert_eq!(discovery.endpoint(), "http://localhost:8000");
}

#[test]
fn test_discovery_service_empty_endpoint() {
    let discovery = DiscoveryService::new("".to_string());
    assert_eq!(discovery.endpoint(), "");
}

#[test]
fn test_search_request_creation() {
    let request = dcap::discovery::SearchRequest {
        category: Some("Electronics".to_string()),
        min_reputation: Some(50),
        payment_methods: Some(vec![PaymentMethod::Stripe]),
    };

    assert!(request.category.is_some());
    assert_eq!(request.min_reputation, Some(50));
    assert_eq!(request.payment_methods.as_ref().unwrap().len(), 1);
}

#[test]
fn test_search_request_empty_filters() {
    let request = dcap::discovery::SearchRequest {
        category: None,
        min_reputation: None,
        payment_methods: None,
    };

    assert!(request.category.is_none());
    assert!(request.min_reputation.is_none());
    assert!(request.payment_methods.is_none());
}

#[test]
fn test_agent_info_creation() {
    let agent = AgentInfo {
        id: Uuid::new_v4(),
        agent_type: AgentType::Seller,
        name: "Test Agent".to_string(),
        endpoint: "http://localhost:8001".to_string(),
        public_key: "test-public-key".to_string(),
        reputation_score: 100,
        products: vec![],
        payment_methods: vec![PaymentMethod::Stripe],
        created_at: chrono::Utc::now(),
        last_active: chrono::Utc::now(),
    };

    assert_eq!(format!("{:?}", agent.agent_type), "Seller");
    assert_eq!(agent.reputation_score, 100);
}

#[test]
fn test_agent_info_default_reputation() {
    let agent = AgentInfo {
        id: Uuid::new_v4(),
        agent_type: AgentType::Seller,
        name: "Test Agent".to_string(),
        endpoint: "http://localhost:8001".to_string(),
        public_key: "test-public-key".to_string(),
        reputation_score: 0,
        products: vec![],
        payment_methods: vec![PaymentMethod::Stripe],
        created_at: chrono::Utc::now(),
        last_active: chrono::Utc::now(),
    };

    assert_eq!(agent.reputation_score, 0);
}

#[test]
fn test_register_request_creation() {
    let agent_info = AgentInfo {
        id: Uuid::new_v4(),
        agent_type: AgentType::Seller,
        name: "Test Agent".to_string(),
        endpoint: "http://localhost:8001".to_string(),
        public_key: "test-public-key".to_string(),
        reputation_score: 100,
        products: vec![],
        payment_methods: vec![PaymentMethod::Stripe],
        created_at: chrono::Utc::now(),
        last_active: chrono::Utc::now(),
    };

    let request = dcap::discovery::RegisterRequest {
        agent_type: agent_info.agent_type,
        name: agent_info.name.clone(),
        endpoint: agent_info.endpoint.clone(),
        public_key: agent_info.public_key.clone(),
        payment_methods: agent_info.payment_methods.clone(),
    };

    assert_eq!(request.name, "Test Agent");
    assert_eq!(request.endpoint, "http://localhost:8001");
}
