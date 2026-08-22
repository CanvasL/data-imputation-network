//! Node + MemoryStore end-to-end: submit signed events incrementally,
//! verify rejection paths.

use din_core::event::{Envelope, EventType, Keypair};
use din_core::log::{ContributePayload, SourceRef, SubscribePayload};
use din_core::model::{AgentId, Dataset};
use din_core::node::Node;
use din_store_memory::MemoryStore;

fn contribute_payload(owner: &str, schema: &str, content: &[u8]) -> String {
    serde_json::to_string(&ContributePayload {
        owner: owner.into(),
        schema: schema.into(),
        version: 1,
        content: content.to_vec(),
    })
    .unwrap()
}

#[test]
fn node_incremental_flow() {
    let alice = Keypair::random(AgentId("alice".into()));
    let bob = Keypair::random(AgentId("bob".into()));
    let carol = Keypair::random(AgentId("carol".into()));

    let mut node = Node::new(MemoryStore::new()).unwrap();
    node.register(&alice);
    node.register(&bob);
    node.register(&carol);

    let a = Dataset::new(alice.agent.clone(), "raw/v1", 1, b"alice-data");
    let b = Dataset::new(bob.agent.clone(), "raw/v1", 1, b"bob-data");
    let c = Dataset::new(carol.agent.clone(), "report/v1", 1, b"report");

    // event 1: alice contributes
    let mut e1 = Envelope::new(
        1,
        EventType::Contribute,
        alice.agent.clone(),
        contribute_payload("alice", "raw/v1", b"alice-data"),
        String::new(),
    );
    let h1 = e1.self_hash();
    e1.sign(&alice);
    node.submit(e1).unwrap();

    // event 2: bob contributes
    let mut e2 = Envelope::new(
        2,
        EventType::Contribute,
        bob.agent.clone(),
        contribute_payload("bob", "raw/v1", b"bob-data"),
        h1,
    );
    let h2 = e2.self_hash();
    e2.sign(&bob);
    node.submit(e2).unwrap();

    // event 3: carol subscribes to a & b (external revenue 300)
    let payload = serde_json::to_string(&SubscribePayload {
        child: ContributePayload {
            owner: "carol".into(),
            schema: "report/v1".into(),
            version: 1,
            content: b"report".to_vec(),
        },
        sources: vec![
            SourceRef { parent: a.id.0.clone(), price: 100, alpha: 0.5 },
            SourceRef { parent: b.id.0.clone(), price: 200, alpha: 0.5 },
        ],
        external_revenue: 300,
    })
    .unwrap();
    let mut e3 = Envelope::new(3, EventType::Subscribe, carol.agent.clone(), payload, h2);
    e3.sign(&carol);
    node.submit(e3).unwrap();

    let v = node.imputed_values();
    assert!((v[&c.id] - 300.0).abs() < 1e-9, "V(C) = {}", v[&c.id]);
    assert!((v[&a.id] - 200.0).abs() < 1e-9, "V(A) = {}", v[&a.id]);
    assert!((v[&b.id] - 250.0).abs() < 1e-9, "V(B) = {}", v[&b.id]);
    assert_eq!(node.event_count(), 3);
}

#[test]
fn node_rejects_bad_chain() {
    let alice = Keypair::random(AgentId("alice".into()));
    let mut node = Node::new(MemoryStore::new()).unwrap();
    node.register(&alice);

    // Wrong prev_hash for the first event (must be empty).
    let mut e1 = Envelope::new(
        1,
        EventType::Contribute,
        alice.agent.clone(),
        contribute_payload("alice", "raw/v1", b"alice-data"),
        "not-the-genesis".into(),
    );
    e1.sign(&alice);
    assert!(node.submit(e1).is_err());
}

#[test]
fn node_rejects_unknown_author() {
    let alice = Keypair::random(AgentId("alice".into()));
    let mallory = Keypair::random(AgentId("mallory".into()));
    let mut node = Node::new(MemoryStore::new()).unwrap();
    // Only alice is registered; mallory is not.
    node.register(&alice);

    let mut e1 = Envelope::new(
        1,
        EventType::Contribute,
        mallory.agent.clone(),
        contribute_payload("mallory", "raw/v1", b"m-data"),
        String::new(),
    );
    e1.sign(&mallory);
    assert!(node.submit(e1).is_err());
}
