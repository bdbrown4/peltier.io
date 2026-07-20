//! Post-verdict diagnosis (SPEC §3.7). Turns one ledger row into a plain-text
//! explanation of WHY the attempt won or lost.
//!
//! Invariants, in order of importance:
//! - **Advisory.** Runs after the verdict is written; cannot change one.
//! - **Ledger-only inputs.** Every statement derives from the row passed in —
//!   no config files, no re-benching, no clock. Same row, same bytes out.
//! - **Verdicts are records; explanations are inferences.** Lines that go
//!   beyond restating the row are prefixed `inference:` so nobody can quote
//!   an explanation as if the machine measured it.

use serde_json::Value;

/// Everything explain reads, pulled out of the row once.
pub struct Facts {
    pub run_id: String,
    pub target: String,
    pub verdict: String,
    pub hypothesis: String,
    pub hotspot: String,
    pub playbook_class: i64,
    pub timestamp: String,
    pub patch: String,
    // gates
    pub upstream_tests: bool,
    pub golden_replay: bool,
    pub fuzz_iters: u64,
    pub fuzz_divergence: bool,
    pub sanitizers_clean: bool,
    pub tsan_clean: Option<bool>,
    pub risk_signals: Vec<String>,
    pub equivalence_mode: Option<String>,
    // bench (absent when gates failed before benching)
    pub bench: Option<BenchFacts>,
}

pub struct BenchFacts {
    pub baseline_median: f64,
    pub candidate_median: f64,
    pub speedup_median: f64,
    pub speedup_lo: f64,
    pub speedup_hi: f64,
    pub workload: Option<String>,
    pub isolation: Option<String>,
    /// Accept bar in force for this run — recorded per-row since 2026-07-20;
    /// None on older rows (and explain must not substitute today's config).
    pub accept_threshold: Option<f64>,
    pub max_rss_baseline_kib: Option<u64>,
    pub max_rss_candidate_kib: Option<u64>,
}

impl Facts {
    pub fn from_row(row: &Value) -> Option<Facts> {
        let s = |k: &str| row.get(k)?.as_str().map(str::to_string);
        let gates = row.get("gates")?;
        let bench = row.get("bench").filter(|b| !b.is_null()).map(|b| {
            let env = b.get("env_fingerprint");
            let ci = b.get("speedup_ci").and_then(|c| c.as_array().cloned());
            let ci_at = |i: usize| -> f64 {
                ci.as_ref()
                    .and_then(|c| c.get(i))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(f64::NAN)
            };
            let rss = env.and_then(|e| e.get("max_rss_kib"));
            BenchFacts {
                baseline_median: b
                    .get("baseline_median")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(f64::NAN),
                candidate_median: b
                    .get("candidate_median")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(f64::NAN),
                speedup_median: b
                    .get("speedup_median")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(f64::NAN),
                speedup_lo: ci_at(0),
                speedup_hi: ci_at(1),
                workload: env
                    .and_then(|e| e.get("workload"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                isolation: env
                    .and_then(|e| e.get("isolation"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                accept_threshold: env
                    .and_then(|e| e.get("accept_threshold"))
                    .and_then(|v| v.as_f64()),
                max_rss_baseline_kib: rss
                    .and_then(|r| r.get("baseline_median"))
                    .and_then(|v| v.as_u64()),
                max_rss_candidate_kib: rss
                    .and_then(|r| r.get("candidate_median"))
                    .and_then(|v| v.as_u64()),
            }
        });
        Some(Facts {
            run_id: s("run_id")?,
            target: s("target")?,
            verdict: s("verdict")?,
            hypothesis: s("hypothesis")?,
            hotspot: s("hotspot")?,
            playbook_class: row.get("playbook_class")?.as_i64()?,
            timestamp: s("timestamp")?,
            patch: s("patch")?,
            upstream_tests: gates
                .get("upstream_tests")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            golden_replay: gates
                .get("golden_replay")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            fuzz_iters: gates
                .get("fuzz_iters")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            fuzz_divergence: gates
                .get("fuzz_divergence")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            sanitizers_clean: gates
                .get("sanitizers_clean")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            tsan_clean: gates.get("tsan_clean").and_then(|v| v.as_bool()),
            risk_signals: gates
                .get("risk_signals")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            equivalence_mode: gates
                .get("equivalence_mode")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            bench,
        })
    }
}

/// Where the bootstrap CI of the speedup ratio landed, relative to 1.0 and
/// (when the row records it) the accept bar.
#[derive(Debug, PartialEq)]
pub enum CiClass {
    /// Lower bound clears the recorded bar.
    MeetsBar,
    /// CI excludes 1.0 upward, but does not clear the bar (or no bar recorded).
    RealBelowBar,
    /// CI straddles 1.0 — indistinguishable from no change.
    Indistinguishable,
    /// CI entirely below 1.0 — a measured regression.
    Regression,
}

pub fn classify_ci(lo: f64, hi: f64, threshold: Option<f64>) -> CiClass {
    if hi < 1.0 {
        CiClass::Regression
    } else if lo <= 1.0 {
        CiClass::Indistinguishable
    } else if let Some(t) = threshold {
        if lo >= 1.0 + t {
            CiClass::MeetsBar
        } else {
            CiClass::RealBelowBar
        }
    } else {
        CiClass::RealBelowBar
    }
}

pub struct DiffStats {
    pub files: usize,
    pub added: usize,
    pub removed: usize,
}

/// Mechanical shape of the stored unified diff. None when the row carries a
/// placeholder instead of a real patch (build-config attempts, smoke rows).
pub fn diff_stats(patch: &str) -> Option<DiffStats> {
    if !patch.contains("--- ") || !patch.contains("+++ ") {
        return None;
    }
    let mut files = 0;
    let mut added = 0;
    let mut removed = 0;
    for line in patch.lines() {
        if line.starts_with("+++ ") {
            files += 1;
        } else if line.starts_with('+') && !line.starts_with("+++") {
            added += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            removed += 1;
        }
    }
    Some(DiffStats {
        files,
        added,
        removed,
    })
}

fn pct(x: f64) -> String {
    format!("{:+.1}%", (x - 1.0) * 100.0)
}

/// The whole report. Pure function of the facts: byte-identical on re-run.
pub fn explain(f: &Facts) -> String {
    let mut out = String::new();
    let mut line = |s: String| {
        out.push_str(&s);
        out.push('\n');
    };

    line(format!("explain {} — advisory diagnosis", f.run_id));
    line(
        "derived solely from the ledger row; regeneration is byte-identical; \
          this can never change a verdict"
            .to_string(),
    );
    line(String::new());
    line(format!(
        "attempt: {} | class {} | hotspot: {} | recorded {}",
        f.target, f.playbook_class, f.hotspot, f.timestamp
    ));
    line(format!("hypothesis: {}", f.hypothesis));
    if let Some(d) = diff_stats(&f.patch) {
        line(format!(
            "patch shape: {} file(s), +{} / -{} lines",
            d.files, d.added, d.removed
        ));
    } else {
        line(
            "patch shape: no source diff recorded (build-config or out-of-band attempt)"
                .to_string(),
        );
    }
    line(String::new());
    line(format!("verdict (machine record): {}", f.verdict));

    // --- what decided it -------------------------------------------------
    match f.verdict.as_str() {
        "rejected-gate" => {
            let failed: Vec<&str> = [
                (!f.upstream_tests, "upstream test suite"),
                (!f.golden_replay, "golden replay"),
                (f.fuzz_divergence, "differential fuzz (divergence found)"),
            ]
            .iter()
            .filter(|(cond, _)| *cond)
            .map(|(_, name)| *name)
            .collect();
            if failed.is_empty() {
                line("decided by: a gate failure (the failing gate is not individually recorded in this row)".to_string());
            } else {
                line(format!("decided by: gate failure — {}", failed.join(", ")));
            }
            line(
                "inference: the patch changed observable behavior or broke the build contract; \
                  the bench (if any ran) is irrelevant until equivalence holds"
                    .to_string(),
            );
        }
        "accepted" | "rejected-bench" | "needs-human-review" => {
            if let Some(b) = &f.bench {
                line(format!(
                    "bench: baseline median {:.4}s, candidate median {:.4}s — speedup {:.4} ({}), 95% CI [{:.4}, {:.4}]",
                    b.baseline_median, b.candidate_median, b.speedup_median,
                    pct(b.speedup_median), b.speedup_lo, b.speedup_hi
                ));
                if let Some(w) = &b.workload {
                    line(format!("workload: {}", w));
                }
                match &b.accept_threshold {
                    Some(t) => line(format!(
                        "accept bar recorded for this run: CI lower bound >= {:.2} (1 + threshold {:.2})",
                        1.0 + t, t
                    )),
                    None => line(
                        "accept bar: not machine-recorded for this row (rows before 2026-07-20); \
                         the rule is CI lower bound >= 1 + threshold from config/accept.toml at run time"
                            .to_string(),
                    ),
                }
                // The narrative must agree with the machine verdict: an
                // `accepted` row cleared the bar in force at run time by
                // definition, even when the row predates per-row threshold
                // recording — never describe an accept as a rejection.
                match f.verdict.as_str() {
                    "accepted" => {
                        line(format!(
                            "inference: the defensible claim is the lower bound — \"at least {}\" on this workload, not the median {}",
                            pct(b.speedup_lo), pct(b.speedup_median)
                        ));
                        if let Some(t) = b.accept_threshold {
                            if b.speedup_lo < 1.0 + t + 0.01 {
                                line("inference: thin margin over the bar — re-run on calibrated hardware before any external claim".to_string());
                            }
                        }
                    }
                    "needs-human-review" => {
                        line("inference: the numbers were not the cap — the review routing below decided this verdict".to_string());
                    }
                    _ => match classify_ci(b.speedup_lo, b.speedup_hi, b.accept_threshold) {
                        CiClass::MeetsBar => {
                            line("inference: the CI lower bound clears the recorded bar yet the verdict is a rejection — inconsistent row, inspect by hand".to_string());
                        }
                        CiClass::RealBelowBar => {
                            line(format!(
                                "inference: the improvement is real (CI excludes 1.0) but below the ship bar — \
                                 a policy rejection, not noise; the measured effect is {} to {}",
                                pct(b.speedup_lo), pct(b.speedup_hi)
                            ));
                        }
                        CiClass::Indistinguishable => {
                            line("inference: indistinguishable from no change — the CI straddles 1.0. \
                                  This is a null result, not a small win"
                                .to_string());
                        }
                        CiClass::Regression => {
                            line(format!(
                                "inference: a measured regression — the candidate is {} to {} SLOWER; report it as one",
                                pct(2.0 - b.speedup_hi), pct(2.0 - b.speedup_lo)
                            ));
                        }
                    },
                }
                if let (Some(rb), Some(rc)) = (b.max_rss_baseline_kib, b.max_rss_candidate_kib) {
                    if rb > 0 {
                        let ratio = rc as f64 / rb as f64;
                        line(format!(
                            "max RSS medians: baseline {} KiB, candidate {} KiB ({}) — context, never an accept metric",
                            rb, rc, pct(ratio)
                        ));
                        if f.verdict == "accepted" && ratio > 1.05 {
                            line("inference: the speedup is partly bought with memory — note the trade in any claim".to_string());
                        }
                    }
                }
                if let Some(iso) = &b.isolation {
                    if iso.contains("NOT isolated") || iso == "unwrapped-host" {
                        line(format!(
                            "isolation note on record: \"{}\" — this run did not have the network-namespace boundary",
                            iso
                        ));
                    }
                }
            } else {
                line("bench: no evidence recorded on this row".to_string());
            }
        }
        other => line(format!(
            "verdict string '{}' is outside the known set — inspect the row by hand",
            other
        )),
    }

    // --- caps and caveats -------------------------------------------------
    if f.verdict == "needs-human-review" {
        if !f.risk_signals.is_empty() {
            line(format!(
                "routed to human review by the lexical risk classifier (SPEC §8): {}",
                f.risk_signals.join(", ")
            ));
            line("inference: the numbers do not clear a risk class — concurrency/unsafe/FP changes can be \
                  correct on this workload and wrong in production"
                .to_string());
        } else if f.fuzz_iters == 0 {
            line("no passed differential-fuzz gate on record — the no-fuzz-no-accept rule caps any would-be accept at review".to_string());
        }
        if f.equivalence_mode.as_deref() == Some("fp-tolerance") {
            line(
                "equivalence used the fp-tolerance tier — always a review signal (SPEC §8)"
                    .to_string(),
            );
        }
    }
    if f.verdict == "accepted" {
        if !f.sanitizers_clean {
            line("CAUTION: accepted with sanitizers_clean=false — this row predates the machine-enforced \
                  sanitizer lane; under current doctrine it could not be machine-accepted. See results/rulings/"
                .to_string());
        }
        match f.fuzz_iters {
            0 => line(
                "fuzz: 0 iterations in the machine record — a pre-integration row; differential fuzz \
                 ran out-of-band via scripts through Phase 5 (an accept today requires a passed in-pipeline run)"
                    .to_string(),
            ),
            n => line(format!("fuzz: {} iterations executed, no divergence", n)),
        }
        if let Some(false) = f.tsan_clean {
            line("CAUTION: TSan lane flagged this attempt".to_string());
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ci_classification_covers_all_regions() {
        assert_eq!(classify_ci(0.91, 0.96, Some(0.02)), CiClass::Regression);
        assert_eq!(
            classify_ci(0.98, 1.01, Some(0.02)),
            CiClass::Indistinguishable
        );
        assert_eq!(classify_ci(1.005, 1.015, Some(0.02)), CiClass::RealBelowBar);
        assert_eq!(classify_ci(1.03, 1.06, Some(0.02)), CiClass::MeetsBar);
        // No recorded bar: a real improvement can never be called MeetsBar.
        assert_eq!(classify_ci(1.08, 1.10, None), CiClass::RealBelowBar);
        assert_eq!(
            classify_ci(1.0, 1.02, Some(0.02)),
            CiClass::Indistinguishable
        );
    }

    #[test]
    fn diff_stats_counts_and_rejects_placeholders() {
        let d = diff_stats("--- a/f.c\n+++ b/f.c\n@@\n-old\n+new\n+more\n").unwrap();
        assert_eq!((d.files, d.added, d.removed), (1, 2, 1));
        assert!(diff_stats("(no source patch)").is_none());
    }

    fn row(verdict: &str, gates: serde_json::Value, bench: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "run_id": "t-001", "timestamp": "2026-07-20T00:00:00Z", "target": "demo",
            "target_commit": "abc", "phase": 6, "hotspot": "hot() (40%)",
            "playbook_class": 5, "hypothesis": "loop fusion cuts passes",
            "patch": "--- a/x\n+++ b/x\n-a\n+b\n", "gates": gates, "bench": bench,
            "verdict": verdict, "tokens_spent": 0, "wall_time_s": 1.0
        })
    }

    fn bench(lo: f64, hi: f64, thr: Option<f64>) -> serde_json::Value {
        let mut env = serde_json::json!({"workload": "10k records", "isolation": "no-net.sh"});
        if let Some(t) = thr {
            env["accept_threshold"] = serde_json::json!(t);
        }
        serde_json::json!({
            "baseline_median": 2.0, "baseline_ci": [1.9, 2.1],
            "candidate_median": 1.8, "candidate_ci": [1.7, 1.9],
            "speedup_median": (lo + hi) / 2.0, "speedup_ci": [lo, hi],
            "env_fingerprint": env
        })
    }

    #[test]
    fn explanation_is_deterministic_and_labels_inference() {
        let r = row(
            "accepted",
            serde_json::json!({"upstream_tests": true, "golden_replay": true,
                "fuzz_iters": 10000, "fuzz_divergence": false, "sanitizers_clean": true}),
            bench(1.05, 1.09, Some(0.02)),
        );
        let f = Facts::from_row(&r).unwrap();
        let a = explain(&f);
        let b = explain(&f);
        assert_eq!(a, b, "explanations must be byte-identical on re-run");
        assert!(a.contains("advisory"));
        assert!(a.contains("inference:"));
        assert!(
            a.contains("at least +5.0%"),
            "claim must be the lower bound: {a}"
        );
        assert!(a.contains("10000 iterations"));
    }

    #[test]
    fn regression_and_null_results_are_named_plainly() {
        let g = serde_json::json!({"upstream_tests": true, "golden_replay": true,
            "fuzz_iters": 0, "fuzz_divergence": false, "sanitizers_clean": false});
        let reg = Facts::from_row(&row(
            "rejected-bench",
            g.clone(),
            bench(0.92, 0.97, Some(0.02)),
        ))
        .unwrap();
        assert!(explain(&reg).contains("SLOWER"));
        let null =
            Facts::from_row(&row("rejected-bench", g, bench(0.99, 1.01, Some(0.02)))).unwrap();
        assert!(explain(&null).contains("indistinguishable from no change"));
    }

    #[test]
    fn historical_accept_without_sanitizers_is_flagged() {
        let r = row(
            "accepted",
            serde_json::json!({"upstream_tests": true, "golden_replay": true,
                "fuzz_iters": 0, "fuzz_divergence": false, "sanitizers_clean": false}),
            bench(1.08, 1.12, None),
        );
        let text = explain(&Facts::from_row(&r).unwrap());
        assert!(text.contains("sanitizers_clean=false"));
        assert!(text.contains("not machine-recorded for this row"));
        assert!(text.contains("out-of-band"));
        // An accepted row cleared the bar in force at run time by definition:
        // the missing per-row threshold must never read as a rejection.
        assert!(!text.contains("policy rejection"), "{text}");
        assert!(text.contains("at least +8.0%"));
    }

    #[test]
    fn nhr_names_its_cap() {
        let r = row(
            "needs-human-review",
            serde_json::json!({"upstream_tests": true, "golden_replay": true,
                "fuzz_iters": 10000, "fuzz_divergence": false, "sanitizers_clean": true,
                "risk_signals": ["concurrency"], "equivalence_mode": "byte-identical"}),
            bench(1.10, 1.15, Some(0.02)),
        );
        let text = explain(&Facts::from_row(&r).unwrap());
        assert!(text.contains("lexical risk classifier"));
        assert!(text.contains("concurrency"));
    }
}
