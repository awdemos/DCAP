//! # DCAP - Decentralized Commerce Agentic Protocol
//!
//! A decentralized commerce protocol for LLM-to-LLM negotiation.
//!
//! ## New Architecture (v2)
//!
//! The codebase is transitioning to a kernel-centric, event-sourced architecture:
//!
//! - **`kernel/`** — The irreducible core: identity, commitments, events, state machine.
//!   All participants must agree on the kernel. It has no external dependencies.
//! - **`ports/`** — Trait definitions for adapters (discovery, settlement, trust, network, event store).
//! - **`adapters/`** — Concrete implementations of ports (to be populated).
//! - **`protocol/`** — Wire format and MCP mapping (to be populated).
//! - **`agent/`** — Agent runtime and strategy plugins (to be populated).
//!
//! ## Legacy Modules (v1, transitional)
//!
//! The following modules represent the original MVP implementation. They are preserved
//! for backward compatibility during the migration:
//!
//! - `agent` — BuyerAgent / SellerAgent structs
//! - `discovery` — HTTP registry client/server
//! - `settlement` — Mock payment processing
//! - `trust` — In-memory reputation cache
//! - `model` — Original data models
//! - `database` — SQLite schema (unused by binaries)
//! - `mcp` — Custom TCP protocol (non-compliant, to be replaced)
//!
//! ## Design Principles
//!
//! 1. **Events are the API.** All state changes flow through immutable, signed events.
//! 2. **Pure state machine.** `NegotiationState` is a deterministic fold over events.
//! 3. **Ports and adapters.** The kernel depends only on traits; infrastructure is injected.
//! 4. **No floating-point money.** `Decimal` is used everywhere for prices.
//! 5. **Self-sovereign identity.** Ed25519 keypairs, not UUIDs or shared secrets.

// New architecture modules
pub mod kernel;
pub mod ports;

// Legacy modules (transitional)
pub mod agent;
pub mod config;
pub mod discovery;
pub mod error;
pub mod model;
pub mod database;
pub mod settlement;
pub mod trust;
pub mod mcp;

// Re-exports for convenience
pub use agent::{BuyerAgent, SellerAgent};
pub use config::AppConfig;
pub use discovery::{DiscoveryService, RegisterRequest, SearchRequest};
pub use error::{NegotiationError, Result, KernelError, StoreError, NetworkError, SettlementError, TrustError, DiscoveryError};
pub use model::{NegotiationRecord, Product, Quote as LegacyQuote, RFQ as LegacyRFQ, PaymentMethod};
pub use settlement::SettlementService;
pub use trust::{TrustSystem, ReputationScore};

// Legacy type aliases
pub type TransactionId = uuid::Uuid;
pub type AgentId = uuid::Uuid;
