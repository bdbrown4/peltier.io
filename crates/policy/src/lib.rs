//! Learned class-selection policy (SPEC §13 research fork).
//!
//! The append-only ledger is a growing dataset of `(playbook_class,
//! target) -> verdict`. This turns that history into a *prior*: given a
//! target, rank the playbook classes by how likely each is to yield a
//! shippable win — learned from evidence, not the fixed cheapest-first
//! order alone.
//!
//! The ranking statistic is the **Wilson score lower bound** of the
//! win-rate, not the point estimate. This is the same discipline the rest
//! of the project uses (quote the CI lower bound, never the median): a
//! class that won 1/1 does NOT outrank one that won 8/10, because its
//! interval is wide. With no data a class scores 0 and falls back to the
//! cheapest-first prior. As the ledger grows, the estimates sharpen — the
//! policy is honest about its own uncertainty by construction.

/// Wilson score interval lower bound for `wins`/`n` at z (1.96 = 95%).
/// Returns 0.0 for n == 0 (no evidence).
pub fn wilson_lower_bound(wins: u64, n: u64, z: f64) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let n = n as f64;
    let p = wins as f64 / n;
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let center = p + z2 / (2.0 * n);
    let margin = z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    ((center - margin) / denom).max(0.0)
}

/// Aggregated evidence for one playbook class.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassStat {
    pub class: u8,
    pub attempts: u64,
    /// Shippable wins: `accepted` AND sanitizers-clean.
    pub wins: u64,
    /// Real wins held for a human ruling (needs-human-review) — counted
    /// separately, since they are neither clean auto-wins nor failures.
    pub held: u64,
    pub win_rate: f64,
    pub wilson_lb: f64,
}

impl ClassStat {
    /// A short human note on the evidence state.
    pub fn note(&self) -> &'static str {
        if self.attempts == 0 {
            "untried — cheapest-first prior only"
        } else if self.wins > 0 {
            "proven winner"
        } else if self.held > 0 {
            "held for human review (measured win, tier-gated)"
        } else {
            "evidence against — tried, no shippable win"
        }
    }
}

/// Build per-class stats (classes 1..=7) from ledger outcome rows,
/// optionally filtered to one target. Each outcome is
/// `(class, target, verdict, sanitizers_clean)`.
pub fn class_stats(
    outcomes: &[(u8, String, String, bool)],
    target: Option<&str>,
    z: f64,
) -> Vec<ClassStat> {
    (1u8..=7)
        .map(|class| {
            let rows = outcomes
                .iter()
                .filter(|(c, t, _, _)| *c == class && target.is_none_or(|want| t == want));
            let (mut attempts, mut wins, mut held) = (0u64, 0u64, 0u64);
            for (_, _, verdict, san) in rows {
                attempts += 1;
                if verdict == "accepted" && *san {
                    wins += 1;
                } else if verdict == "needs-human-review" {
                    held += 1;
                }
            }
            let win_rate = if attempts > 0 {
                wins as f64 / attempts as f64
            } else {
                0.0
            };
            ClassStat {
                class,
                attempts,
                wins,
                held,
                win_rate,
                wilson_lb: wilson_lower_bound(wins, attempts, z),
            }
        })
        .collect()
}

/// Rank classes for selection. Primary key: Wilson lower bound (evidence
/// of success), descending. Ties (notably all the zero-evidence classes)
/// break by: fewer failed attempts first (prefer untried over
/// proven-failure), then cheaper class first (the cost prior).
pub fn rank(mut stats: Vec<ClassStat>) -> Vec<ClassStat> {
    stats.sort_by(|a, b| {
        b.wilson_lb
            .partial_cmp(&a.wilson_lb)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                // failures = attempts without a win or a hold
                let fa = a.attempts - a.wins - a.held;
                let fb = b.attempts - b.wins - b.held;
                fa.cmp(&fb)
            })
            .then_with(|| a.class.cmp(&b.class))
    });
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wilson_is_zero_without_evidence() {
        assert_eq!(wilson_lower_bound(0, 0, 1.96), 0.0);
    }

    #[test]
    fn wilson_penalizes_small_samples() {
        // 1/1 must NOT outrank 8/10 — the small sample's interval is wide.
        let one_of_one = wilson_lower_bound(1, 1, 1.96);
        let eight_of_ten = wilson_lower_bound(8, 10, 1.96);
        assert!(
            one_of_one < eight_of_ten,
            "1/1 ({one_of_one:.3}) should rank below 8/10 ({eight_of_ten:.3})"
        );
    }

    #[test]
    fn wilson_monotonic_in_evidence() {
        // Same rate, more evidence → higher lower bound.
        assert!(wilson_lower_bound(50, 100, 1.96) > wilson_lower_bound(5, 10, 1.96));
    }

    #[test]
    fn rank_prefers_proven_then_untried_then_failed() {
        let outcomes = vec![
            (5u8, "t".into(), "accepted".into(), true),
            (5u8, "t".into(), "accepted".into(), true),
            (5u8, "t".into(), "rejected-bench".into(), false),
            (6u8, "t".into(), "rejected-bench".into(), false), // evidence against
                                                               // class 7 untried
        ];
        let ranked = rank(class_stats(&outcomes, None, 1.96));
        assert_eq!(ranked[0].class, 5, "proven class 5 ranks first");
        // untried class (7) must rank above the tried-and-failed class 6
        let pos7 = ranked.iter().position(|s| s.class == 7).unwrap();
        let pos6 = ranked.iter().position(|s| s.class == 6).unwrap();
        assert!(pos7 < pos6, "untried class 7 outranks failed class 6");
    }
}
