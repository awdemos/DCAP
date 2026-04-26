//! Event store port: append-only persistence for domain events.
//!
//! The event store is the single source of truth. All state is derived from
//! the events it stores. This port must guarantee durability and ordering.

use async_trait::async_trait;
use crate::kernel::{Event, EventId, NegotiationId};
use std::pin::Pin;
use futures::Stream;

/// A stream of events, returned by subscribe operations.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<Event, EventStoreError>> + Send>>;

/// Errors that can occur when interacting with the event store.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventStoreError {
    #[error("io error: {0}")]
    Io(String),
    #[error("serialization error: {0}")]
    Serialize(String),
    #[error("conflict: event {0} already exists")]
    Conflict(EventId),
    #[error("not found: negotiation {0}")]
    NotFound(NegotiationId),
    #[error("unavailable")]
    Unavailable,
}

/// Append-only store for domain events.
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Append an event to the store.
    ///
    /// Must be atomic and durable. If the event already exists (by `EventId`),
    /// returns `EventStoreError::Conflict`.
    async fn append(&self, event: &Event) -> Result<(), EventStoreError>;

    /// Read all events for a negotiation, in order.
    async fn read_stream(&self, negotiation_id: NegotiationId) -> Result<Vec<Event>, EventStoreError>;

    /// Read events for a negotiation starting after a specific event ID.
    async fn read_stream_after(
        &self,
        negotiation_id: NegotiationId,
        after: EventId,
    ) -> Result<Vec<Event>, EventStoreError>;

    /// Subscribe to new events for a negotiation.
    ///
    /// Returns a stream that yields events as they are appended.
    async fn subscribe(&self, negotiation_id: NegotiationId) -> Result<EventStream, EventStoreError>;

    /// Check if an event ID already exists.
    async fn exists(&self, event_id: EventId) -> Result<bool, EventStoreError>;
}
