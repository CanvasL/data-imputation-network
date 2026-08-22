//! Data model: content-addressed datasets, lineage DAG, payments.
//!
//! Per PROTOCOL.md §1, dataset IDs must be content-addressed so that
//! "same content = same id", preventing re-publishing the same data
//! under a new face.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Amounts are integer micro-units of the unit of account (e.g., 1e-6),
/// so that settlement never relies on floating point.
pub type Amount = i64;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DatasetId(pub String);

impl std::fmt::Display for DatasetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EdgeId(pub String);

/// SHA-256 hex digest.
pub fn hash_bytes(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

/// Content-addressed dataset. `id = hash(state_hash, schema, version)`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dataset {
    pub id: DatasetId,
    pub owner: AgentId,
    pub schema: String,
    pub version: u32,
    pub state_hash: String,
}

impl Dataset {
    pub fn new(owner: AgentId, schema: &str, version: u32, content: &[u8]) -> Self {
        let state_hash = hash_bytes(content);
        let id = DatasetId(hash_bytes(format!("{state_hash}|{schema}|{version}").as_bytes()));
        Dataset {
            id,
            owner,
            schema: schema.to_string(),
            version,
            state_hash,
        }
    }
}

/// A subscription payment attached to a lineage edge.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Payment {
    pub edge: EdgeId,
    pub amount: Amount,
    pub paid_by: AgentId,
    pub received_by: AgentId,
}

/// Lineage DAG. Enforces the acyclic constraint on insert.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LineageGraph {
    nodes: BTreeSet<DatasetId>,
    children: HashMap<DatasetId, Vec<DatasetId>>,
    parents: HashMap<DatasetId, Vec<DatasetId>>,
}

impl LineageGraph {
    pub fn add_node(&mut self, id: DatasetId) {
        self.nodes.insert(id);
    }

    /// Add edge `parent -> child`. Panics (protocol-invalid) if it would
    /// create a cycle.
    pub fn add_edge(&mut self, parent: DatasetId, child: DatasetId) {
        assert!(
            !self.is_ancestor(&child, &parent),
            "DIN protocol violation: adding edge {parent}->{child} creates a cycle"
        );
        self.nodes.insert(parent.clone());
        self.nodes.insert(child.clone());
        self.children
            .entry(parent.clone())
            .or_default()
            .push(child.clone());
        self.parents.entry(child.clone()).or_default().push(parent.clone());
    }

    pub fn children(&self, v: &DatasetId) -> Vec<DatasetId> {
        self.children.get(v).cloned().unwrap_or_default()
    }

    pub fn parents(&self, v: &DatasetId) -> Vec<DatasetId> {
        self.parents.get(v).cloned().unwrap_or_default()
    }

    pub fn nodes(&self) -> &BTreeSet<DatasetId> {
        &self.nodes
    }

    /// All edges as (parent, child) pairs (for export / graph queries).
    pub fn edges(&self) -> Vec<(DatasetId, DatasetId)> {
        let mut out = Vec::new();
        for (parent, children) in &self.children {
            for child in children {
                out.push((parent.clone(), child.clone()));
            }
        }
        out
    }

    /// Is `node` an ancestor of `target` (can `node` reach `target`)?
    fn is_ancestor(&self, node: &DatasetId, target: &DatasetId) -> bool {
        let mut stack = vec![node.clone()];
        let mut visited = HashSet::new();
        while let Some(cur) = stack.pop() {
            if &cur == target {
                return true;
            }
            if visited.insert(cur.clone()) {
                for c in self.children(&cur) {
                    stack.push(c);
                }
            }
        }
        false
    }

    /// Reverse topological order: every child appears before its parent
    /// (leaves first). Used by the imputation recursion.
    pub fn reverse_topological(&self) -> Vec<DatasetId> {
        let mut visited = HashSet::new();
        let mut order = Vec::new();
        let nodes: Vec<DatasetId> = self.nodes.iter().cloned().collect();
        for n in nodes {
            if !visited.contains(&n) {
                self.dfs_post(&n, &mut visited, &mut order);
            }
        }
        order
    }

    fn dfs_post(
        &self,
        v: &DatasetId,
        visited: &mut HashSet<DatasetId>,
        order: &mut Vec<DatasetId>,
    ) {
        if visited.contains(v) {
            return;
        }
        visited.insert(v.clone());
        for c in self.children(v) {
            self.dfs_post(&c, visited, order);
        }
        order.push(v.clone());
    }
}

/// Edge id derived deterministically from its endpoints.
pub fn derive_edge_id(parent: &DatasetId, child: &DatasetId) -> EdgeId {
    EdgeId(hash_bytes(format!("{}|{}", parent.0, child.0).as_bytes()))
}
