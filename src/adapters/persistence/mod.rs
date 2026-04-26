//! Persistence adapters for the `EventStore` port.

pub mod memory;
// pub mod sqlite; // Future: migrate database.rs into here

pub use memory::MemoryEventStore;
