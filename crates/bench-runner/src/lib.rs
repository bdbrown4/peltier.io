//! bench-runner core (SPEC §3.1, §7).
//!
//! The product is this crate: interleaved A/B measurement, bootstrap CIs
//! over the ratio of medians, threshold-gated accept decisions, and A/A
//! self-calibration. Pure statistics live here and are unit-tested;
//! process execution and `perf stat` integration live in `exec`.

pub mod config;
pub mod exec;
pub mod fingerprint;
pub mod schedule;
pub mod service;
pub mod stats;

use ledger::Verdict;

/// Decide a verdict from a speedup CI and the accept threshold.
///
/// Speedup is baseline/candidate (>1 means the candidate is faster).
/// Accepted only if the CI *lower bound* clears `1 + threshold` — the
/// point estimate is never enough (SPEC §3.1).
pub fn decide(speedup_ci: stats::RatioCi, threshold: f64) -> Verdict {
    if speedup_ci.lo >= 1.0 + threshold {
        Verdict::Accepted
    } else {
        Verdict::RejectedBench
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::RatioCi;

    #[test]
    fn accept_requires_ci_lower_bound_not_point_estimate() {
        // 10% median speedup but the CI dips below threshold: rejected.
        let noisy = RatioCi {
            median: 1.10,
            lo: 1.005,
            hi: 1.21,
        };
        assert_eq!(decide(noisy, 0.02), Verdict::RejectedBench);
        // 4% speedup with a tight CI: accepted.
        let tight = RatioCi {
            median: 1.04,
            lo: 1.025,
            hi: 1.055,
        };
        assert_eq!(decide(tight, 0.02), Verdict::Accepted);
    }
}
