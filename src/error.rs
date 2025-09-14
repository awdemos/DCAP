use std::fmt;
use thiserror::Error;
use crate::AgentId;

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
    AgentNotFound(AgentId),

    #[error("Product not found: {0}")]
    ProductNotFound(String),

    #[error("Quote expired")]
    QuoteExpired,

    #[error("Insufficient reputation score: {0}")]
    InsufficientReputation(u32),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
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