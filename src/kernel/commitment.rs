//! Time-bounded commitments: RFQs, Quotes, Offers, and CounterOffers.
//!
//! A commitment is a signed, irrevocable statement of intent with an explicit TTL.
//! Once a commitment's TTL expires, it is no longer valid for state transitions.

use super::{DeliverySpec, Identity, PaymentMethod, Product, Quantity, Signature, Timestamp};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// The kind of commitment, used for routing and display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentKind {
    Rfq,
    Quote,
    CounterOffer,
    Acceptance,
    Rejection,
}

/// A request for quote sent by a buyer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RFQ {
    pub product: Product,
    pub quantity: Quantity,
    pub max_unit_price: Decimal,
    pub currency: String,
    pub delivery: Option<DeliverySpec>,
    pub payment_methods: Vec<PaymentMethod>,
    pub ttl_seconds: u32,
}

/// A quote sent by a seller in response to an RFQ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub rfq_hash: String, // hash of the RFQ this responds to
    pub unit_price: Decimal,
    pub currency: String,
    pub available_quantity: Quantity,
    pub delivery_estimate: Option<String>,
    pub ttl_seconds: u32,
}

/// A counter-offer or revised offer during negotiation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub unit_price: Decimal,
    pub currency: String,
    pub quantity: Quantity,
    pub ttl_seconds: u32,
}

/// A signed, time-bounded commitment envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commitment {
    pub kind: CommitmentKind,
    pub sender: Identity,
    pub payload: CommitmentPayload,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub signature: Signature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentPayload {
    Rfq(RFQ),
    Quote(Quote),
    CounterOffer(Offer),
    Acceptance { final_price: Decimal, currency: String },
    Rejection { reason: Option<String> },
}

impl Commitment {
    /// Check if the commitment has expired relative to `now`.
    pub fn is_expired(&self, now: Timestamp) -> bool {
        now > self.expires_at
    }

    /// Verify the signature over the canonical serialization of the payload.
    pub fn verify_signature(&self) -> Result<(), crate::kernel::identity::CryptoError> {
        let canonical = serde_json::to_vec(&self.payload)
            .map_err(|e| crate::kernel::identity::CryptoError::InvalidSignature(e.to_string()))?;
        self.sender.verify(&canonical, &self.signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn dummy_product() -> Product {
        Product {
            id: "laptop-001".into(),
            name: "Gaming Laptop".into(),
            description: "RTX 4080".into(),
            category: "Electronics".into(),
            unit_price: Decimal::new(249999, 2),
            currency: "USD".into(),
            stock_quantity: 10,
            metadata: Default::default(),
        }
    }

    #[test]
    fn commitment_expiration() {
        let kp = crate::kernel::identity::Keypair::generate();
        let payload = CommitmentPayload::Rfq(RFQ {
            product: dummy_product(),
            quantity: Quantity { amount: 1, unit: "each".into() },
            max_unit_price: Decimal::new(250000, 2),
            currency: "USD".into(),
            delivery: None,
            payment_methods: vec![PaymentMethod::stripe()],
            ttl_seconds: 3600,
        });
        let canonical = serde_json::to_vec(&payload).unwrap();
        let commitment = Commitment {
            kind: CommitmentKind::Rfq,
            sender: kp.identity(),
            payload,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(3600),
            signature: kp.sign(&canonical),
        };

        assert!(!commitment.is_expired(Utc::now()));
        assert!(commitment.is_expired(Utc::now() + chrono::Duration::seconds(7200)));
        assert!(commitment.verify_signature().is_ok());
    }
}
