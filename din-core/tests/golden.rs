//! Golden tests (PROTOCOL.md §7): a fixed event stream must yield fixed
//! imputed values in ANY compliant implementation. These are the
//! interoperability check-vectors for the DIN protocol.

use std::collections::HashMap;

use din_core::attribution::{attribute, AttributionRule, AttributionSplit};
use din_core::imputation::{impute, AlphaMap, PaymentMap, RevenueMap};
use din_core::model::{AgentId, Dataset, DatasetId, LineageGraph};

/// Build the golden DAG: A (raw) -> B (clean) -> C (report, external revenue).
fn golden_graph() -> (LineageGraph, DatasetId, DatasetId, DatasetId) {
    let a = Dataset::new(AgentId("alice".into()), "raw/v1", 1, b"alice-health-data");
    let b = Dataset::new(AgentId("bob".into()), "clean/v1", 1, b"alice-health-data-cleaned");
    let c = Dataset::new(AgentId("carol".into()), "report/v1", 1, b"health-report");

    let mut graph = LineageGraph::default();
    graph.add_edge(a.id.clone(), b.id.clone());
    graph.add_edge(b.id.clone(), c.id.clone());
    (graph, a.id, b.id, c.id)
}

#[test]
fn golden_imputation_values() {
    let (graph, a, b, c) = golden_graph();

    let payments: PaymentMap = HashMap::from([
        ((a.clone(), b.clone()), 100),
        ((b.clone(), c.clone()), 200),
    ]);
    let revenue: RevenueMap = HashMap::from([(c.clone(), 300)]);
    let alpha: AlphaMap = HashMap::from([
        ((a.clone(), b.clone()), 1.0),
        ((b.clone(), c.clone()), 1.0),
    ]);

    let v = impute(&graph, &revenue, &payments, &alpha);

    // Hand-computed expectations:
    //   V(C) = R_C = 300
    //   V(B) = α·(P_BC + V(C)) = 1·(200 + 300) = 500
    //   V(A) = α·(P_AB + V(B)) = 1·(100 + 500) = 600
    assert!((v[&c] - 300.0).abs() < 1e-9, "V(C) = {}", v[&c]);
    assert!((v[&b] - 500.0).abs() < 1e-9, "V(B) = {}", v[&b]);
    assert!((v[&a] - 600.0).abs() < 1e-9, "V(A) = {}", v[&a]);
}

#[test]
fn golden_attribution_split() {
    // mu = 0.10, lambda = 0.70 on amount 100:
    //   retained = 10, post = 90, curator = 90·0.3 = 27, upstream = 90·0.7 = 63
    let rule = AttributionRule::default();
    let split = attribute(100, &rule);
    assert_eq!(
        split,
        AttributionSplit {
            protocol_retained: 10,
            curator_margin: 27,
            upstream_pool: 63,
        }
    );
}

#[test]
fn dag_rejects_cycles() {
    let a = Dataset::new(AgentId("alice".into()), "x", 1, b"a");
    let b = Dataset::new(AgentId("bob".into()), "y", 1, b"b");

    let mut graph = LineageGraph::default();
    graph.add_edge(a.id.clone(), b.id.clone());

    // Adding b -> a would create a cycle; must be rejected.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        graph.add_edge(b.id.clone(), a.id.clone());
    }));
    assert!(result.is_err(), "cycle must be rejected");
}

#[test]
fn imputation_is_deterministic_across_orders() {
    // Same events, different insertion order must yield identical values.
    let a = Dataset::new(AgentId("alice".into()), "raw/v1", 1, b"alice-health-data");
    let b = Dataset::new(AgentId("bob".into()), "clean/v1", 1, b"alice-health-data-cleaned");
    let c = Dataset::new(AgentId("carol".into()), "report/v1", 1, b"health-report");

    let mut g1 = LineageGraph::default();
    g1.add_edge(a.id.clone(), b.id.clone());
    g1.add_edge(b.id.clone(), c.id.clone());

    let mut g2 = LineageGraph::default();
    g2.add_edge(b.id.clone(), c.id.clone());
    g2.add_edge(a.id.clone(), b.id.clone());

    let payments: PaymentMap = HashMap::from([
        ((a.id.clone(), b.id.clone()), 100),
        ((b.id.clone(), c.id.clone()), 200),
    ]);
    let revenue: RevenueMap = HashMap::from([(c.id.clone(), 300)]);
    let alpha: AlphaMap = HashMap::from([
        ((a.id.clone(), b.id.clone()), 1.0),
        ((b.id.clone(), c.id.clone()), 1.0),
    ]);

    let v1 = impute(&g1, &revenue, &payments, &alpha);
    let v2 = impute(&g2, &revenue, &payments, &alpha);
    assert_eq!(v1[&a.id], v2[&a.id]);
    assert_eq!(v1[&b.id], v2[&b.id]);
    assert_eq!(v1[&c.id], v2[&c.id]);
}
