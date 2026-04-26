//! Settlement port: convert accepted negotiations into payment intents.
//!
//! Settlement is inherently async and external. The kernel only knows that a
//! `Settled` event occurred; this port is responsible for making that event happen.

use async_trait::async_trait;
use crate::kernel::{Identity, NegotiationId, PaymentMethod};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Errors that can occur during settlement.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettlementError {
    #[error("insufficient funds")]
    InsufficientFunds,
    #[error("payment method unavailable: {0}")]
    MethodUnavailable(String),
    #[error("rejected by processor: {0}")]
    Rejected(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("timeout")]
    Timeout,
    #[error("unavailable")]
    Unavailable,
}

/// A payment intent created by a settlement adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentIntent {
    pub id: String,
    pub negotiation_id: NegotiationId,
    pub buyer: Identity,
    pub seller: Identity,
    pub amount: Decimal,
    pub currency: String,
    pub method: PaymentMethod,
    pub status: IntentStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentStatus {
    Pending,
    Authorized,
    Captured,
    Failed,
    Cancelled,
    Refunded,
}

/// The result of attempting to settle a negotiation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementResult {
    pub intent: PaymentIntent,
    pub processor_metadata: Option<String>,
}

/// The settlement port.
#[async_trait]
pub trait Settlement: Send + Sync {
    /// Create a payment intent for an accepted negotiation.
    ///
    /// This does not necessarily capture funds; it may only authorize.
    /// The adapter is responsible for polling or webhooks to detect final status.
    async fn create_intent(
        &self,
        negotiation_id: NegotiationId,
        buyer: &Identity,
        seller: &Identity,
        amount: Decimal,
        currency: &str,
        method: &PaymentMethod,
    ) -> Result<SettlementResult, SettlementError>;

    /// Check the current status of a payment intent.
    async fn check_status(&self, intent_id: &str) -> Result<PaymentIntent, SettlementError>;

    /// Cancel a pending payment intent.
    async fn cancel(&self, intent_id: &str) -> Result<PaymentIntent, SettlementError>;

    /// Release funds from escrow (if applicable).
    async fn release(&self, intent_id: &str) -> Result<PaymentIntent, SettlementError>;
}
