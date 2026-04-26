//! # DCAP Kernel
//!
//! The irreducible core of the DCAP protocol.
//!
//! This module contains only the types and logic that all participants must agree on.
//! It has no dependencies on external services, databases, or network transports.
//!
//! ## Design Principles
//!
//! 1. **Events are the API.** All state changes are represented as immutable, signed events.
//! 2. **Pure state machine.** `NegotiationState` is a deterministic fold over events.
//! 3. **Self-sovereign identity.** No centralized certificate authority; agents prove identity
//!    via Ed25519 signatures on every message.
//! 4. **Time-bounded commitments.** Every offer, quote, and RFQ carries an explicit TTL.
//! 5. **No floating-point money.** All monetary values use `Decimal`.

pub mod commitment;
pub mod event;
pub mod identity;
pub mod state_machine;
pub mod validation;

pub use commitment::{Commitment, CommitmentKind, Offer, RFQ, Quote};
pub use event::{Event, EventId, Payload};
pub use identity::{Identity, Signature};
pub use state_machine::{NegotiationState, Status, TransitionError};
pub use validation::{validate_event, ValidationError};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A globally unique negotiation identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NegotiationId(pub uuid::Uuid);

impl NegotiationId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for NegotiationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NegotiationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A timestamp with millisecond precision, used for TTL and ordering.
pub type Timestamp = DateTime<Utc>;

/// A nonce to prevent replay attacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nonce(pub [u8; 16]);

impl Nonce {
    pub fn random() -> Self {
        Self(rand::random())
    }
}

/// A product or service description used in commitments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub unit_price: Decimal,
    pub currency: String,
    pub stock_quantity: u64,
    pub metadata: BTreeMap<String, String>,
}

/// A quantity specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quantity {
    pub amount: u64,
    pub unit: String,
}

/// A payment method identifier (open string, not closed enum).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentMethod(pub String);

impl PaymentMethod {
    pub fn stripe() -> Self {
        Self("stripe".to_string())
    }
    pub fn solana_usdc() -> Self {
        Self("solana:usdc".to_string())
    }
    pub fn escrow_7day() -> Self {
        Self("escrow:7day".to_string())
    }
}

/// A delivery specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliverySpec {
    pub location: String,
    pub earliest: Option<Timestamp>,
    pub latest: Option<Timestamp>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiation_id_is_unique() {
        let a = NegotiationId::new();
        let b = NegotiationId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn payment_method_open_string() {
        let pm = PaymentMethod("custom:ach".to_string());
        assert_eq!(pm.0, "custom:ach");
    }
}
