//! # DCAP - Decentralized Commerce Agentic Protocol
//!
//! A decentralized commerce protocol for LLM-to-LLM negotiation.
//!
//! ## Architecture
//!
//! - **Buyer Agent**: Rust + reqwest + serde - LLM that browses product feeds and negotiates
//! - **Seller Agent**: Axum server exposing /quote endpoint - LLM that fields RFQs
//! - **Discovery**: Simple POST registry for onboarding and search
//! - **Settlement**: Stripe, Solana, or pay-on-delivery escrow
//! - **Trust/Reputation**: Signed JWT + SQLite ledger to prevent sybil attacks
//! - **MCP Server**: Custom implementation for standardized LLM-to-LLM communication

pub mod agent;
pub mod config;
pub mod discovery;
pub mod error;
pub mod model;
pub mod settlement;
pub mod trust;
pub mod mcp;
pub mod sgx_quote;

pub use agent::{BuyerAgent, SellerAgent};
pub use config::AppConfig;
pub use discovery::{DiscoveryService, RegisterRequest, SearchRequest};
pub use error::{NegotiationError, Result};
pub use model::{NegotiationRecord, Product, Quote, RFQ, PaymentMethod};
pub use settlement::SettlementService;
pub use trust::{TrustSystem, ReputationScore};
pub use sgx_quote::{SgxQuoteManager, SgxConfig, SgxQuote};


pub type TransactionId = uuid::Uuid;
pub type AgentId = uuid::Uuid;