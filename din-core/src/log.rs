//! Event-log replay (PROTOCOL.md §2 / §5): rebuild protocol state from an
//! append-only event stream, then compute imputed values as a pure
//! projection. The event stream is the single source of truth; nothing is
//! stored beyond what can be re-derived from it.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::event::{Envelope, EventType};
use crate::imputation::{impute, AlphaMap, PaymentMap, RevenueMap};
use crate::model::{AgentId, Amount, Dataset, DatasetId, LineageGraph, hash_bytes};

// ---------------------------------------------------------------------------
// Payload schemas (JSON-encoded in `Envelope.payload`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContributePayload {
    pub owner: String,
    pub schema: String,
    pub version: u32,
    pub content: Vec<u8>,
}

/// One source the child subscribes from: its dataset id, per-edge price,
/// and the attribution weight assigned to this parent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceRef {
    pub parent: String,
    pub price: Amount,
    pub alpha: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscribePayload {
    pub child: ContributePayload,
    pub sources: Vec<SourceRef>,
    /// External end-consumer revenue attributed to the child (usually 0
    /// except at leaves).
    pub external_revenue: Amount,
}

// ---------------------------------------------------------------------------
// Projection: the full state derivable from the event stream
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct Projection {
    pub datasets: HashMap<DatasetId, Dataset>,
    pub graph: LineageGraph,
    pub payments: PaymentMap,
    pub revenue: RevenueMap,
    pub alpha: AlphaMap,
}

impl Projection {
    /// Imputed values are a pure projection of the event stream
    /// (PROTOCOL.md §3.2).
    pub fn imputed_values(&self) -> HashMap<DatasetId, f64> {
        impute(&self.graph, &self.revenue, &self.payments, &self.alpha)
    }
}

#[derive(Debug, PartialEq)]
pub enum ReplayError {
    InvalidHashChain { event_id: u64 },
    InvalidPayload { event_id: u64, msg: String },
    InvalidSignature { event_id: u64, msg: String },
    AlphaNotNormalized { event_id: u64, sum: f64 },
}

/// Validate the hash chain (`prev_hash`) and payload hashes. Verifies
/// signatures only if `pubkeys` is provided (map of AgentId -> public key hex).
pub fn validate_chain(
    events: &[Envelope],
    pubkeys: Option<&HashMap<AgentId, String>>,
) -> Result<(), ReplayError> {
    let mut prev = String::new();
    for env in events {
        if env.prev_hash != prev {
            return Err(ReplayError::InvalidHashChain {
                event_id: env.event_id,
            });
        }
        if env.payload_hash != hash_bytes(env.payload.as_bytes()) {
            return Err(ReplayError::InvalidPayload {
                event_id: env.event_id,
                msg: "payload hash mismatch".into(),
            });
        }
        if let Some(keys) = pubkeys {
            let hex_key = keys
                .get(&env.authored_by)
                .ok_or(ReplayError::InvalidSignature {
                    event_id: env.event_id,
                    msg: "unknown author".into(),
                })?;
            let bytes = hex::decode(hex_key).map_err(|e| ReplayError::InvalidSignature {
                event_id: env.event_id,
                msg: e.to_string(),
            })?;
            let vk = ed25519_dalek::VerifyingKey::from_bytes(
                bytes.as_slice().try_into().map_err(|_| {
                    ReplayError::InvalidSignature {
                        event_id: env.event_id,
                        msg: "bad public key length".into(),
                    }
                })?,
            )
            .map_err(|e| ReplayError::InvalidSignature {
                event_id: env.event_id,
                msg: e.to_string(),
            })?;
            env.verify(&vk).map_err(|e| ReplayError::InvalidSignature {
                event_id: env.event_id,
                msg: e,
            })?;
        }
        prev = env.self_hash();
    }
    Ok(())
}

/// Apply a single (already-validated) event to the projection.
/// This is the incremental path used by `Node`; `replay` just loops over it.
pub fn apply_event(proj: &mut Projection, env: &Envelope) -> Result<(), ReplayError> {
    match env.event_type {
        EventType::Contribute => {
            let p: ContributePayload =
                serde_json::from_str(&env.payload).map_err(|e| ReplayError::InvalidPayload {
                    event_id: env.event_id,
                    msg: e.to_string(),
                })?;
            let ds = Dataset::new(AgentId(p.owner.clone()), &p.schema, p.version, &p.content);
            proj.datasets.insert(ds.id.clone(), ds.clone());
            proj.graph.add_node(ds.id.clone());
        }
        EventType::Subscribe => {
            let p: SubscribePayload =
                serde_json::from_str(&env.payload).map_err(|e| ReplayError::InvalidPayload {
                    event_id: env.event_id,
                    msg: e.to_string(),
                })?;
            if p.sources.is_empty() {
                return Err(ReplayError::InvalidPayload {
                    event_id: env.event_id,
                    msg: "subscribe without sources".into(),
                });
            }
            let alpha_sum: f64 = p.sources.iter().map(|s| s.alpha).sum();
            if (alpha_sum - 1.0).abs() > 1e-9 {
                return Err(ReplayError::AlphaNotNormalized {
                    event_id: env.event_id,
                    sum: alpha_sum,
                });
            }

            let child = Dataset::new(
                AgentId(p.child.owner.clone()),
                &p.child.schema,
                p.child.version,
                &p.child.content,
            );
            proj.datasets.insert(child.id.clone(), child.clone());

            for src in &p.sources {
                let parent = DatasetId(src.parent.clone());
                proj.graph.add_edge(parent.clone(), child.id.clone());
                proj.payments
                    .insert((parent.clone(), child.id.clone()), src.price);
                proj.alpha.insert((parent.clone(), child.id.clone()), src.alpha);
            }

            if p.external_revenue != 0 {
                proj.revenue.insert(child.id.clone(), p.external_revenue);
            }
        }
        EventType::Payment
        | EventType::Attribute
        | EventType::Challenge
        | EventType::Resolve => {
            // Not required for the value-imputation projection.
        }
    }
    Ok(())
}

/// Replay the event stream into a `Projection`.
pub fn replay(events: &[Envelope]) -> Result<Projection, ReplayError> {
    validate_chain(events, None)?;
    let mut proj = Projection::default();
    for env in events {
        apply_event(&mut proj, env)?;
    }
    Ok(proj)
}
