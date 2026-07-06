//! Ratios with bootstrap CIs; never means without spread; never a single
//! run, ever (SPEC §7).

/// Bootstrap confidence interval over the ratio of medians
/// (baseline / candidate).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RatioCi {
    pub median: f64,
    pub lo: f64,
    pub hi: f64,
}

pub fn median(xs: &[f64]) -> f64 {
    assert!(!xs.is_empty(), "median of empty sample");
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).expect("NaN in sample"));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// Deterministic xorshift64* PRNG. Seeded explicitly so calibration runs
/// are reproducible; no wall-clock entropy anywhere in the stats path.
struct XorShift64(u64);

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn index(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Percentile-method bootstrap CI of median(baseline)/median(candidate).
///
/// `confidence` is e.g. 0.95. Both samples are resampled independently
/// with replacement `iters` times.
pub fn bootstrap_ratio_ci(
    baseline: &[f64],
    candidate: &[f64],
    iters: usize,
    confidence: f64,
    seed: u64,
) -> RatioCi {
    assert!(baseline.len() >= 2 && candidate.len() >= 2, "need >=2 runs per side");
    assert!((0.0..1.0).contains(&confidence));
    let mut rng = XorShift64::new(seed);
    let mut ratios = Vec::with_capacity(iters);
    let mut b_resample = vec![0.0; baseline.len()];
    let mut c_resample = vec![0.0; candidate.len()];
    for _ in 0..iters {
        for slot in b_resample.iter_mut() {
            *slot = baseline[rng.index(baseline.len())];
        }
        for slot in c_resample.iter_mut() {
            *slot = candidate[rng.index(candidate.len())];
        }
        ratios.push(median(&b_resample) / median(&c_resample));
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).expect("NaN ratio"));
    let alpha = (1.0 - confidence) / 2.0;
    let lo_idx = ((iters as f64) * alpha).floor() as usize;
    let hi_idx = (((iters as f64) * (1.0 - alpha)).ceil() as usize).min(iters - 1);
    RatioCi {
        median: median(baseline) / median(candidate),
        lo: ratios[lo_idx],
        hi: ratios[hi_idx],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_odd_and_even() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), 2.5);
    }

    #[test]
    fn aa_ci_straddles_one() {
        // Same distribution both sides: the CI must contain 1.0.
        let sample: Vec<f64> = (0..40).map(|i| 1.0 + 0.01 * ((i * 7) % 11) as f64).collect();
        let ci = bootstrap_ratio_ci(&sample, &sample, 2000, 0.95, 42);
        assert!(ci.lo <= 1.0 && ci.hi >= 1.0, "A/A CI must straddle 1.0, got {ci:?}");
    }

    #[test]
    fn detects_injected_regression() {
        // Candidate 5% slower with mild noise: speedup CI entirely < 1.
        let base: Vec<f64> = (0..40).map(|i| 1.0 + 0.002 * ((i * 3) % 7) as f64).collect();
        let cand: Vec<f64> = base.iter().map(|t| t * 1.05).collect();
        let ci = bootstrap_ratio_ci(&base, &cand, 2000, 0.95, 42);
        assert!(ci.hi < 1.0, "5% regression must be detected, got {ci:?}");
    }

    #[test]
    fn deterministic_for_fixed_seed() {
        let a = vec![1.0, 1.1, 1.05, 0.98, 1.02, 1.07];
        let b = vec![0.9, 0.95, 0.93, 0.91, 0.97, 0.92];
        let x = bootstrap_ratio_ci(&a, &b, 500, 0.95, 7);
        let y = bootstrap_ratio_ci(&a, &b, 500, 0.95, 7);
        assert_eq!(x, y);
    }
}
