//! Data Imputation Network (DIN) — core protocol reference implementation.
//!
//! Implements the rules specified in `PROTOCOL.md`:
//! - content-addressed datasets & lineage DAG (`model`)
//! - payment attribution (`attribution`)
//! - value imputation (`imputation`)
//! - append-only event envelope (`event`)
//!
//! Determinism requirement: any independent implementation fed the same
//! event stream must compute the same imputed values. See `tests/golden.rs`.

pub mod attribution;
pub mod event;
pub mod imputation;
pub mod log;
pub mod model;
pub mod node;
pub mod store;

pub use attribution::{attribute, distribute_upstream, AttributionRule, AttributionSplit};
pub use event::{Envelope, EventType, Keypair};
pub use imputation::{impute, AlphaMap, PaymentMap, RevenueMap};
pub use log::{
    replay, validate_chain, ContributePayload, Projection, ReplayError, SourceRef,
    SubscribePayload,
};
pub use model::{
    AgentId, Dataset, DatasetId, EdgeId, LineageGraph, Payment, derive_edge_id, hash_bytes,
};
pub use node::{Node, NodeError};
pub use store::{EventStore, StoreError};
