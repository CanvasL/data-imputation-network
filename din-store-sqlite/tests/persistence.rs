//! Node + SqliteStore: events must survive reopening the store (durability),
//! and the projection must rebuild identically.

use din_core::event::{Envelope, EventType, Keypair};
use din_core::log::{ContributePayload, SourceRef, SubscribePayload};
use din_core::model::{AgentId, Dataset};
use din_core::node::Node;
use din_store_sqlite::SqliteStore;

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
fn persists_across_reopen() {
    let path = std::env::temp_dir().join(format!("din_test_{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let alice = Keypair::random(AgentId("alice".into()));
    let bob = Keypair::random(AgentId("bob".into()));
    let carol = Keypair::random(AgentId("carol".into()));

    let a = Dataset::new(alice.agent.clone(), "raw/v1", 1, b"alice-data");
    let b = Dataset::new(bob.agent.clone(), "raw/v1", 1, b"bob-data");
    let c = Dataset::new(carol.agent.clone(), "report/v1", 1, b"report");

    {
        let mut node = Node::new(SqliteStore::open(&path).unwrap()).unwrap();
        node.register(&alice);
        node.register(&bob);
        node.register(&carol);

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
        assert_eq!(node.event_count(), 3);
    } // node & store dropped here (releases the file)

    // Reopen from disk: projection must rebuild identically.
    let mut node2 = Node::new(SqliteStore::open(&path).unwrap()).unwrap();
    node2.register(&alice);
    node2.register(&bob);
    node2.register(&carol);
    assert_eq!(node2.event_count(), 3);

    let v = node2.imputed_values();
    assert!((v[&c.id] - 300.0).abs() < 1e-9, "V(C) = {}", v[&c.id]);
    assert!((v[&a.id] - 200.0).abs() < 1e-9, "V(A) = {}", v[&a.id]);
    assert!((v[&b.id] - 250.0).abs() < 1e-9, "V(B) = {}", v[&b.id]);

    let _ = std::fs::remove_file(&path);
}
