use dcap::{
    error::NegotiationError,
    model::{PaymentMethod, PaymentStatus},
    settlement::{PaymentResult, SettlementConfig, SettlementService},
};
use uuid::Uuid;

#[test]
fn test_settlement_config_creation() {
    let config = SettlementConfig {
        stripe_secret_key: Some("sk_test_key".to_string()),
        solana_rpc_url: Some("https://api.devnet.solana.com".to_string()),
        escrow_service_url: Some("https://escrow.example.com".to_string()),
    };

    assert!(config.stripe_secret_key.is_some());
    assert_eq!(config.stripe_secret_key.as_ref().unwrap(), "sk_test_key");
}

#[test]
fn test_settlement_config_none() {
    let config = SettlementConfig {
        stripe_secret_key: None,
        solana_rpc_url: None,
        escrow_service_url: None,
    };

    assert!(config.stripe_secret_key.is_none());
    assert!(config.solana_rpc_url.is_none());
    assert!(config.escrow_service_url.is_none());
}

#[test]
fn test_settlement_service_creation() {
    let config = SettlementConfig {
        stripe_secret_key: None,
        solana_rpc_url: None,
        escrow_service_url: None,
    };

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { SettlementService::new(config).await });

    assert!(result.is_ok());
}

#[test]
fn test_payment_result_creation() {
    let result = PaymentResult {
        payment_id: Uuid::new_v4(),
        success: true,
        amount: 100.0,
        currency: "USD".to_string(),
        transaction_hash: None,
    };

    assert_eq!(result.amount, 100.0);
    assert!(result.success);
    assert_eq!(result.currency, "USD");
}

#[test]
fn test_payment_result_transaction_hash() {
    let result = PaymentResult {
        payment_id: Uuid::new_v4(),
        success: true,
        amount: 100.0,
        currency: "USD".to_string(),
        transaction_hash: Some("0x123abc".to_string()),
    };

    assert!(result.transaction_hash.is_some());
    assert_eq!(result.transaction_hash.unwrap(), "0x123abc");
}

#[test]
fn test_payment_status_serialization() {
    let status = PaymentStatus::Pending;
    let serialized = serde_json::to_string(&status).unwrap();
    let deserialized: PaymentStatus = serde_json::from_str(&serialized).unwrap();

    assert_eq!(status, deserialized);
}

#[test]
fn test_payment_status_values() {
    assert_eq!(format!("{:?}", PaymentStatus::Pending), "Pending");
    assert_eq!(format!("{:?}", PaymentStatus::Processing), "Processing");
    assert_eq!(format!("{:?}", PaymentStatus::Completed), "Completed");
    assert_eq!(format!("{:?}", PaymentStatus::Failed), "Failed");
    assert_eq!(format!("{:?}", PaymentStatus::Refunded), "Refunded");
}

#[test]
fn test_stripe_payment_creation() {
    let result = dcap::settlement::StripePayment {
        payment_intent_id: "pi_test_id".to_string(),
        client_secret: "secret_test_key".to_string(),
        amount: 100.0,
        currency: "USD".to_string(),
        status: dcap::settlement::StripePaymentStatus::RequiresPaymentMethod,
    };

    assert_eq!(result.payment_intent_id, "pi_test_id");
    assert_eq!(result.amount, 100.0);
}

#[test]
fn test_solana_payment_creation() {
    let result = dcap::settlement::SolanaPayment {
        transaction_signature: "sig123abc".to_string(),
        from: Uuid::new_v4(),
        to: Uuid::new_v4(),
        lamports: 1000000,
    };

    assert_eq!(result.transaction_signature, "sig123abc");
    assert_eq!(result.lamports, 1000000);
}

#[test]
fn test_escrow_payment_creation() {
    let result = dcap::settlement::EscrowPayment {
        escrow_id: Uuid::new_v4(),
        escrow_amount: 100.0,
        currency: "USD".to_string(),
        escrow_fee: 5.0,
    };

    assert_eq!(result.escrow_amount, 100.0);
    assert_eq!(result.escrow_fee, 5.0);
}

#[test]
fn test_settlement_error_cases() {
    let error = NegotiationError::Payment("Payment failed".to_string());
    let error_str = format!("{}", error);
    assert!(error_str.contains("Payment failed"));

    let error = NegotiationError::Validation("Invalid amount".to_string());
    let error_str = format!("{}", error);
    assert!(error_str.contains("Invalid amount"));
}
