//! Interleaved run scheduling (SPEC §3.1): baseline and candidate run
//! ABABAB…, never sequential blocks, so thermal and background drift
//! land on both sides equally.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Baseline,
    Candidate,
}

/// ABAB… schedule with `runs_per_side` measured runs on each side,
/// preceded by `warmup` unmeasured warm-up entries (alternating too, so
/// both binaries reach steady state).
pub fn interleaved(runs_per_side: usize, warmup: usize) -> Vec<(Side, bool)> {
    let mut plan = Vec::with_capacity(2 * (runs_per_side + warmup));
    for i in 0..(runs_per_side + warmup) {
        let measured = i >= warmup;
        plan.push((Side::Baseline, measured));
        plan.push((Side::Candidate, measured));
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strictly_alternating_and_counts_match() {
        let plan = interleaved(30, 3);
        assert_eq!(plan.len(), 66);
        for pair in plan.chunks(2) {
            assert_eq!(pair[0].0, Side::Baseline);
            assert_eq!(pair[1].0, Side::Candidate);
        }
        let measured_baseline = plan
            .iter()
            .filter(|(s, m)| *s == Side::Baseline && *m)
            .count();
        assert_eq!(measured_baseline, 30);
        assert!(!plan[0].1 && !plan[5].1, "warm-ups are unmeasured");
        assert!(plan[6].1, "runs after warm-up are measured");
    }
}
