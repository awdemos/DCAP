//! Discovery port: resolve identities to capabilities and endpoints.
//!
//! Discovery is not part of the kernel because different deployments will use
//! different discovery mechanisms (centralized registry, DHT, DNS, static files).

use async_trait::async_trait;
use crate::kernel::{Identity, PaymentMethod, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Errors that can occur during discovery.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiscoveryError {
    #[error("not found: {0}")]
    NotFound(Identity),
    #[error("network error: {0}")]
    Network(String),
    #[error("timeout")]
    Timeout,
    #[error("unavailable")]
    Unavailable,
}

/// A record describing an agent's capabilities and how to reach it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRecord {
    pub identity: Identity,
    pub endpoints: Vec<String>,
    pub capabilities: Vec<Capability>,
    pub payment_methods: Vec<PaymentMethod>,
    pub last_seen: Timestamp,
    pub metadata: BTreeMap<String, String>,
}

/// A capability advertised by an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub category: String,
    pub subcategory: Option<String>,
    pub schema_url: Option<String>,
}

/// A query for searching agents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Query {
    pub category: Option<String>,
    pub payment_method: Option<PaymentMethod>,
    pub min_trust_threshold: Option<f64>,
    pub limit: Option<usize>,
}

/// The discovery port.
#[async_trait]
pub trait Discovery: Send + Sync {
    /// Announce this agent's record to the discovery system.
    async fn announce(&self, record: &AgentRecord) -> Result<(), DiscoveryError>;

    /// Resolve an identity to its current record.
    async fn resolve(&self, identity: &Identity) -> Result<AgentRecord, DiscoveryError>;

    /// Search for agents matching a query.
    async fn search(&self, query: &Query) -> Result<Vec<AgentRecord>, DiscoveryError>;

    /// Validate that an agent's endpoint is reachable.
    async fn validate(&self, identity: &Identity) -> Result<bool, DiscoveryError>;
}
