//! DIN demo: build a small lineage A -> B -> C, compute imputed values,
//! and show the attribution split of a payment.

use std::collections::HashMap;

use din_core::attribution::{attribute, AttributionRule};
use din_core::imputation::{impute, AlphaMap, PaymentMap, RevenueMap};
use din_core::model::{AgentId, Dataset, LineageGraph};

fn main() {
    // A (raw health data) --bob cleans--> B --carol derives--> C (report with external revenue)
    let alice = AgentId("alice".into());
    let bob = AgentId("bob".into());
    let carol = AgentId("carol".into());

    let a = Dataset::new(alice.clone(), "raw/v1", 1, b"alice-health-data");
    let b = Dataset::new(bob.clone(), "clean/v1", 1, b"alice-health-data-cleaned");
    let c = Dataset::new(carol.clone(), "report/v1", 1, b"health-report");

    let mut graph = LineageGraph::default();
    graph.add_edge(a.id.clone(), b.id.clone());
    graph.add_edge(b.id.clone(), c.id.clone());

    let payments: PaymentMap = HashMap::from([
        ((a.id.clone(), b.id.clone()), 100),
        ((b.id.clone(), c.id.clone()), 200),
    ]);
    let revenue: RevenueMap = HashMap::from([(c.id.clone(), 300)]);
    let alpha: AlphaMap = HashMap::from([
        ((a.id.clone(), b.id.clone()), 1.0),
        ((b.id.clone(), c.id.clone()), 1.0),
    ]);

    let v = impute(&graph, &revenue, &payments, &alpha);

    println!("Imputed values (V):");
    println!("  {} (raw)   = {:.2}", a.id.0, v[&a.id]);
    println!("  {} (clean) = {:.2}", b.id.0, v[&b.id]);
    println!("  {} (report)= {:.2}", c.id.0, v[&c.id]);

    let rule = AttributionRule::default();
    let split = attribute(100, &rule);
    println!("\nAttribution of 100 units (mu={}, lambda={}):", rule.mu, rule.lambda);
    println!("  protocol retained : {}", split.protocol_retained);
    println!("  curator margin    : {}", split.curator_margin);
    println!("  upstream pool     : {}", split.upstream_pool);
    println!(
        "  total             : {}",
        split.protocol_retained + split.curator_margin + split.upstream_pool
    );
}
