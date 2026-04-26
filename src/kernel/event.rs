//! Domain events: the protocol grammar.
//!
//! Events are immutable facts. They are the only way to change `NegotiationState`.
//! Every event carries a sender identity and a signature proving provenance.

use super::{Commitment, Identity, NegotiationId, Nonce, Signature, Timestamp};
use serde::{Deserialize, Serialize};

/// A unique event identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventId(pub uuid::Uuid);

impl EventId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The top-level event envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub negotiation_id: NegotiationId,
    pub payload: Payload,
    pub sender: Identity,
    pub timestamp: Timestamp,
    pub nonce: Nonce,
    pub signature: Signature,
}

impl Event {
    /// Verify the signature over the canonical serialization of the payload.
    pub fn verify_signature(&self) -> Result<(), crate::kernel::identity::CryptoError> {
        let canonical = serde_json::to_vec(&self.payload)
            .map_err(|e| crate::kernel::identity::CryptoError::InvalidSignature(e.to_string()))?;
        self.sender.verify(&canonical, &self.signature)
    }
}

/// The payload of a DCAP event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Payload {
    /// A buyer submitted an RFQ.
    RfqSubmitted {
        commitment: Commitment,
        recipients: Vec<Identity>,
    },
    /// A seller responded with a quote.
    Quoted {
        commitment: Commitment,
    },
    /// A party countered with a new offer.
    Countered {
        commitment: Commitment,
    },
    /// A party accepted the current offer.
    Accepted {
        commitment: Commitment,
    },
    /// A party rejected the negotiation.
    Rejected {
        commitment: Commitment,
    },
    /// The negotiation was settled (payment intent created / escrow locked).
    Settled {
        settlement_id: String,
        payment_method: String,
    },
    /// The negotiation expired (TTL reached with no resolution).
    Expired {
        reason: Option<String>,
    },
    /// A dispute was raised.
    Disputed {
        reason: String,
        evidence_hash: Option<String>,
    },
    /// A human approval was requested (for high-value or anomalous negotiations).
    HumanApprovalRequested {
        context: String,
    },
    /// A human approved the negotiation.
    HumanApproved,
    /// A human rejected the negotiation.
    HumanRejected {
        reason: Option<String>,
    },
}

impl Payload {
    /// A short string identifier for the payload kind.
    pub fn kind(&self) -> &'static str {
        match self {
            Payload::RfqSubmitted { .. } => "rfq_submitted",
            Payload::Quoted { .. } => "quoted",
            Payload::Countered { .. } => "countered",
            Payload::Accepted { .. } => "accepted",
            Payload::Rejected { .. } => "rejected",
            Payload::Settled { .. } => "settled",
            Payload::Expired { .. } => "expired",
            Payload::Disputed { .. } => "disputed",
            Payload::HumanApprovalRequested { .. } => "human_approval_requested",
            Payload::HumanApproved => "human_approved",
            Payload::HumanRejected { .. } => "human_rejected",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_id_unique() {
        assert_ne!(EventId::new(), EventId::new());
    }

    #[test]
    fn payload_kind_matches() {
        let p = Payload::Expired { reason: None };
        assert_eq!(p.kind(), "expired");
    }
}
