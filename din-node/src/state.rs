//! Shared application state: the node engine plus a registry of signing
//! identities. In this "managed node" model the node holds agent keys and
//! signs events on their behalf (centralized-first deployment).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use din_core::event::Keypair;
use din_core::model::AgentId;
use din_core::node::{Node, NodeError};
use din_store_sqlite::SqliteStore;

#[derive(Clone)]
pub struct AppState {
    pub node: Arc<Mutex<Node<SqliteStore>>>,
    pub keys: Arc<Mutex<HashMap<AgentId, Keypair>>>,
}

impl AppState {
    pub fn new(store: SqliteStore) -> Result<Self, NodeError> {
        Ok(Self {
            node: Arc::new(Mutex::new(Node::new(store)?)),
            keys: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}
