//! # DCAP Adapters
//!
//! Concrete implementations of the ports defined in `dcap::ports`.
//! Each adapter lives in its own submodule and depends only on the port trait it implements.

pub mod discovery;
pub mod persistence;
pub mod settlement;
pub mod transport;
pub mod trust;
