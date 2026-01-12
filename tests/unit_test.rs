use chrono::Utc;
use dcap::{
    error::NegotiationError,
    model::{NegotiationStatus, PaymentMethod, Product, RFQ},
};
use uuid::Uuid;

#[test]
fn test_product_creation() {
    let product = Product {
        id: "test-001".to_string(),
        name: "Test Product".to_string(),
        description: "A test product".to_string(),
        category: "Electronics".to_string(),
        base_price: 99.99,
        currency: "USD".to_string(),
        stock_quantity: 10,
        metadata: std::collections::HashMap::new(),
    };

    assert_eq!(product.id, "test-001");
    assert_eq!(product.base_price, 99.99);
}

#[test]
fn test_rfq_creation() {
    let rfq = RFQ::new(
        Uuid::new_v4(),
        "test-product".to_string(),
        5,
        100.0,
        "USD".to_string(),
        Utc::now(),
    );

    assert_eq!(rfq.quantity, 5);
    assert_eq!(rfq.max_price, 100.0);
    assert!(rfq.validate().is_ok());
}

#[test]
fn test_rfq_invalid_quantity() {
    let rfq = RFQ::new(
        Uuid::new_v4(),
        "test-product".to_string(),
        0,
        100.0,
        "USD".to_string(),
        Utc::now(),
    );

    assert!(rfq.validate().is_err());
}

#[test]
fn test_negotiation_status_transitions() {
    let buyer_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let rfq = RFQ::new(
        Uuid::new_v4(),
        "test-product".to_string(),
        1,
        100.0,
        "USD".to_string(),
        Utc::now(),
    );

    let mut negotiation = dcap::model::Negotiation::new(rfq, seller_id);
    assert_eq!(negotiation.status, NegotiationStatus::Pending);

    let quote = dcap::model::Quote::new(rfq.id, seller_id, 90.0, "USD".to_string(), 1, 3600);
    negotiation.add_quote(&quote).unwrap();
    assert_eq!(negotiation.status, NegotiationStatus::Quoted);

    negotiation.accept(quote.price).unwrap();
    assert_eq!(negotiation.status, NegotiationStatus::Accepted);
}

#[test]
fn test_payment_methods() {
    let methods = vec![
        PaymentMethod::Stripe,
        PaymentMethod::Solana,
        PaymentMethod::Escrow,
        PaymentMethod::PayOnDelivery,
    ];

    for method in methods {
        let serialized = serde_json::to_string(&method).unwrap();
        let deserialized: PaymentMethod = serde_json::from_str(&serialized).unwrap();
        assert_eq!(method, deserialized);
    }
}

#[test]
fn test_error_types() {
    let error = NegotiationError::ProductNotFound("test-product".to_string());

    assert!(matches!(error, NegotiationError::ProductNotFound(_)));

    let error_str = format!("{}", error);
    assert!(error_str.contains("test-product"));
}
