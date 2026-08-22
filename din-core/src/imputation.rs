//! Value imputation per PROTOCOL.md §3.2 and the paper's Prop 1.
//!
//! V(v) = R(v) + Σ_{child} α(v,child) · ( P(v,child) + V(child) )
//!
//! Computed in reverse topological order (leaves first) in O(|V| + |E|).
//! Because the lineage is a DAG, the system has a unique solution and any
//! implementation must agree on it (determinism requirement).

use std::collections::HashMap;

use crate::model::{Amount, DatasetId, LineageGraph};

/// Payments keyed by (parent, child).
pub type PaymentMap = HashMap<(DatasetId, DatasetId), Amount>;
/// External end-consumer revenue per dataset (PROTOCOL.md `R`).
pub type RevenueMap = HashMap<DatasetId, Amount>;
/// Attribution weights keyed by (parent, child), α ∈ [0,1].
pub type AlphaMap = HashMap<(DatasetId, DatasetId), f64>;

/// Compute the imputed value for every dataset in the graph.
pub fn impute(
    graph: &LineageGraph,
    revenue: &RevenueMap,
    payments: &PaymentMap,
    alpha: &AlphaMap,
) -> HashMap<DatasetId, f64> {
    let mut v: HashMap<DatasetId, f64> = HashMap::new();

    for node in graph.reverse_topological() {
        let mut downstream = 0.0f64;
        for child in graph.children(&node) {
            let a = alpha
                .get(&(node.clone(), child.clone()))
                .copied()
                .unwrap_or(0.0);
            let p = payments
                .get(&(node.clone(), child.clone()))
                .copied()
                .unwrap_or(0) as f64;
            let child_val = v.get(&child).copied().unwrap_or(0.0);
            downstream += a * (p + child_val);
        }
        let r = revenue.get(&node).copied().unwrap_or(0) as f64;
        v.insert(node, r + downstream);
    }

    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AgentId, Dataset};

    fn graph_a_b_c() -> (LineageGraph, DatasetId, DatasetId, DatasetId) {
        let a = Dataset::new(AgentId("alice".into()), "raw/v1", 1, b"alice-health");
        let b = Dataset::new(AgentId("bob".into()), "clean/v1", 1, b"alice-cleaned");
        let c = Dataset::new(AgentId("carol".into()), "report/v1", 1, b"report");
        let mut g = LineageGraph::default();
        g.add_edge(a.id.clone(), b.id.clone());
        g.add_edge(b.id.clone(), c.id.clone());
        (g, a.id, b.id, c.id)
    }

    #[test]
    fn single_chain_golden() {
        let (g, a, b, c) = graph_a_b_c();
        let payments: PaymentMap = HashMap::from([
            ((a.clone(), b.clone()), 100),
            ((b.clone(), c.clone()), 200),
        ]);
        let revenue: RevenueMap = HashMap::from([(c.clone(), 300)]);
        let alpha: AlphaMap = HashMap::from([
            ((a.clone(), b.clone()), 1.0),
            ((b.clone(), c.clone()), 1.0),
        ]);

        let v = impute(&g, &revenue, &payments, &alpha);
        assert!((v[&c] - 300.0).abs() < 1e-9);
        assert!((v[&b] - 500.0).abs() < 1e-9);
        assert!((v[&a] - 600.0).abs() < 1e-9);
    }

    #[test]
    fn internal_payments_circulate_without_external_payers() {
        // Paper Thm 1: without external revenue, internal payments still
        // propagate as "circulation value" (V(C)=0, V(B)=200, V(A)=300),
        // but they are pure zero-sum redistribution — no net value is
        // created. External payers are what anchor value.
        let (g, a, b, c) = graph_a_b_c();
        let payments: PaymentMap = HashMap::from([
            ((a.clone(), b.clone()), 100),
            ((b.clone(), c.clone()), 200),
        ]);
        let revenue: RevenueMap = HashMap::new(); // no external payers
        let alpha: AlphaMap = HashMap::from([
            ((a.clone(), b.clone()), 1.0),
            ((b.clone(), c.clone()), 1.0),
        ]);

        let v = impute(&g, &revenue, &payments, &alpha);
        assert_eq!(v[&c], 0.0);
        assert_eq!(v[&b], 200.0);
        assert_eq!(v[&a], 300.0);
    }
}
