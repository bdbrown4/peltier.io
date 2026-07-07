//! Per-target equivalence policy (`targets/<name>/equivalence.toml`).
//! Absent a policy file, byte-identical is required (SPEC §3.2).

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum EquivalencePolicy {
    /// Default: stdout, output files, and exit codes must match exactly.
    ByteIdentical,
    /// FP-producing targets only: numeric fields may differ within the
    /// declared tolerance; everything else stays byte-identical. Using
    /// this tier at all routes the attempt to needs-human-review when
    /// FP *flags* changed (SPEC §8) — tolerance covers output compare,
    /// not compiler-flag laxity.
    FpTolerance {
        /// Maximum absolute difference per numeric value.
        abs: f64,
        /// Maximum relative difference per numeric value.
        rel: f64,
    },
}

impl EquivalencePolicy {
    /// Load the target's policy, defaulting to byte-identical when no
    /// equivalence.toml exists.
    pub fn load(target_dir: &Path) -> anyhow::Result<Self> {
        let path = target_dir.join("equivalence.toml");
        if !path.exists() {
            return Ok(Self::ByteIdentical);
        }
        let raw = std::fs::read_to_string(&path)?;
        let policy: Self = toml::from_str(&raw)?;
        if let Self::FpTolerance { abs, rel } = &policy {
            anyhow::ensure!(
                *abs >= 0.0 && *rel >= 0.0,
                "tolerances must be non-negative"
            );
        }
        Ok(policy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_policy_means_byte_identical() {
        let dir = std::env::temp_dir().join("hotpath-policy-none");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            EquivalencePolicy::load(&dir).unwrap(),
            EquivalencePolicy::ByteIdentical
        );
    }

    #[test]
    fn parses_fp_tolerance() {
        let dir = std::env::temp_dir().join("hotpath-policy-fp");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("equivalence.toml"),
            "mode = \"fp-tolerance\"\nabs = 1e-9\nrel = 1e-6\n",
        )
        .unwrap();
        assert_eq!(
            EquivalencePolicy::load(&dir).unwrap(),
            EquivalencePolicy::FpTolerance {
                abs: 1e-9,
                rel: 1e-6
            }
        );
    }
}
