//! In-process HTTP smoke tests for din-node (tower ServiceExt::oneshot).
//! Full flow: register agents -> contribute -> subscribe -> query values.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use tower::ServiceExt;

use din_core::model::{AgentId, Dataset};
use din_node::{AppState, router};
use din_store_sqlite::SqliteStore;

async fn app() -> Router {
    let store = SqliteStore::open_in_memory().unwrap();
    let state = AppState::new(store).unwrap();
    router(state)
}

async fn body_json(res: axum::response::Response) -> serde_json::Value {
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes).to_string();
    if bytes.is_empty() {
        panic!("empty body, status {status}");
    }
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse failed: {e}, body={text}, status={status}"))
}

async fn post_json(app: &Router, uri: &str, body: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn health_ok() {
    let app = app().await;
    let res = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn full_flow_end_to_end() {
    let app = app().await;

    // The dataset ids are content-derived; compute the expected ids.
    let a = Dataset::new(AgentId("alice".into()), "raw/v1", 1, b"alice-data");
    let b = Dataset::new(AgentId("bob".into()), "raw/v1", 1, b"bob-data");
    let c = Dataset::new(AgentId("carol".into()), "report/v1", 1, b"report");

    // register agents
    for id in ["alice", "bob", "carol"] {
        let res = post_json(&app, "/agents", &format!(r#"{{"agent_id":"{id}"}}"#)).await;
        assert_eq!(res.status(), StatusCode::OK, "register {id}");
        let _ = res.into_body().collect().await.unwrap();
    }

    // alice contributes
    let contribute = r#"{"agent_id":"alice","event_type":"contribute","payload":{"owner":"alice","schema":"raw/v1","version":1,"content":"alice-data"}}"#;
    let res = post_json(&app, "/events", contribute).await;
    assert_eq!(res.status(), StatusCode::OK, "alice contribute");
    let _ = res.into_body().collect().await.unwrap();

    // bob contributes
    let contribute2 = r#"{"agent_id":"bob","event_type":"contribute","payload":{"owner":"bob","schema":"raw/v1","version":1,"content":"bob-data"}}"#;
    let res = post_json(&app, "/events", contribute2).await;
    assert_eq!(res.status(), StatusCode::OK, "bob contribute");
    let _ = res.into_body().collect().await.unwrap();

    // carol subscribes to a & b
    let subscribe = format!(
        r#"{{"agent_id":"carol","event_type":"subscribe","payload":{{"child":{{"owner":"carol","schema":"report/v1","version":1,"content":"report"}},"sources":[{{"parent":"{}","price":100,"alpha":0.5}},{{"parent":"{}","price":200,"alpha":0.5}}],"external_revenue":300}}}}"#,
        a.id.0, b.id.0
    );
    let res = post_json(&app, "/events", &subscribe).await;
    assert_eq!(res.status(), StatusCode::OK, "carol subscribe");
    let _ = res.into_body().collect().await.unwrap();

    // values: C=300, A=0.5*(100+300)=200, B=0.5*(200+300)=250
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/datasets/{}/value", c.id.0))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let j = body_json(res).await;
    assert!((j["value"].as_f64().unwrap() - 300.0).abs() < 1e-9, "{j}");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/datasets/{}/value", a.id.0))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let j = body_json(res).await;
    assert!((j["value"].as_f64().unwrap() - 200.0).abs() < 1e-9, "{j}");

    // graph has 2 edges
    let res = app
        .clone()
        .oneshot(Request::builder().uri("/graph").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let j = body_json(res).await;
    assert_eq!(j["edges"].as_array().unwrap().len(), 2, "{j}");
    assert_eq!(j["nodes"].as_array().unwrap().len(), 3, "{j}");

    // /datasets lists 3 datasets
    let res = app
        .clone()
        .oneshot(Request::builder().uri("/datasets").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let j = body_json(res).await;
    assert_eq!(j["datasets"].as_array().unwrap().len(), 3, "{j}");
}

#[tokio::test]
async fn unregistered_agent_rejected() {
    let app = app().await;
    // nobody registered
    let contribute = r#"{"agent_id":"mallory","event_type":"contribute","payload":{"owner":"mallory","schema":"raw/v1","version":1,"content":"x"}}"#;
    let res = post_json(&app, "/events", contribute).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_dataset_value_not_found() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/datasets/deadbeef/value")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
