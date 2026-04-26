//! Network port: send and receive signed envelopes between agents.
//!
//! This port abstracts over HTTP, WebSocket, libp2p, MCP stdio, or any other
//! transport. The kernel only sees `Envelope`s.

use async_trait::async_trait;
use crate::kernel::{Identity, Signature, Timestamp};
use serde::{Deserialize, Serialize};

/// Errors that can occur during network operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    #[error("io error: {0}")]
    Io(String),
    #[error("timeout")]
    Timeout,
    #[error("decode error: {0}")]
    Decode(String),
    #[error("unreachable: {0}")]
    Unreachable(Identity),
    #[error("rejected by recipient")]
    Rejected,
}

/// A signed, routed message container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    pub sender: Identity,
    pub recipient: Identity,
    pub payload: Vec<u8>,
    pub timestamp: Timestamp,
    pub signature: Signature,
}

/// The network port.
#[async_trait]
pub trait Network: Send + Sync {
    /// Send an envelope to its recipient.
    async fn send(&self, envelope: &Envelope) -> Result<(), TransportError>;

    /// Receive the next envelope addressed to us.
    ///
    /// This is a pull-based interface. Implementations may internally use
    /// polling, WebSocket listeners, or actor mailboxes.
    async fn recv(&self) -> Result<Envelope, TransportError>;

    /// Register our identity so the transport knows which envelopes are ours.
    async fn bind(&self, identity: &Identity) -> Result<(), TransportError>;
}
