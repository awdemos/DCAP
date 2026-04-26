//! # DCAP Ports
//!
//! Ports define the boundaries between the kernel and the external world.
//! Each port is a trait that the kernel depends on. Adapters implement these traits.
//!
//! This follows the Ports and Adapters (Hexagonal) architecture:
//! - The kernel is at the center.
//! - Ports face outward.
//! - Adapters sit outside and implement the ports.
//!
//! Adding a new capability (e.g., a new settlement rail) requires only a new adapter,
//! never a change to the kernel or existing adapters.

pub mod discovery;
pub mod event_store;
pub mod network;
pub mod settlement;
pub mod trust;

pub use discovery::{Discovery, Query, AgentRecord, Capability};
pub use event_store::{EventStore, EventStream};
pub use network::{Network, Envelope, TransportError};
pub use settlement::{Settlement, PaymentIntent, SettlementResult};
pub use trust::{TrustEngine, TrustVector, TrustContext, Attestation};
