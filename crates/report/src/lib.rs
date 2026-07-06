//! ROI methodology (SPEC §3.6, §9): what did the stopwatch say, and what
//! does that cost or buy? Every figure carries its CI; the caveats print
//! on the report, not in an appendix.

pub mod pricing;

/// Cores returned to the fleet by a throughput speedup:
/// `fleet_cores × (1 − 1/speedup)`.
pub fn cores_saved(fleet_cores: f64, speedup: f64) -> f64 {
    assert!(speedup > 0.0, "speedup must be positive");
    fleet_cores * (1.0 - 1.0 / speedup)
}

/// Annual dollars from saved cores at a $/core-hour rate.
pub fn dollars_per_year(cores: f64, dollars_per_core_hour: f64, hours_per_year: f64) -> f64 {
    cores * dollars_per_core_hour * hours_per_year
}

/// ROI range induced by a speedup CI — the report quotes the interval,
/// never just the point estimate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoiRange {
    pub cores_lo: f64,
    pub cores_median: f64,
    pub cores_hi: f64,
    pub dollars_lo: f64,
    pub dollars_median: f64,
    pub dollars_hi: f64,
}

pub fn roi_from_speedup_ci(
    fleet_cores: f64,
    speedup_median: f64,
    speedup_ci: (f64, f64),
    dollars_per_core_hour: f64,
    hours_per_year: f64,
) -> RoiRange {
    let (lo, hi) = speedup_ci;
    let (c_lo, c_med, c_hi) = (
        cores_saved(fleet_cores, lo),
        cores_saved(fleet_cores, speedup_median),
        cores_saved(fleet_cores, hi),
    );
    RoiRange {
        cores_lo: c_lo,
        cores_median: c_med,
        cores_hi: c_hi,
        dollars_lo: dollars_per_year(c_lo, dollars_per_core_hour, hours_per_year),
        dollars_median: dollars_per_year(c_med, dollars_per_core_hour, hours_per_year),
        dollars_hi: dollars_per_year(c_hi, dollars_per_core_hour, hours_per_year),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cores_saved_formula() {
        // 2x speedup halves the fleet: 100 cores → 50 saved.
        assert!((cores_saved(100.0, 2.0) - 50.0).abs() < 1e-12);
        // 1.25x on 1000 cores: 1000 × (1 − 0.8) = 200.
        assert!((cores_saved(1000.0, 1.25) - 200.0).abs() < 1e-9);
        // No speedup, no savings.
        assert_eq!(cores_saved(100.0, 1.0), 0.0);
    }

    #[test]
    fn roi_range_is_ordered() {
        let r = roi_from_speedup_ci(1000.0, 1.10, (1.05, 1.16), 0.04, 8760.0);
        assert!(r.cores_lo < r.cores_median && r.cores_median < r.cores_hi);
        assert!(r.dollars_lo < r.dollars_median && r.dollars_median < r.dollars_hi);
    }
}
