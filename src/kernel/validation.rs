//! Invariant validation for events before they enter the state machine.
//!
//! Validation checks structural and cryptographic well-formedness.
//! It does NOT check negotiation-specific transitions; that is the state machine's job.

use super::{commitment::CommitmentPayload, event::Payload, Commitment, CommitmentKind, Event, Timestamp};

/// Errors that indicate an event is structurally invalid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("event signature verification failed")]
    SignatureInvalid,
    #[error("commitment signature verification failed")]
    CommitmentSignatureInvalid,
    #[error("RFQ missing recipients")]
    RfqMissingRecipients,
    #[error("quote missing RFQ hash")]
    QuoteMissingRfqHash,
    #[error("offer price is not positive: {0}")]
    NonPositivePrice(String),
    #[error("TTL must be > 0, got {0}")]
    InvalidTtl(u32),
    #[error("currency must not be empty")]
    EmptyCurrency,
    #[error("quantity must be > 0")]
    ZeroQuantity,
}

/// Validate an event and its nested commitments.
pub fn validate_event(event: &Event, now: Timestamp) -> Result<(), ValidationError> {
    // 1. Verify event signature
    event.verify_signature().map_err(|_| ValidationError::SignatureInvalid)?;

    // 2. Validate payload-specific rules
    match &event.payload {
        Payload::RfqSubmitted { commitment, recipients } => {
            validate_commitment(commitment, now)?;
            if recipients.is_empty() {
                return Err(ValidationError::RfqMissingRecipients);
            }
            if !matches!(commitment.kind, CommitmentKind::Rfq) {
                return Err(ValidationError::CommitmentSignatureInvalid);
            }
        }
        Payload::Quoted { commitment } => {
            validate_commitment(commitment, now)?;
            if !matches!(commitment.kind, CommitmentKind::Quote) {
                return Err(ValidationError::CommitmentSignatureInvalid);
            }
        }
        Payload::Countered { commitment } => {
            validate_commitment(commitment, now)?;
            if !matches!(commitment.kind, CommitmentKind::CounterOffer) {
                return Err(ValidationError::CommitmentSignatureInvalid);
            }
        }
        Payload::Accepted { commitment } => {
            validate_commitment(commitment, now)?;
            if !matches!(commitment.kind, CommitmentKind::Acceptance) {
                return Err(ValidationError::CommitmentSignatureInvalid);
            }
        }
        Payload::Rejected { commitment } => {
            validate_commitment(commitment, now)?;
            if !matches!(commitment.kind, CommitmentKind::Rejection) {
                return Err(ValidationError::CommitmentSignatureInvalid);
            }
        }
        Payload::Settled { .. }
        | Payload::Expired { .. }
        | Payload::Disputed { .. }
        | Payload::HumanApprovalRequested { .. }
        | Payload::HumanApproved
        | Payload::HumanRejected { .. } => {
            // No embedded commitment to validate
        }
    }

    Ok(())
}

/// Validate a commitment's structural invariants.
pub fn validate_commitment(commitment: &Commitment, _now: Timestamp) -> Result<(), ValidationError> {
    // Verify commitment signature
    commitment
        .verify_signature()
        .map_err(|_| ValidationError::CommitmentSignatureInvalid)?;

    // Validate payload specifics
    match &commitment.payload {
        CommitmentPayload::Rfq(rfq) => {
            if rfq.ttl_seconds == 0 {
                return Err(ValidationError::InvalidTtl(0));
            }
            if rfq.currency.is_empty() {
                return Err(ValidationError::EmptyCurrency);
            }
            if rfq.quantity.amount == 0 {
                return Err(ValidationError::ZeroQuantity);
            }
            if rfq.max_unit_price <= rust_decimal::Decimal::ZERO {
                return Err(ValidationError::NonPositivePrice(rfq.max_unit_price.to_string()));
            }
        }
        CommitmentPayload::Quote(q) => {
            if q.ttl_seconds == 0 {
                return Err(ValidationError::InvalidTtl(0));
            }
            if q.currency.is_empty() {
                return Err(ValidationError::EmptyCurrency);
            }
            if q.available_quantity.amount == 0 {
                return Err(ValidationError::ZeroQuantity);
            }
            if q.unit_price <= rust_decimal::Decimal::ZERO {
                return Err(ValidationError::NonPositivePrice(q.unit_price.to_string()));
            }
            if q.rfq_hash.is_empty() {
                return Err(ValidationError::QuoteMissingRfqHash);
            }
        }
        CommitmentPayload::CounterOffer(o) => {
            if o.ttl_seconds == 0 {
                return Err(ValidationError::InvalidTtl(0));
            }
            if o.currency.is_empty() {
                return Err(ValidationError::EmptyCurrency);
            }
            if o.quantity.amount == 0 {
                return Err(ValidationError::ZeroQuantity);
            }
            if o.unit_price <= rust_decimal::Decimal::ZERO {
                return Err(ValidationError::NonPositivePrice(o.unit_price.to_string()));
            }
        }
        CommitmentPayload::Acceptance { final_price, currency } => {
            if currency.is_empty() {
                return Err(ValidationError::EmptyCurrency);
            }
            if *final_price <= rust_decimal::Decimal::ZERO {
                return Err(ValidationError::NonPositivePrice(final_price.to_string()));
            }
        }
        CommitmentPayload::Rejection { .. } => {
            // No additional constraints
        }
    }

    Ok(())
}
