//! Incremental execution node.
//!
//! `Node<S>` is the event-sourced engine: it owns a storage backend `S`
//! (any `EventStore`), validates each incoming event (hash chain, payload
//! hash, author signature), appends it to the log, and applies it to the
//! in-memory `Projection`. All state is a projection of the event log.

use std::collections::HashMap;

use crate::event::{Envelope, Keypair};
use crate::log::{Projection, ReplayError, apply_event, replay};
use crate::model::{AgentId, DatasetId, hash_bytes};
use crate::store::{EventStore, StoreError};

#[derive(Debug)]
pub enum NodeError {
    Store(StoreError),
    Replay(ReplayError),
    InvalidChain { event_id: u64 },
    InvalidPayload { event_id: u64 },
    UnknownAuthor { agent: AgentId },
    Signature { event_id: u64, msg: String },
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeError::Store(e) => write!(f, "store: {e}"),
            NodeError::Replay(e) => write!(f, "replay: {e:?}"),
            NodeError::InvalidChain { event_id } => {
                write!(f, "invalid hash chain at event {event_id}")
            }
            NodeError::InvalidPayload { event_id } => {
                write!(f, "invalid payload hash at event {event_id}")
            }
            NodeError::UnknownAuthor { agent } => write!(f, "unknown author {agent:?}"),
            NodeError::Signature { event_id, msg } => {
                write!(f, "signature failed at event {event_id}: {msg}")
            }
        }
    }
}
impl std::error::Error for NodeError {}

pub struct Node<S: EventStore> {
    store: S,
    projection: Projection,
    /// AgentId -> public key hex registry.
    pubkeys: HashMap<AgentId, String>,
    tail_hash: String,
    last_event_id: u64,
}

impl<S: EventStore> Node<S> {
    /// Load an existing log (if any) and rebuild the projection.
    pub fn new(store: S) -> Result<Self, NodeError> {
        let events = store.load_all().map_err(NodeError::Store)?;
        let projection = replay(&events).map_err(NodeError::Replay)?;
        let tail_hash = events.last().map(|e| e.self_hash()).unwrap_or_default();
        let last_event_id = events.last().map(|e| e.event_id).unwrap_or(0);
        Ok(Self {
            store,
            projection,
            pubkeys: HashMap::new(),
            tail_hash,
            last_event_id,
        })
    }

    /// Register an author so their signed events can be verified.
    pub fn register(&mut self, keypair: &Keypair) {
        self.pubkeys.insert(keypair.agent.clone(), keypair.public_key_hex());
    }

    /// Validate, append, and apply one event. Returns the updated projection.
    pub fn submit(&mut self, env: Envelope) -> Result<&Projection, NodeError> {
        // 1. hash chain (anti-reordering) + monotonic id
        if env.prev_hash != self.tail_hash {
            return Err(NodeError::InvalidChain {
                event_id: env.event_id,
            });
        }
        if env.event_id <= self.last_event_id {
            return Err(NodeError::InvalidChain {
                event_id: env.event_id,
            });
        }
        // 2. payload integrity
        if env.payload_hash != hash_bytes(env.payload.as_bytes()) {
            return Err(NodeError::InvalidPayload {
                event_id: env.event_id,
            });
        }
        // 3. author signature
        let key_hex = self
            .pubkeys
            .get(&env.authored_by)
            .ok_or(NodeError::UnknownAuthor {
                agent: env.authored_by.clone(),
            })?;
        env.verify_hex(key_hex)
            .map_err(|msg| NodeError::Signature {
                event_id: env.event_id,
                msg,
            })?;
        // 4. persist, then apply incrementally
        self.store.append(&env).map_err(NodeError::Store)?;
        apply_event(&mut self.projection, &env).map_err(NodeError::Replay)?;
        self.tail_hash = env.self_hash();
        self.last_event_id = env.event_id;
        Ok(&self.projection)
    }

    pub fn projection(&self) -> &Projection {
        &self.projection
    }

    pub fn imputed_values(&self) -> HashMap<DatasetId, f64> {
        self.projection.imputed_values()
    }

    pub fn event_count(&self) -> u64 {
        self.last_event_id
    }

    /// Hash of the most recent event (the log tail), needed to chain the
    /// next event.
    pub fn tail_hash(&self) -> String {
        self.tail_hash.clone()
    }
}
