//! Event-log replay integration tests: build a signed event stream, replay
//! it into a projection, and verify the imputed values — including a
//! multi-parent (branching) lineage with attribution weights that sum to 1.

use std::collections::HashMap;

use din_core::event::{Envelope, EventType, Keypair};
use din_core::log::{
    replay, validate_chain, ContributePayload, ReplayError, SourceRef, SubscribePayload,
};
use din_core::model::{AgentId, Dataset};

/// Sign and append a Contribute event; returns (event, new chain hash).
fn contribute(
    id: u64,
    kp: &Keypair,
    prev: &str,
    owner: &str,
    schema: &str,
    content: &[u8],
) -> (Envelope, String) {
    let payload = serde_json::to_string(&ContributePayload {
        owner: owner.into(),
        schema: schema.into(),
        version: 1,
        content: content.to_vec(),
    })
    .unwrap();
    let mut env = Envelope::new(id, EventType::Contribute, kp.agent.clone(), payload, prev.into());
    env.sign(kp);
    let h = env.self_hash();
    (env, h)
}

/// Build the golden multi-parent stream:
///   A (alice) and B (bob) contribute raw data;
///   C (carol) subscribes to both, external revenue 300.
fn build_stream() -> (Vec<Envelope>, Dataset, Dataset, Dataset, Vec<Keypair>) {
    let alice = Keypair::random(AgentId("alice".into()));
    let bob = Keypair::random(AgentId("bob".into()));
    let carol = Keypair::random(AgentId("carol".into()));

    let a = Dataset::new(alice.agent.clone(), "raw/v1", 1, b"alice-data");
    let b = Dataset::new(bob.agent.clone(), "raw/v1", 1, b"bob-data");
    let c = Dataset::new(carol.agent.clone(), "report/v1", 1, b"report");

    let mut events = Vec::new();
    let mut prev = String::new();
    let (e1, h1) = contribute(1, &alice, &prev, "alice", "raw/v1", b"alice-data");
    events.push(e1);
    prev = h1;
    let (e2, h2) = contribute(2, &bob, &prev, "bob", "raw/v1", b"bob-data");
    events.push(e2);
    prev = h2;

    // C subscribes to A (price 100, alpha 0.5) and B (price 200, alpha 0.5)
    let payload = serde_json::to_string(&SubscribePayload {
        child: ContributePayload {
            owner: "carol".into(),
            schema: "report/v1".into(),
            version: 1,
            content: b"report".to_vec(),
        },
        sources: vec![
            SourceRef {
                parent: a.id.0.clone(),
                price: 100,
                alpha: 0.5,
            },
            SourceRef {
                parent: b.id.0.clone(),
                price: 200,
                alpha: 0.5,
            },
        ],
        external_revenue: 300,
    })
    .unwrap();
    let mut e3 = Envelope::new(3, EventType::Subscribe, carol.agent.clone(), payload, prev);
    e3.sign(&carol);
    events.push(e3);

    (events, a, b, c, vec![alice, bob, carol])
}

#[test]
fn replay_multi_parent_projection() {
    let (events, a, b, c, _keys) = build_stream();

    let proj = replay(&events).unwrap();
    let v = proj.imputed_values();

    // Hand-computed expectations with alpha = 0.5 each:
    //   V(C) = R_C = 300
    //   V(A) = 0.5 * (100 + 300) = 200
    //   V(B) = 0.5 * (200 + 300) = 250
    assert!((v[&c.id] - 300.0).abs() < 1e-9, "V(C) = {}", v[&c.id]);
    assert!((v[&a.id] - 200.0).abs() < 1e-9, "V(A) = {}", v[&a.id]);
    assert!((v[&b.id] - 250.0).abs() < 1e-9, "V(B) = {}", v[&b.id]);

    // Lineage edges present.
    assert_eq!(proj.graph.children(&a.id), vec![c.id.clone()]);
    assert_eq!(proj.graph.children(&b.id), vec![c.id.clone()]);
}

#[test]
fn replay_validates_signatures_with_registry() {
    let (events, _a, _b, _c, keys) = build_stream();
    let mut registry = HashMap::new();
    for kp in &keys {
        registry.insert(kp.agent.clone(), kp.public_key_hex());
    }
    // Correct registry => chain + signatures valid.
    assert!(validate_chain(&events, Some(&registry)).is_ok());

    // Tamper with a payload after signing => payload hash mismatch.
    let mut tampered = events.clone();
    tampered[2].payload = "tampered".into();
    assert!(matches!(
        replay(&tampered),
        Err(ReplayError::InvalidPayload { event_id: 3, .. })
    ));

    // Break the hash chain => reordering detected.
    let mut reordered = events.clone();
    reordered[2].prev_hash = "deadbeef".into();
    assert!(matches!(
        replay(&reordered),
        Err(ReplayError::InvalidHashChain { event_id: 3 })
    ));
}

#[test]
fn replay_rejects_non_normalized_alpha() {
    let (mut events, _a, _b, _c, keys) = build_stream();
    // Rewrite C's subscribe with alpha 0.5 + 0.6 = 1.1
    let mut e3 = events.pop().unwrap();
    let mut payload: SubscribePayload = serde_json::from_str(&e3.payload).unwrap();
    payload.sources[1].alpha = 0.6;
    e3.payload = serde_json::to_string(&payload).unwrap();
    e3.payload_hash = din_core::model::hash_bytes(e3.payload.as_bytes());
    e3.sign(&keys[2]);
    events.push(e3);

    assert!(matches!(
        replay(&events),
        Err(ReplayError::AlphaNotNormalized { event_id: 3, .. })
    ));
}
