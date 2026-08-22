//! Payment attribution per PROTOCOL.md §3.1.
//!
//! A single payment is split into:
//! - protocol retention `mu` (non-recoverable; blocks self-dealing)
//! - curator margin `1 - lambda` (kept by the direct parent owner)
//! - upstream pool `lambda` (remitted along the lineage by weights `alpha`)

use serde::{Deserialize, Serialize};

use crate::model::{AgentId, Amount};

/// Attribution rule. Spec defaults: `mu = 0.10` (protocol),
/// `lambda = 0.70` (upstream), leaving `0.20` as curator margin.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AttributionRule {
    pub mu: f64,
    pub lambda: f64,
}

impl Default for AttributionRule {
    fn default() -> Self {
        AttributionRule {
            mu: 0.10,
            lambda: 0.70,
        }
    }
}

/// Result of splitting one payment. Always sums exactly back to `amount`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributionSplit {
    pub protocol_retained: Amount,
    pub curator_margin: Amount,
    pub upstream_pool: Amount,
}

/// Split a single payment into the three buckets.
pub fn attribute(amount: Amount, rule: &AttributionRule) -> AttributionSplit {
    let protocol_retained = (amount as f64 * rule.mu).round() as Amount;
    let post_retention = amount - protocol_retained;
    let curator_margin = (post_retention as f64 * (1.0 - rule.lambda)).round() as Amount;
    let upstream_pool = post_retention - curator_margin;
    AttributionSplit {
        protocol_retained,
        curator_margin,
        upstream_pool,
    }
}

/// Distribute the upstream pool to ancestors by weights.
/// Weights should sum to at most 1; rounding dust stays with the protocol.
pub fn distribute_upstream(
    pool: Amount,
    alpha: &[(AgentId, f64)],
) -> Vec<(AgentId, Amount)> {
    let mut out = Vec::with_capacity(alpha.len());
    let mut used: Amount = 0;
    for (agent, w) in alpha {
        let share = (*w * pool as f64).round() as Amount;
        used += share;
        out.push((agent.clone(), share));
    }
    let _dust = pool - used; // protocol keeps rounding dust
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_is_exact() {
        let rule = AttributionRule::default();
        for amount in [1, 7, 100, 1000, 999_999] {
            let s = attribute(amount, &rule);
            assert_eq!(
                s.protocol_retained + s.curator_margin + s.upstream_pool,
                amount,
                "split must reconstruct the original amount"
            );
        }
    }
}
