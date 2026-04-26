use thiserror::Error;

// ---------------------------------------------------------------------------
// Domain-specific error types
// ---------------------------------------------------------------------------

/// Errors originating in the protocol kernel (state machine, validation, crypto).
#[derive(Error, Debug, Clone, PartialEq)]
pub enum KernelError {
    #[error("invalid state transition: {details}")]
    InvalidTransition { details: String },

    #[error("signature verification failed")]
    SignatureInvalid,

    #[error("commitment expired at {expires_at}")]
    CommitmentExpired { expires_at: String },

    #[error("unauthorized sender: {sender}")]
    Unauthorized { sender: String },

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("currency mismatch: expected {expected}, got {actual}")]
    CurrencyMismatch { expected: String, actual: String },

    #[error("duplicate event: {0}")]
    DuplicateEvent(String),

    #[error("cryptographic error: {0}")]
    Crypto(String),
}

/// Errors originating from persistence adapters.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum StoreError {
    #[error("io error: {0}")]
    Io(String),

    #[error("serialization error: {0}")]
    Serialize(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("unavailable")]
    Unavailable,
}

/// Errors originating from network transport adapters.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum NetworkError {
    #[error("io error: {0}")]
    Io(String),

    #[error("timeout")]
    Timeout,

    #[error("decode error: {0}")]
    Decode(String),

    #[error("unreachable: {0}")]
    Unreachable(String),

    #[error("rejected by recipient")]
    Rejected,
}

/// Errors originating from settlement adapters.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum SettlementError {
    #[error("insufficient funds")]
    InsufficientFunds,

    #[error("payment method unavailable: {0}")]
    MethodUnavailable(String),

    #[error("rejected by processor: {0}")]
    Rejected(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("timeout")]
    Timeout,

    #[error("unavailable")]
    Unavailable,
}

/// Errors originating from trust engines.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum TrustError {
    #[error("insufficient data")]
    InsufficientData,

    #[error("computation error: {0}")]
    Computation(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("unavailable")]
    Unavailable,
}

/// Errors originating from discovery adapters.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum DiscoveryError {
    #[error("agent not found: {0}")]
    NotFound(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("timeout")]
    Timeout,

    #[error("unavailable")]
    Unavailable,
}

// ---------------------------------------------------------------------------
// Legacy unified error (transitional — old modules reference this)
// ---------------------------------------------------------------------------

pub type Result<T> = std::result::Result<T, NegotiationError>;

#[derive(Error, Debug)]
pub enum NegotiationError {
    #[error("Invalid configuration: {0}")]
    Config(String),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Negotiation failed: {0}")]
    Negotiation(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Payment error: {0}")]
    Payment(String),

    #[error("Trust validation failed: {0}")]
    Trust(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Agent not found: {0}")]
    AgentNotFound(crate::AgentId),

    #[error("Product not found: {0}")]
    ProductNotFound(String),

    #[error("Quote expired")]
    QuoteExpired,

    #[error("Insufficient reputation score: {0}")]
    InsufficientReputation(u32),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    // New domain errors for transitional use
    #[error("Kernel error: {0}")]
    Kernel(#[from] KernelError),

    #[error("Store error: {0}")]
    Store(#[from] StoreError),
}

impl From<serde_json::Error> for NegotiationError {
    fn from(err: serde_json::Error) -> Self {
        NegotiationError::Serialization(err.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for NegotiationError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        NegotiationError::Auth(err.to_string())
    }
}

impl From<uuid::Error> for NegotiationError {
    fn from(err: uuid::Error) -> Self {
        NegotiationError::Validation(err.to_string())
    }
}

impl From<std::io::Error> for NegotiationError {
    fn from(err: std::io::Error) -> Self {
        NegotiationError::Io(err.to_string())
    }
}
