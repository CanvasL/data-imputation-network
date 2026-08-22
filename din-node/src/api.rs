//! HTTP API handlers and router (axum).

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use din_core::event::{Envelope, EventType, Keypair};
use din_core::log::{ContributePayload, SourceRef, SubscribePayload};
use din_core::model::{AgentId, DatasetId};

use crate::state::AppState;

// ---------------------------------------------------------------------------
// API DTOs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct RegisterAgentReq {
    pub agent_id: String,
}

#[derive(Serialize)]
pub struct RegisterAgentResp {
    pub agent_id: String,
    pub public_key: String,
    pub secret_key: String,
}

/// User-friendly "contribute" payload (content as a UTF-8 string).
#[derive(Deserialize)]
pub struct ApiContribute {
    pub owner: String,
    pub schema: String,
    pub version: u32,
    pub content: String,
}

#[derive(Deserialize)]
pub struct ApiSource {
    pub parent: String,
    pub price: i64,
    pub alpha: f64,
}

#[derive(Deserialize)]
pub struct ApiSubscribe {
    pub child: ApiContribute,
    pub sources: Vec<ApiSource>,
    pub external_revenue: i64,
}

#[derive(Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum ApiEvent {
    Contribute { payload: ApiContribute },
    Subscribe { payload: ApiSubscribe },
}

impl ApiEvent {
    fn event_type(&self) -> EventType {
        match self {
            ApiEvent::Contribute { .. } => EventType::Contribute,
            ApiEvent::Subscribe { .. } => EventType::Subscribe,
        }
    }

    /// Convert to the protocol's canonical JSON payload string.
    fn to_core_payload(&self) -> Result<String, ApiError> {
        match self {
            ApiEvent::Contribute { payload } => serde_json::to_string(&ContributePayload {
                owner: payload.owner.clone(),
                schema: payload.schema.clone(),
                version: payload.version,
                content: payload.content.as_bytes().to_vec(),
            })
            .map_err(|e| ApiError::bad_request(e.to_string())),
            ApiEvent::Subscribe { payload } => serde_json::to_string(&SubscribePayload {
                child: ContributePayload {
                    owner: payload.child.owner.clone(),
                    schema: payload.child.schema.clone(),
                    version: payload.child.version,
                    content: payload.child.content.as_bytes().to_vec(),
                },
                sources: payload
                    .sources
                    .iter()
                    .map(|s| SourceRef {
                        parent: s.parent.clone(),
                        price: s.price,
                        alpha: s.alpha,
                    })
                    .collect(),
                external_revenue: payload.external_revenue,
            })
            .map_err(|e| ApiError::bad_request(e.to_string())),
        }
    }
}

#[derive(Deserialize)]
pub struct SubmitEventReq {
    pub agent_id: String,
    #[serde(flatten)]
    pub event: ApiEvent,
}

#[derive(Serialize)]
pub struct SubmitEventResp {
    pub event_id: u64,
    pub accepted: bool,
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

pub struct ApiError {
    status: StatusCode,
    msg: String,
}

impl ApiError {
    fn new(status: StatusCode, msg: impl Into<String>) -> Self {
        Self {
            status,
            msg: msg.into(),
        }
    }
    fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, msg)
    }
    fn not_found(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, msg)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({ "error": self.msg }))).into_response()
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn register_agent(
    State(state): State<AppState>,
    Json(req): Json<RegisterAgentReq>,
) -> impl IntoResponse {
    let agent = AgentId(req.agent_id.clone());
    let kp = Keypair::random(agent.clone());
    let public_key = kp.public_key_hex();
    let secret_key = kp.secret_key_hex();

    {
        let mut node = state.node.lock().unwrap();
        node.register(&kp);
    }
    state.keys.lock().unwrap().insert(agent, kp);

    Json(RegisterAgentResp {
        agent_id: req.agent_id,
        public_key,
        secret_key,
    })
}

async fn submit_event(
    State(state): State<AppState>,
    Json(req): Json<SubmitEventReq>,
) -> Result<Json<SubmitEventResp>, ApiError> {
    let agent = AgentId(req.agent_id.clone());
    let kp = state
        .keys
        .lock()
        .unwrap()
        .get(&agent)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("agent not registered: {}", req.agent_id)))?;

    let event_type = req.event.event_type();
    let core_payload = req.event.to_core_payload()?;

    let mut node = state.node.lock().unwrap();
    let event_id = node.event_count() + 1;
    let prev_hash = node.tail_hash();

    let mut env = Envelope::new(event_id, event_type, agent, core_payload, prev_hash);
    env.sign(&kp);
    node.submit(env).map_err(|e| ApiError::bad_request(e.to_string()))?;

    Ok(Json(SubmitEventResp {
        event_id,
        accepted: true,
    }))
}

async fn list_datasets(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    let values = node.imputed_values();
    let proj = node.projection();
    let datasets: Vec<serde_json::Value> = proj
        .datasets
        .iter()
        .map(|(id, ds)| {
            serde_json::json!({
                "id": id.0,
                "owner": ds.owner.0,
                "schema": ds.schema,
                "version": ds.version,
                "value": values.get(id).copied().unwrap_or(0.0),
            })
        })
        .collect();
    Json(serde_json::json!({ "datasets": datasets }))
}

async fn dataset_value(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let node = state.node.lock().unwrap();
    let values = node.imputed_values();
    let key = DatasetId(id.clone());
    let value = values
        .get(&key)
        .copied()
        .ok_or_else(|| ApiError::not_found("dataset not found"))?;
    Ok(Json(serde_json::json!({ "dataset_id": id, "value": value })))
}

async fn graph(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    let proj = node.projection();
    let nodes: Vec<serde_json::Value> = proj
        .datasets
        .keys()
        .map(|id| serde_json::json!({ "id": id.0 }))
        .collect();
    let edges: Vec<serde_json::Value> = proj
        .graph
        .edges()
        .iter()
        .map(|(p, c)| {
            serde_json::json!({
                "parent": p.0,
                "child": c.0,
                "price": proj.payments.get(&(p.clone(), c.clone())).copied().unwrap_or(0),
                "alpha": proj.alpha.get(&(p.clone(), c.clone())).copied().unwrap_or(0.0),
            })
        })
        .collect();
    Json(serde_json::json!({ "nodes": nodes, "edges": edges }))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/agents", post(register_agent))
        .route("/events", post(submit_event))
        .route("/datasets", get(list_datasets))
        .route("/datasets/:id/value", get(dataset_value))
        .route("/graph", get(graph))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state)
}
