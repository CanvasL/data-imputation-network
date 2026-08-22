//! din-node — a deployable lightweight DIN node: an HTTP API over din-core.
//!
//! This is the "product" layer of the protocol: it makes concrete technology
//! choices (SQLite storage, axum HTTP) and exposes the protocol's core
//! operations (register agent, submit event, query imputed values, inspect
//! the lineage graph) over HTTP.

pub mod api;
pub mod state;

pub use api::router;
pub use state::AppState;
