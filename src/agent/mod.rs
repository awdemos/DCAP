//! Agent runtime and strategy plugins.
//!
//! ## Modules
//!
//! - `runtime` — Actor-like mailbox + event loop (future)
//! - `buyer` — Buyer-specific behavior (future)
//! - `seller` — Seller-specific behavior (future)
//! - `strategy` — Trait for negotiation strategies (future)
//! - `legacy` — Original BuyerAgent / SellerAgent implementations (transitional)

pub mod legacy;

// Re-export legacy types for backward compatibility
pub use legacy::{BuyerAgent, BuyerAgentConfig, SellerAgent, SellerAgentConfig, LLMConfig};
