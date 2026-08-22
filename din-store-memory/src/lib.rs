//! In-memory event store — an adapter for `din_core::store::EventStore`.
//!
//! Useful for tests and ephemeral (non-persistent) nodes.

use din_core::event::Envelope;
use din_core::store::{EventStore, StoreError};

#[derive(Default, Clone)]
pub struct MemoryStore {
    events: Vec<Envelope>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl EventStore for MemoryStore {
    fn append(&mut self, env: &Envelope) -> Result<(), StoreError> {
        if self.events.iter().any(|e| e.event_id == env.event_id) {
            return Err(StoreError::Duplicate {
                event_id: env.event_id,
            });
        }
        self.events.push(env.clone());
        Ok(())
    }

    fn load_all(&self) -> Result<Vec<Envelope>, StoreError> {
        Ok(self.events.clone())
    }
}
