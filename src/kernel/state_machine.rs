//! Deterministic negotiation state machine.
//!
//! `NegotiationState` is a pure fold over `Event` payloads. Given the same stream of
//! events, every participant arrives at the same state. This is the foundation of
//! distributed consensus without a central coordinator.

use super::{commitment::CommitmentPayload, Commitment, Event, EventId, Identity, NegotiationId, Payload, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The status of a negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Quoted,
    Negotiating,
    Accepted,
    Rejected,
    Settled,
    Expired,
    Disputed,
    AwaitingHumanApproval,
}

/// The state of a single participant in a negotiation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantState {
    pub identity: Identity,
    pub role: ParticipantRole,
    pub last_activity: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantRole {
    Buyer,
    Seller,
    Observer,
}

/// The full state of a negotiation, derived from its event stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegotiationState {
    pub id: NegotiationId,
    pub created_at: Timestamp,
    pub expires_at: Option<Timestamp>,
    pub status: Status,
    pub buyer: Option<Identity>,
    pub sellers: BTreeMap<Identity, SellerState>,
    pub rfq: Option<Commitment>,
    pub current_offer: Option<Commitment>,
    pub accepted_offer: Option<Commitment>,
    pub settlement_id: Option<String>,
    pub payment_method: Option<String>,
    pub events: Vec<EventId>,
    pub dispute_reason: Option<String>,
}

/// Per-seller state within a multi-party RFQ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SellerState {
    pub identity: Identity,
    pub quote: Option<Commitment>,
    pub status: SellerNegotiationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SellerNegotiationStatus {
    Invited,
    Quoted,
    Countered,
    Accepted,
    Rejected,
}

/// Errors that can occur when applying an event.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransitionError {
    #[error("invalid transition: cannot apply {event} from state {state:?}")]
    Invalid { event: String, state: Status },
    #[error("signature verification failed for event {event_id:?}")]
    SignatureInvalid { event_id: EventId },
    #[error("event timestamp {event} is before negotiation creation {created}")]
    TimestampBeforeCreation { event: Timestamp, created: Timestamp },
    #[error("commitment expired: {expires_at}")]
    CommitmentExpired { expires_at: Timestamp },
    #[error("unauthorized sender: {sender}")]
    Unauthorized { sender: Identity },
    #[error("seller {seller} not invited to negotiation")]
    SellerNotInvited { seller: Identity },
    #[error("duplicate event: {event_id:?}")]
    DuplicateEvent { event_id: EventId },
    #[error("offer currency mismatch: expected {expected}, got {actual}")]
    CurrencyMismatch { expected: String, actual: String },
}

impl NegotiationState {
    /// Create the initial state from the first event (must be `RfqSubmitted`).
    pub fn init(event: &Event) -> Result<Self, TransitionError> {
        match &event.payload {
            Payload::RfqSubmitted { commitment, recipients } => {
                let mut sellers = BTreeMap::new();
                for recipient in recipients {
                    if *recipient != commitment.sender {
                        sellers.insert(
                            recipient.clone(),
                            SellerState {
                                identity: recipient.clone(),
                                quote: None,
                                status: SellerNegotiationStatus::Invited,
                            },
                        );
                    }
                }
                Ok(Self {
                    id: event.negotiation_id,
                    created_at: event.timestamp,
                    expires_at: Some(commitment.expires_at),
                    status: Status::Pending,
                    buyer: Some(commitment.sender.clone()),
                    sellers,
                    rfq: Some(commitment.clone()),
                    current_offer: None,
                    accepted_offer: None,
                    settlement_id: None,
                    payment_method: None,
                    events: vec![event.id],
                    dispute_reason: None,
                })
            }
            other => Err(TransitionError::Invalid {
                event: format!("expected RfqSubmitted, got {}", other.kind()),
                state: Status::Pending,
            }),
        }
    }

    /// Apply a single event to the state, producing a new state.
    pub fn apply(mut self, event: &Event) -> Result<Self, TransitionError> {
        // Idempotency check
        if self.events.contains(&event.id) {
            return Err(TransitionError::DuplicateEvent { event_id: event.id });
        }

        // Temporal validation
        if event.timestamp < self.created_at {
            return Err(TransitionError::TimestampBeforeCreation {
                event: event.timestamp,
                created: self.created_at,
            });
        }

        self.events.push(event.id);

        match (&self.status, &event.payload) {
            // --- Pending ---
            (Status::Pending, Payload::Quoted { commitment }) => {
                self.ensure_commitment_valid(commitment, event.timestamp)?;
                self.ensure_sender_is_seller(&commitment.sender)?;
        let sender = commitment.sender.clone();
                let seller = self.sellers.get_mut(&sender).ok_or_else(|| {
                    TransitionError::SellerNotInvited { seller: sender }
                })?;
                seller.quote = Some(commitment.clone());
                seller.status = SellerNegotiationStatus::Quoted;
                self.current_offer = Some(commitment.clone());
                self.status = Status::Quoted;
                Ok(self)
            }

            (Status::Pending, Payload::Expired { .. }) => {
                self.status = Status::Expired;
                Ok(self)
            }

            // --- Quoted ---
            (Status::Quoted, Payload::Countered { commitment }) => {
                self.ensure_commitment_valid(commitment, event.timestamp)?;
                self.ensure_sender_is_buyer_or_seller(&commitment.sender)?;
                self.update_current_offer(commitment)?;
                self.status = Status::Negotiating;
                Ok(self)
            }

            (Status::Quoted, Payload::Accepted { commitment }) => {
                self.ensure_commitment_valid(commitment, event.timestamp)?;
                self.ensure_sender_is_buyer(&commitment.sender)?;
                self.accepted_offer = self.current_offer.clone();
                self.status = Status::Accepted;
                Ok(self)
            }

            (Status::Quoted, Payload::Rejected { commitment }) => {
                self.ensure_commitment_valid(commitment, event.timestamp)?;
                self.ensure_sender_is_buyer(&commitment.sender)?;
                self.status = Status::Rejected;
                Ok(self)
            }

            (Status::Quoted, Payload::Expired { .. }) => {
                self.status = Status::Expired;
                Ok(self)
            }

            // --- Negotiating ---
            (Status::Negotiating, Payload::Countered { commitment }) => {
                self.ensure_commitment_valid(commitment, event.timestamp)?;
                self.ensure_sender_is_buyer_or_seller(&commitment.sender)?;
                self.update_current_offer(commitment)?;
                Ok(self)
            }

            (Status::Negotiating, Payload::Accepted { commitment }) => {
                self.ensure_commitment_valid(commitment, event.timestamp)?;
                self.ensure_sender_is_buyer(&commitment.sender)?;
                self.accepted_offer = self.current_offer.clone();
                self.status = Status::Accepted;
                Ok(self)
            }

            (Status::Negotiating, Payload::Rejected { commitment }) => {
                self.ensure_commitment_valid(commitment, event.timestamp)?;
                self.ensure_sender_is_buyer(&commitment.sender)?;
                self.status = Status::Rejected;
                Ok(self)
            }

            (Status::Negotiating, Payload::Expired { .. }) => {
                self.status = Status::Expired;
                Ok(self)
            }

            // --- Accepted ---
            (Status::Accepted, Payload::Settled { settlement_id, payment_method }) => {
                self.settlement_id = Some(settlement_id.clone());
                self.payment_method = Some(payment_method.clone());
                self.status = Status::Settled;
                Ok(self)
            }

            (Status::Accepted, Payload::Disputed { reason, .. }) => {
                self.dispute_reason = Some(reason.clone());
                self.status = Status::Disputed;
                Ok(self)
            }

            (Status::Accepted, Payload::HumanApprovalRequested { .. }) => {
                self.status = Status::AwaitingHumanApproval;
                Ok(self)
            }

            // --- AwaitingHumanApproval ---
            (Status::AwaitingHumanApproval, Payload::HumanApproved) => {
                self.status = Status::Accepted;
                Ok(self)
            }

            (Status::AwaitingHumanApproval, Payload::HumanRejected { .. }) => {
                self.status = Status::Rejected;
                Ok(self)
            }

            // --- Terminal states (no transitions except Disputed) ---
            (Status::Settled, Payload::Disputed { reason, .. }) => {
                self.dispute_reason = Some(reason.clone());
                self.status = Status::Disputed;
                Ok(self)
            }

            (state, payload) => Err(TransitionError::Invalid {
                event: payload.kind().to_string(),
                state: *state,
            }),
        }
    }

    // --- Helper methods ---

    fn ensure_commitment_valid(
        &self,
        commitment: &Commitment,
        now: Timestamp,
    ) -> Result<(), TransitionError> {
        if commitment.is_expired(now) {
            return Err(TransitionError::CommitmentExpired {
                expires_at: commitment.expires_at,
            });
        }
        Ok(())
    }

    fn ensure_sender_is_buyer(&self, sender: &Identity) -> Result<(), TransitionError> {
        match &self.buyer {
            Some(buyer) if buyer == sender => Ok(()),
            _ => Err(TransitionError::Unauthorized { sender: sender.clone() }),
        }
    }

    fn ensure_sender_is_seller(&self, sender: &Identity) -> Result<(), TransitionError> {
        if self.sellers.contains_key(sender) {
            Ok(())
        } else {
            Err(TransitionError::Unauthorized { sender: sender.clone() })
        }
    }

    fn ensure_sender_is_buyer_or_seller(&self, sender: &Identity) -> Result<(), TransitionError> {
        if self.buyer.as_ref() == Some(sender) || self.sellers.contains_key(sender) {
            Ok(())
        } else {
            Err(TransitionError::Unauthorized { sender: sender.clone() })
        }
    }

    fn update_current_offer(&mut self, commitment: &Commitment) -> Result<(), TransitionError> {
        // Currency consistency check against RFQ
        if let Some(ref rfq) = self.rfq {
            let rfq_currency = match &rfq.payload {
                CommitmentPayload::Rfq(r) => &r.currency,
                _ => unreachable!(),
            };
            let offer_currency = match &commitment.payload {
                CommitmentPayload::Quote(q) => &q.currency,
                CommitmentPayload::CounterOffer(o) => &o.currency,
                _ => rfq_currency, // acceptance/rejection don't carry currency
            };
            if offer_currency != rfq_currency {
                return Err(TransitionError::CurrencyMismatch {
                    expected: rfq_currency.clone(),
                    actual: offer_currency.clone(),
                });
            }
        }
        self.current_offer = Some(commitment.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{
        commitment::{Commitment, CommitmentKind, CommitmentPayload, RFQ},
        event::{Event, EventId, Payload},
        identity::Keypair,
        Nonce, PaymentMethod, Product, Quantity,
    };
    use chrono::Utc;
    use rust_decimal::Decimal;

    fn dummy_product() -> Product {
        Product {
            id: "p1".into(),
            name: "Widget".into(),
            description: "A widget".into(),
            category: "Tools".into(),
            unit_price: Decimal::new(10000, 2),
            currency: "USD".into(),
            stock_quantity: 100,
            metadata: Default::default(),
        }
    }

    fn rfq_commitment(buyer: &Keypair, _sellers: Vec<Identity>, ttl_sec: i64) -> Commitment {
        let payload = CommitmentPayload::Rfq(RFQ {
            product: dummy_product(),
            quantity: Quantity { amount: 1, unit: "each".into() },
            max_unit_price: Decimal::new(12000, 2),
            currency: "USD".into(),
            delivery: None,
            payment_methods: vec![PaymentMethod::stripe()],
            ttl_seconds: ttl_sec as u32,
        });
        let canonical = serde_json::to_vec(&payload).unwrap();
        Commitment {
            kind: CommitmentKind::Rfq,
            sender: buyer.identity(),
            payload,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(ttl_sec),
            signature: buyer.sign(&canonical),
        }
    }

    fn event(buyer: &Keypair, neg_id: NegotiationId, payload: Payload) -> Event {
        let canonical = serde_json::to_vec(&payload).unwrap();
        Event {
            id: EventId::new(),
            negotiation_id: neg_id,
            payload,
            sender: buyer.identity(),
            timestamp: Utc::now(),
            nonce: Nonce::random(),
            signature: buyer.sign(&canonical),
        }
    }

    #[test]
    fn full_negotiation_lifecycle() {
        let buyer_kp = Keypair::generate();
        let seller_kp = Keypair::generate();
        let seller_id = seller_kp.identity();
        let neg_id = NegotiationId::new();

        // 1. RFQ
        let rfq = rfq_commitment(&buyer_kp, vec![seller_id.clone()], 3600);
        let e1 = event(&buyer_kp, neg_id, Payload::RfqSubmitted {
            commitment: rfq.clone(),
            recipients: vec![seller_id.clone()],
        });
        let state = NegotiationState::init(&e1).unwrap();
        assert_eq!(state.status, Status::Pending);
        assert_eq!(state.buyer, Some(buyer_kp.identity()));

        // 2. Quote
        let quote_payload = CommitmentPayload::Quote(crate::kernel::commitment::Quote {
            rfq_hash: "hash".into(),
            unit_price: Decimal::new(11000, 2),
            currency: "USD".into(),
            available_quantity: Quantity { amount: 1, unit: "each".into() },
            delivery_estimate: None,
            ttl_seconds: 1800,
        });
        let quote_canonical = serde_json::to_vec(&quote_payload).unwrap();
        let quote_commitment = Commitment {
            kind: CommitmentKind::Quote,
            sender: seller_id,
            payload: quote_payload,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(1800),
            signature: seller_kp.sign(&quote_canonical),
        };
        let e2 = event(&seller_kp, neg_id, Payload::Quoted { commitment: quote_commitment });
        let state = state.apply(&e2).unwrap();
        assert_eq!(state.status, Status::Quoted);

        // 3. Accept
        let accept_payload = CommitmentPayload::Acceptance { final_price: Decimal::new(11000, 2), currency: "USD".into() };
        let accept_canonical = serde_json::to_vec(&accept_payload).unwrap();
        let accept_commitment = Commitment {
            kind: CommitmentKind::Acceptance,
            sender: buyer_kp.identity(),
            payload: accept_payload,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(3600),
            signature: buyer_kp.sign(&accept_canonical),
        };
        let e3 = event(&buyer_kp, neg_id, Payload::Accepted { commitment: accept_commitment });
        let state = state.apply(&e3).unwrap();
        assert_eq!(state.status, Status::Accepted);

        // 4. Settle
        let e4 = event(&seller_kp, neg_id, Payload::Settled {
            settlement_id: "settle_123".into(),
            payment_method: "stripe".into(),
        });
        let state = state.apply(&e4).unwrap();
        assert_eq!(state.status, Status::Settled);
    }

    #[test]
    fn invalid_transition_quoted_to_settled() {
        let buyer_kp = Keypair::generate();
        let seller_kp = Keypair::generate();
        let neg_id = NegotiationId::new();

        let rfq = rfq_commitment(&buyer_kp, vec![seller_kp.identity()], 3600);
        let e1 = event(&buyer_kp, neg_id, Payload::RfqSubmitted {
            commitment: rfq,
            recipients: vec![seller_kp.identity()],
        });
        let state = NegotiationState::init(&e1).unwrap();

        let e2 = event(&seller_kp, neg_id, Payload::Settled {
            settlement_id: "x".into(),
            payment_method: "stripe".into(),
        });
        assert!(state.apply(&e2).is_err());
    }

    #[test]
    fn expired_commitment_rejected() {
        let buyer_kp = Keypair::generate();
        let seller_kp = Keypair::generate();
        let neg_id = NegotiationId::new();

        let rfq = rfq_commitment(&buyer_kp, vec![seller_kp.identity()], 3600);
        let e1 = event(&buyer_kp, neg_id, Payload::RfqSubmitted {
            commitment: rfq,
            recipients: vec![seller_kp.identity()],
        });
        let state = NegotiationState::init(&e1).unwrap();

        // Create a quote that is already expired
        let quote_payload = CommitmentPayload::Quote(crate::kernel::commitment::Quote {
            rfq_hash: "hash".into(),
            unit_price: Decimal::new(11000, 2),
            currency: "USD".into(),
            available_quantity: Quantity { amount: 1, unit: "each".into() },
            delivery_estimate: None,
            ttl_seconds: 1,
        });
        let quote_canonical = serde_json::to_vec(&quote_payload).unwrap();
        let quote_commitment = Commitment {
            kind: CommitmentKind::Quote,
            sender: seller_kp.identity(),
            payload: quote_payload,
            created_at: Utc::now() - chrono::Duration::seconds(10),
            expires_at: Utc::now() - chrono::Duration::seconds(5),
            signature: seller_kp.sign(&quote_canonical),
        };
        let e2 = event(&seller_kp, neg_id, Payload::Quoted { commitment: quote_commitment });
        assert!(matches!(state.apply(&e2), Err(TransitionError::CommitmentExpired { .. })));
    }
}
