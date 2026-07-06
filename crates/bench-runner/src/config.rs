//! Accept-threshold configuration, loaded from `config/accept.toml`.

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptConfig {
    /// CI lower bound must clear 1 + threshold (default 2%).
    pub threshold: f64,
    /// Measured runs per side (SPEC minimum: 30).
    pub runs_per_side: usize,
    /// Unmeasured warm-up runs per side, discarded.
    pub warmup_runs: usize,
    /// Bootstrap resample count.
    pub bootstrap_iters: usize,
    /// Confidence level for the ratio CI, e.g. 0.95.
    pub confidence: f64,
    /// PRNG seed for the bootstrap (reproducible calibration).
    pub bootstrap_seed: u64,
    /// Machine-specific command prefix for CPU pinning (e.g.
    /// "taskset -c 2"); empty disables pinning. Applied by verdict to
    /// both sides identically.
    #[serde(default)]
    pub pin_prefix: String,
}

impl Default for AcceptConfig {
    fn default() -> Self {
        Self {
            threshold: 0.02,
            runs_per_side: 30,
            warmup_runs: 3,
            bootstrap_iters: 10_000,
            confidence: 0.95,
            bootstrap_seed: 0x707E_17E5,
            pin_prefix: String::new(),
        }
    }
}

impl AcceptConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        let cfg: Self = toml::from_str(&raw)?;
        anyhow::ensure!(cfg.runs_per_side >= 2, "runs_per_side must be >= 2");
        anyhow::ensure!(cfg.threshold > 0.0, "threshold must be positive");
        anyhow::ensure!(
            (0.0..1.0).contains(&cfg.confidence),
            "confidence must be in (0, 1)"
        );
        Ok(cfg)
    }
}
