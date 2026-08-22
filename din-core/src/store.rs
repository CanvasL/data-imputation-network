//! Storage abstraction (ports & adapters, PROTOCOL.md §5).
//!
//! The protocol core is deliberately **storage-agnostic**: it only declares
//! *what* it needs from a persistence backend, not *which* one. Concrete
//! backends (SQLite, Postgres, in-memory, ...) live in separate adapter
//! crates (e.g. `din-store-sqlite`, `din-store-memory`).

use std::error::Error;
use std::fmt;

use crate::event::Envelope;

#[derive(Debug)]
pub enum StoreError {
    Io(String),
    Serialization(String),
    Duplicate { event_id: u64 },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "store io error: {e}"),
            StoreError::Serialization(e) => write!(f, "store serialization error: {e}"),
            StoreError::Duplicate { event_id } => write!(f, "duplicate event: {event_id}"),
        }
    }
}

impl Error for StoreError {}

/// A backend for the append-only event log.
///
/// Implementors must guarantee:
/// - `append` is idempotent per `event_id` (duplicates rejected);
/// - `load_all` returns events in append order (event_id ascending).
pub trait EventStore {
    fn append(&mut self, env: &Envelope) -> Result<(), StoreError>;
    fn load_all(&self) -> Result<Vec<Envelope>, StoreError>;
}
