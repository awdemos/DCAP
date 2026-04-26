//! In-memory event store for testing and development.
//!
//! This adapter is fast and requires no external dependencies, but all data is lost
//! on process termination. Use the SQLite adapter for durability.

use async_trait::async_trait;
use crate::kernel::{Event, EventId, NegotiationId};
use crate::ports::event_store::{EventStore, EventStoreError, EventStream};
use parking_lot::RwLock;
use std::collections::HashMap;

/// A thread-safe in-memory event store backed by a `HashMap`.
pub struct MemoryEventStore {
    inner: RwLock<HashMap<NegotiationId, Vec<Event>>>,
}

impl MemoryEventStore {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventStore for MemoryEventStore {
    async fn append(&self, event: &Event) -> Result<(), EventStoreError> {
        let mut guard = self.inner.write();
        let stream = guard.entry(event.negotiation_id).or_default();

        // Idempotency check
        if stream.iter().any(|e| e.id == event.id) {
            return Err(EventStoreError::Conflict(event.id));
        }

        stream.push(event.clone());
        Ok(())
    }

    async fn read_stream(&self, negotiation_id: NegotiationId) -> Result<Vec<Event>, EventStoreError> {
        let guard = self.inner.read();
        let events = guard.get(&negotiation_id).cloned().unwrap_or_default();
        Ok(events)
    }

    async fn read_stream_after(
        &self,
        negotiation_id: NegotiationId,
        after: EventId,
    ) -> Result<Vec<Event>, EventStoreError> {
        let guard = self.inner.read();
        let stream = guard.get(&negotiation_id).cloned().unwrap_or_default();

        let mut found = false;
        let result: Vec<Event> = stream
            .into_iter()
            .skip_while(|e| {
                if found {
                    return false;
                }
                if e.id == after {
                    found = true;
                }
                true
            })
            .collect();

        Ok(result)
    }

    async fn subscribe(&self, _negotiation_id: NegotiationId) -> Result<EventStream, EventStoreError> {
        // In-memory store does not support live streaming without additional channels.
        // For now, return an empty stream. Production adapters (SQLite, Kafka) will implement this.
        let stream = futures::stream::empty();
        Ok(Box::pin(stream))
    }

    async fn exists(&self, event_id: EventId) -> Result<bool, EventStoreError> {
        let guard = self.inner.read();
        Ok(guard.values().any(|stream| stream.iter().any(|e| e.id == event_id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{
        commitment::{Commitment, CommitmentKind, CommitmentPayload, RFQ, Quantity},
        event::{Event, EventId, Payload},
        identity::Keypair,
        Nonce, PaymentMethod, Product, Timestamp,
    };
    use chrono::Utc;
    use rust_decimal::Decimal;

    fn dummy_event() -> Event {
        let kp = Keypair::generate();
        let payload = CommitmentPayload::Rfq(RFQ {
            product: Product {
                id: "p1".into(),
                name: "Widget".into(),
                description: "W".into(),
                category: "C".into(),
                unit_price: Decimal::ONE,
                currency: "USD".into(),
                stock_quantity: 1,
                metadata: Default::default(),
            },
            quantity: Quantity { amount: 1, unit: "each".into() },
            max_unit_price: Decimal::ONE,
            currency: "USD".into(),
            delivery: None,
            payment_methods: vec![PaymentMethod::stripe()],
            ttl_seconds: 3600,
        });
        let canonical = serde_json::to_vec(&payload).unwrap();
        let commitment = Commitment {
            kind: CommitmentKind::Rfq,
            sender: kp.identity(),
            payload,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(3600),
            signature: kp.sign(&canonical),
        };
        let event_payload = Payload::RfqSubmitted {
            commitment,
            recipients: vec![],
        };
        let event_canonical = serde_json::to_vec(&event_payload).unwrap();
        Event {
            id: EventId::new(),
            negotiation_id: crate::kernel::NegotiationId::new(),
            payload: event_payload,
            sender: kp.identity(),
            timestamp: Utc::now(),
            nonce: Nonce::random(),
            signature: kp.sign(&event_canonical),
        }
    }

    #[tokio::test]
    async fn append_and_read() {
        let store = MemoryEventStore::new();
        let event = dummy_event();
        store.append(&event).await.unwrap();

        let read = store.read_stream(event.negotiation_id).await.unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].id, event.id);
    }

    #[tokio::test]
    async fn idempotent_append() {
        let store = MemoryEventStore::new();
        let event = dummy_event();
        store.append(&event).await.unwrap();
        let result = store.append(&event).await;
        assert!(matches!(result, Err(EventStoreError::Conflict(_))));
    }

    #[tokio::test]
    async fn exists_check() {
        let store = MemoryEventStore::new();
        let event = dummy_event();
        assert!(!store.exists(event.id).await.unwrap());
        store.append(&event).await.unwrap();
        assert!(store.exists(event.id).await.unwrap());
    }
}
