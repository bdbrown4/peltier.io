//! Equivalence gates (SPEC §3.2) and corpus hash-pinning.
//!
//! The gate *sequencing* (upstream tests → golden replay → differential
//! fuzz → sanitizers) is orchestrated per-target; this crate owns the
//! pieces that must be trustworthy regardless of target: the pin
//! verifier that refuses to run on a tampered corpus, and the
//! per-target equivalence policy parser (byte-identical unless a target
//! declares an explicit FP tolerance).

pub mod pin;
pub mod target;
pub mod policy;

use serde::{Deserialize, Serialize};

/// Outcome of one gate layer. `Skipped` is only legal where the SPEC
/// allows it (e.g. TSan when no concurrency primitives changed) and the
/// reason is always recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateOutcome {
    Passed,
    Failed { detail: String },
    Skipped { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GateLayer {
    UpstreamTests,
    GoldenReplay,
    DifferentialFuzz,
    Sanitizers,
}

/// All layers must pass; any failure is an auto-reject (SPEC §8).
pub fn all_passed(results: &[(GateLayer, GateOutcome)]) -> bool {
    results.iter().all(|(_, o)| {
        matches!(o, GateOutcome::Passed | GateOutcome::Skipped { .. })
    }) && results
        .iter()
        .any(|(_, o)| matches!(o, GateOutcome::Passed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_failure_rejects() {
        let results = vec![
            (GateLayer::UpstreamTests, GateOutcome::Passed),
            (
                GateLayer::GoldenReplay,
                GateOutcome::Failed { detail: "stdout diverged".into() },
            ),
        ];
        assert!(!all_passed(&results));
    }

    #[test]
    fn all_skipped_is_not_a_pass() {
        let results = vec![(
            GateLayer::Sanitizers,
            GateOutcome::Skipped { reason: "n/a".into() },
        )];
        assert!(!all_passed(&results));
    }
}
