//! Equivalence gates (SPEC §3.2) and corpus hash-pinning.
//!
//! The gate *sequencing* (upstream tests → golden replay → differential
//! fuzz → sanitizers) is orchestrated per-target; this crate owns the
//! pieces that must be trustworthy regardless of target: the pin
//! verifier that refuses to run on a tampered corpus, and the
//! per-target equivalence policy parser (byte-identical unless a target
//! declares an explicit FP tolerance).

pub mod orchestrate;
pub mod pin;
pub mod policy;
pub mod target;

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
    results
        .iter()
        .all(|(_, o)| matches!(o, GateOutcome::Passed | GateOutcome::Skipped { .. }))
        && results
            .iter()
            .any(|(_, o)| matches!(o, GateOutcome::Passed))
}

/// Inputs to one gate run: the candidate binary under test, the pristine
/// baseline to differ it against, and the differential-fuzz iteration
/// budget (from AcceptConfig, default 10_000).
#[derive(Debug, Clone, Copy)]
pub struct GateInputs<'a> {
    /// Patched binary (repo-root-relative): golden replay runs it, and it
    /// is the `{candidate}` side of differential fuzz.
    pub candidate_binary: &'a str,
    /// Pristine baseline binary (repo-root-relative), the `{baseline}` side
    /// of differential fuzz. `None` when the caller has no baseline to
    /// differ against — the standalone `just gates` flow builds only the
    /// working tree — and the fuzz gate is then skipped with that reason
    /// rather than silently comparing the candidate to itself. The accept
    /// path (`just verdict`) rebuilds a pristine baseline and passes it,
    /// and only a fuzz gate that actually Passed can mint an accept.
    pub baseline_binary: Option<&'a str>,
    pub fuzz_iters: u64,
}

/// Everything one gate run produced: the four layers in fixed order
/// (UpstreamTests, GoldenReplay, DifferentialFuzz, Sanitizers) plus the
/// fuzz counts and equivalence mode that verdict records in the ledger.
#[derive(Debug, Clone)]
pub struct GateReport {
    pub gates: Vec<(GateLayer, GateOutcome)>,
    /// Fuzz iterations actually executed (parsed from the FUZZ-RESULT
    /// line); 0 when the fuzz gate was skipped or never reported.
    pub fuzz_iters: u64,
    pub fuzz_divergence: bool,
    /// "byte-identical" | "fp-tolerance"
    pub equivalence_mode: String,
}

impl GateReport {
    pub fn outcome(&self, layer: GateLayer) -> Option<&GateOutcome> {
        self.gates.iter().find(|(l, _)| *l == layer).map(|(_, o)| o)
    }

    pub fn all_passed(&self) -> bool {
        all_passed(&self.gates)
    }
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
                GateOutcome::Failed {
                    detail: "stdout diverged".into(),
                },
            ),
        ];
        assert!(!all_passed(&results));
    }

    #[test]
    fn all_skipped_is_not_a_pass() {
        let results = vec![(
            GateLayer::Sanitizers,
            GateOutcome::Skipped {
                reason: "n/a".into(),
            },
        )];
        assert!(!all_passed(&results));
    }

    #[test]
    fn report_outcome_looks_up_by_layer() {
        let report = GateReport {
            gates: vec![
                (GateLayer::UpstreamTests, GateOutcome::Passed),
                (
                    GateLayer::GoldenReplay,
                    GateOutcome::Failed {
                        detail: "stdout diverged".into(),
                    },
                ),
            ],
            fuzz_iters: 0,
            fuzz_divergence: false,
            equivalence_mode: "byte-identical".into(),
        };
        assert_eq!(
            report.outcome(GateLayer::UpstreamTests),
            Some(&GateOutcome::Passed)
        );
        assert!(matches!(
            report.outcome(GateLayer::GoldenReplay),
            Some(GateOutcome::Failed { .. })
        ));
        assert_eq!(report.outcome(GateLayer::Sanitizers), None);
        assert!(!report.all_passed());
    }
}
