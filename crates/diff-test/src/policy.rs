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

    /// Compare two outputs under this policy. `ByteIdentical` requires an
    /// exact match. `FpTolerance` tokenizes on whitespace and compares
    /// numeric tokens within `abs + rel*|baseline|`; non-numeric tokens
    /// (and mismatched token counts) must still match exactly. This is the
    /// gate the kernel lane (SPEC §13) needs: an optimization that reorders
    /// floating-point accumulation changes the last ULPs, so it fails
    /// byte-identical but passes a declared tolerance — while a genuine
    /// correctness bug (a wrong result) still exceeds it.
    pub fn compare(&self, baseline: &str, candidate: &str) -> Result<(), Divergence> {
        match self {
            Self::ByteIdentical => {
                if baseline == candidate {
                    Ok(())
                } else {
                    let pos = baseline
                        .bytes()
                        .zip(candidate.bytes())
                        .position(|(a, b)| a != b)
                        .unwrap_or(baseline.len().min(candidate.len()));
                    Err(Divergence {
                        token: pos,
                        baseline: snippet(baseline, pos),
                        candidate: snippet(candidate, pos),
                        reason: "byte-identical mismatch".into(),
                    })
                }
            }
            Self::FpTolerance { abs, rel } => {
                let bt: Vec<&str> = baseline.split_whitespace().collect();
                let ct: Vec<&str> = candidate.split_whitespace().collect();
                if bt.len() != ct.len() {
                    return Err(Divergence {
                        token: bt.len().min(ct.len()),
                        baseline: format!("{} tokens", bt.len()),
                        candidate: format!("{} tokens", ct.len()),
                        reason: "token-count mismatch".into(),
                    });
                }
                for (i, (b, c)) in bt.iter().zip(&ct).enumerate() {
                    match (b.parse::<f64>(), c.parse::<f64>()) {
                        (Ok(bn), Ok(cn)) => {
                            if bn.is_nan() && cn.is_nan() {
                                continue;
                            }
                            let d = (bn - cn).abs();
                            let tol = abs + rel * bn.abs();
                            // Explicit NaN handling: a lone NaN (the other
                            // side finite) is a divergence, not a pass.
                            if d.is_nan() || d > tol {
                                return Err(Divergence {
                                    token: i,
                                    baseline: (*b).to_string(),
                                    candidate: (*c).to_string(),
                                    reason: format!("|Δ|={d:.3e} exceeds tolerance {tol:.3e}"),
                                });
                            }
                        }
                        _ => {
                            if b != c {
                                return Err(Divergence {
                                    token: i,
                                    baseline: (*b).to_string(),
                                    candidate: (*c).to_string(),
                                    reason: "non-numeric token mismatch".into(),
                                });
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

/// The first place two outputs diverge under a policy.
#[derive(Debug, Clone, PartialEq)]
pub struct Divergence {
    /// Token index (FpTolerance) or byte offset (ByteIdentical).
    pub token: usize,
    pub baseline: String,
    pub candidate: String,
    pub reason: String,
}

fn snippet(s: &str, pos: usize) -> String {
    let start = pos.saturating_sub(8);
    let end = (pos + 8).min(s.len());
    s.get(start..end).unwrap_or("").replace('\n', "\\n")
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

    #[test]
    fn byte_identical_rejects_reordered_fp() {
        // The blocked kernel's low-bit difference: byte-identical says no.
        let p = EquivalencePolicy::ByteIdentical;
        assert!(p.compare("1.0000000 2.0", "1.0000001 2.0").is_err());
    }

    #[test]
    fn fp_tolerance_accepts_low_bits_rejects_real_error() {
        let p = EquivalencePolicy::FpTolerance {
            abs: 1e-6,
            rel: 1e-5,
        };
        // Last-ULP reordering: within tolerance.
        assert!(p.compare("1.0000000 42.0", "1.0000001 42.0").is_ok());
        // A genuine wrong result: exceeds tolerance, caught.
        let d = p.compare("1.0 42.0", "1.0 42.5").unwrap_err();
        assert_eq!(d.token, 1);
        // Non-numeric tokens still must match exactly.
        assert!(p.compare("ok 1.0", "bad 1.0").is_err());
        // Token-count mismatch is a divergence.
        assert!(p.compare("1.0 2.0", "1.0").is_err());
    }
}
