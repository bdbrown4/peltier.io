//! Pricing inputs (`config/pricing.toml`). Rates are engagement inputs,
//! not constants — public cloud list prices by default, customer's own
//! numbers when supplied.

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pricing {
    /// Dollars per core-hour (e.g. public-cloud on-demand vCPU rate).
    pub dollars_per_core_hour: f64,
    /// Fleet hours per year the workload actually runs (8760 for 24/7).
    pub hours_per_year: f64,
    /// Where the rate came from — printed on the report.
    pub rate_source: String,
}

impl Pricing {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        let p: Self = toml::from_str(&raw)?;
        anyhow::ensure!(p.dollars_per_core_hour > 0.0, "rate must be positive");
        anyhow::ensure!(
            p.hours_per_year > 0.0 && p.hours_per_year <= 8784.0,
            "hours_per_year must be in (0, 8784]"
        );
        Ok(p)
    }
}
