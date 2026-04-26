//! Trust engine port: compute trust vectors from observable history.
//!
//! Trust is subjective and derived. The kernel does not store reputation scores;
//! it only requires that adapters can produce `TrustVector`s on demand.

use async_trait::async_trait;
use crate::kernel::Identity;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Errors that can occur during trust computation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TrustError {
    #[error("insufficient data for {0}")]
    InsufficientData(Identity),
    #[error("computation error: {0}")]
    Computation(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("unavailable")]
    Unavailable,
}

/// A multi-dimensional trust vector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrustVector {
    /// Probability [0,1] that the agent honors commitments.
    pub reliability: f64,
    /// Response time percentile (ms).
    pub responsiveness: f64,
    /// Disputes per 100 transactions.
    pub dispute_rate: f64,
    /// Log-scaled total settled value.
    pub volume_weight: f64,
    /// Category-specific expertise scores.
    pub category_expertise: HashMap<String, f64>,
    /// Time decay factor (0 = all old, 1 = all recent).
    pub recency: f64,
    /// Overall composite score [0,1], computed by the engine.
    pub composite: f64,
}

/// Context for a trust query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustContext {
    pub category: Option<String>,
    pub min_transactions: Option<u64>,
    pub max_age_days: Option<u32>,
}

/// A signed attestation of a transaction outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attestation {
    pub attester: Identity,
    pub subject: Identity,
    pub transaction_hash: String,
    pub outcome: AttestationOutcome,
    pub timestamp: crate::kernel::Timestamp,
    pub signature: crate::kernel::Signature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestationOutcome {
    Success,
    Failure,
    Disputed,
}

/// The trust engine port.
#[async_trait]
pub trait TrustEngine: Send + Sync {
    /// Submit an attestation to the trust system.
    async fn attest(&self, attestation: &Attestation) -> Result<(), TrustError>;

    /// Query the trust vector for an agent in a given context.
    async fn query(
        &self,
        identity: &Identity,
        context: &TrustContext,
    ) -> Result<TrustVector, TrustError>;

    /// Query multiple agents at once.
    async fn query_batch(
        &self,
        identities: &[Identity],
        context: &TrustContext,
    ) -> Result<Vec<(Identity, TrustVector)>, TrustError>;
}
