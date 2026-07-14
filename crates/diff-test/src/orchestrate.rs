//! Core gate sequence, callable from the diff-test CLI and from verdict:
//! corpus + test-suite pins (refuse on mismatch) → upstream tests →
//! policy-aware golden replay → differential fuzz. Sanitizers run on the
//! accept path in verdict and are reported here as skipped with a reason.
//!
//! Differential fuzz is the one gate that is not purely a property of the
//! candidate: it differs the candidate against a *pristine baseline*, so it
//! needs both binaries. Callers that have only a candidate (the standalone
//! `just gates` flow) pass `baseline_binary: None` and the gate is skipped
//! with that reason recorded — it is never faked by comparing the candidate
//! against itself, and verdict refuses to mint an accept without it.

use crate::policy::EquivalencePolicy;
use crate::target::{expected_golden_hash, TargetSpec};
use crate::{pin, GateInputs, GateLayer, GateOutcome, GateReport};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn sh(root: &Path, cmd: &str) -> anyhow::Result<std::process::Output> {
    Ok(Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?)
}

/// TESTSUITE.sha256 lives beside the corpus MANIFEST.sha256 (both under
/// corpora/<target>/), so its path derives from the manifest's.
fn suite_manifest_path(corpus_manifest: &Path) -> PathBuf {
    corpus_manifest
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join("TESTSUITE.sha256")
}

fn parse_u64_strict(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Parse the last `FUZZ-RESULT iters=<u64> divergences=<u64>` stdout line
/// of a fuzz command (exact ASCII form, single spaces). Returns
/// `(iters, divergences)`, or None when no line matches.
pub fn parse_fuzz_result(stdout: &str) -> Option<(u64, u64)> {
    stdout.lines().rev().find_map(|line| {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let rest = line.strip_prefix("FUZZ-RESULT iters=")?;
        let (iters, rest) = rest.split_once(' ')?;
        let divergences = rest.strip_prefix("divergences=")?;
        Some((parse_u64_strict(iters)?, parse_u64_strict(divergences)?))
    })
}

/// Recorded verbatim when the target declares no fuzz command at all.
const FUZZ_SKIP_NO_COMMAND: &str = "no fuzz command declared in target.toml [gates].fuzz";

/// Recorded verbatim when a fuzz command exists but the caller has no
/// pristine baseline: differential fuzz differs two binaries, and comparing
/// the candidate against itself would be a gate that cannot fail.
const FUZZ_SKIP_NO_BASELINE: &str =
    "differential fuzz needs a pristine baseline binary; run the accept path (`just verdict`), \
     which rebuilds one";

/// Substitute the three `[gates].fuzz` template placeholders. The contract
/// with `targets/<t>/target.toml` and `scripts/diff-fuzz-*.py`: `{iters}`
/// is the iteration budget, `{baseline}` the pristine baseline binary and
/// `{candidate}` the patched one (both repo-root-relative paths, and the
/// command runs from the repo root). A fuzz script must never hardcode a
/// baseline path — the harness owns where the pristine build lands.
fn fuzz_command(tmpl: &str, iters: u64, baseline: &str, candidate: &str) -> String {
    tmpl.replace("{iters}", &iters.to_string())
        .replace("{baseline}", baseline)
        .replace("{candidate}", candidate)
}

/// What the differential-fuzz gate will do, decided before any process runs.
#[derive(Debug, PartialEq, Eq)]
enum FuzzPlan {
    Skip(&'static str),
    Run(String),
}

/// Differential fuzz is a baseline-vs-candidate gate: it needs both sides.
/// Missing either one is a Skip with the reason recorded, never a silent
/// self-comparison and never a hard error (the standalone gates flow has
/// no baseline by construction).
fn plan_fuzz(fuzz: Option<&str>, inputs: &GateInputs) -> FuzzPlan {
    match (fuzz, inputs.baseline_binary) {
        (None, _) => FuzzPlan::Skip(FUZZ_SKIP_NO_COMMAND),
        (Some(_), None) => FuzzPlan::Skip(FUZZ_SKIP_NO_BASELINE),
        (Some(tmpl), Some(baseline)) => FuzzPlan::Run(fuzz_command(
            tmpl,
            inputs.fuzz_iters,
            baseline,
            inputs.candidate_binary,
        )),
    }
}

/// Grade one executed fuzz run. Passed iff the command exited 0, printed a
/// well-formed `FUZZ-RESULT` line, and reported zero divergences — a run
/// that never reports the line is a Failed gate, not a pass by silence.
/// Returns `(outcome, iters_actually_executed, divergence)`.
fn fuzz_gate_outcome(
    stdout: &str,
    success: bool,
    exit_code: Option<i32>,
) -> (GateOutcome, u64, bool) {
    match parse_fuzz_result(stdout) {
        None => (
            GateOutcome::Failed {
                detail: "fuzz command did not report FUZZ-RESULT".into(),
            },
            0,
            false,
        ),
        Some((iters, divergences)) => (
            if success && divergences == 0 {
                GateOutcome::Passed
            } else {
                GateOutcome::Failed {
                    detail: format!("exit={exit_code:?} iters={iters} divergences={divergences}"),
                }
            },
            iters,
            divergences > 0,
        ),
    }
}

/// The fp-tolerance policy fails closed without a committed reference:
/// there is no hash to fall back to that a tolerance could compare against.
fn fp_reference(golden_reference: Option<&PathBuf>) -> Result<&PathBuf, GateOutcome> {
    golden_reference.ok_or_else(|| GateOutcome::Failed {
        detail: "fp-tolerance policy requires corpus.golden_reference in target.toml".into(),
    })
}

/// GoldenReplay verdict under an fp-tolerance policy: the replay stdout is
/// compared token-wise against the committed reference output.
fn fp_golden_outcome(
    policy: &EquivalencePolicy,
    reference: &str,
    stdout: &str,
    success: bool,
    exit_code: Option<i32>,
) -> GateOutcome {
    if !success {
        return GateOutcome::Failed {
            detail: format!("golden replay command failed (exit={exit_code:?})"),
        };
    }
    match policy.compare(reference, stdout) {
        Ok(()) => GateOutcome::Passed,
        Err(d) => GateOutcome::Failed {
            detail: format!(
                "diverged at token/offset {}: baseline={:?} candidate={:?} — {}",
                d.token, d.baseline, d.candidate, d.reason
            ),
        },
    }
}

/// Run the mechanical gates for `target`, testing `inputs.candidate_binary`
/// (repo-root-relative) in golden replay and differential fuzz. Differential
/// fuzz additionally needs `inputs.baseline_binary` to differ against and is
/// skipped-with-reason without one. Pin mismatches (corpus or test suite)
/// are errors, not gate failures: nothing runs on tampered inputs.
pub fn run_core_gates(
    root: &Path,
    spec: &TargetSpec,
    inputs: &GateInputs,
) -> anyhow::Result<GateReport> {
    let verified = pin::verify_manifest(
        &root.join(&spec.corpus.manifest),
        &root.join(&spec.corpus.root),
    )?;
    eprintln!(
        "corpus pin: {verified} files verified against {}",
        spec.corpus.manifest.display()
    );

    // Test-suite pin: same refuse-on-mismatch posture as the corpus pin,
    // but the manifest is optional until a human deliberately generates it.
    let suite_manifest = suite_manifest_path(&spec.corpus.manifest);
    if root.join(&suite_manifest).exists() {
        let verified = pin::verify_manifest(&root.join(&suite_manifest), root).map_err(|e| {
            anyhow::anyhow!("upstream test suite hash mismatch — refusing to run: {e}")
        })?;
        eprintln!(
            "test-suite pin: {verified} files verified against {}",
            suite_manifest.display()
        );
    } else {
        eprintln!(
            "test-suite pin: {} not found — suite unpinned (generate with scripts/pin-testsuite.sh)",
            suite_manifest.display()
        );
    }

    let mut results = Vec::new();

    let t = sh(root, &spec.gates.tests)?;
    results.push((
        GateLayer::UpstreamTests,
        if t.status.success() {
            GateOutcome::Passed
        } else {
            GateOutcome::Failed {
                detail: String::from_utf8_lossy(&t.stderr)
                    .chars()
                    .take(2000)
                    .collect(),
            }
        },
    ));

    let policy = EquivalencePolicy::load(&spec.target_dir(root))?;
    let equivalence_mode = match &policy {
        EquivalencePolicy::ByteIdentical => "byte-identical",
        EquivalencePolicy::FpTolerance { .. } => "fp-tolerance",
    };
    let golden_cmd = spec
        .gates
        .golden
        .replace("{binary}", inputs.candidate_binary);
    let golden = match &policy {
        EquivalencePolicy::ByteIdentical => {
            let g = sh(root, &golden_cmd)?;
            let expected = expected_golden_hash(&root.join(&spec.corpus.golden_sha256))?;
            let actual = format!("{:x}", Sha256::digest(&g.stdout));
            if g.status.success() && actual == expected {
                GateOutcome::Passed
            } else {
                GateOutcome::Failed {
                    detail: format!(
                        "exit={:?} expected={expected} actual={actual}",
                        g.status.code()
                    ),
                }
            }
        }
        EquivalencePolicy::FpTolerance { .. } => {
            match fp_reference(spec.corpus.golden_reference.as_ref()) {
                Err(outcome) => outcome,
                Ok(reference_path) => {
                    let reference =
                        std::fs::read_to_string(root.join(reference_path)).map_err(|e| {
                            anyhow::anyhow!(
                                "cannot read corpus.golden_reference {}: {e}",
                                reference_path.display()
                            )
                        })?;
                    let g = sh(root, &golden_cmd)?;
                    fp_golden_outcome(
                        &policy,
                        &reference,
                        &String::from_utf8_lossy(&g.stdout),
                        g.status.success(),
                        g.status.code(),
                    )
                }
            }
        }
    };
    results.push((GateLayer::GoldenReplay, golden));

    let (fuzz_outcome, fuzz_iters, fuzz_divergence) =
        match plan_fuzz(spec.gates.fuzz.as_deref(), inputs) {
            FuzzPlan::Skip(reason) => (
                GateOutcome::Skipped {
                    reason: reason.to_string(),
                },
                0,
                false,
            ),
            FuzzPlan::Run(cmd) => {
                let f = sh(root, &cmd)?;
                fuzz_gate_outcome(
                    &String::from_utf8_lossy(&f.stdout),
                    f.status.success(),
                    f.status.code(),
                )
            }
        };
    results.push((GateLayer::DifferentialFuzz, fuzz_outcome));

    results.push((
        GateLayer::Sanitizers,
        GateOutcome::Skipped {
            reason:
                "sanitizers run on the accept path in verdict (ASan/UBSan, TSan when configured)"
                    .into(),
        },
    ));

    Ok(GateReport {
        gates: results,
        fuzz_iters,
        fuzz_divergence,
        equivalence_mode: equivalence_mode.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suite_manifest_derives_from_corpus_manifest() {
        assert_eq!(
            suite_manifest_path(Path::new("corpora/cjson/MANIFEST.sha256")),
            Path::new("corpora/cjson").join("TESTSUITE.sha256")
        );
        assert_eq!(
            suite_manifest_path(Path::new("MANIFEST.sha256")),
            Path::new("TESTSUITE.sha256")
        );
    }

    #[test]
    fn fuzz_result_parses_last_exact_line() {
        assert_eq!(
            parse_fuzz_result("FUZZ-RESULT iters=10000 divergences=0\n"),
            Some((10000, 0))
        );
        // Last matching line wins; trailing non-matching output is ignored.
        let out = "FUZZ-RESULT iters=1 divergences=1\nprogress...\nFUZZ-RESULT iters=9999 divergences=2\ndone\n";
        assert_eq!(parse_fuzz_result(out), Some((9999, 2)));
        // CRLF-terminated capture still parses.
        assert_eq!(
            parse_fuzz_result("FUZZ-RESULT iters=42 divergences=0\r\n"),
            Some((42, 0))
        );
    }

    #[test]
    fn fuzz_result_rejects_malformed_lines() {
        for bad in [
            "",
            "no result here",
            " FUZZ-RESULT iters=1 divergences=0", // leading space
            "FUZZ-RESULT iters=1  divergences=0", // double space
            "FUZZ-RESULT iters=abc divergences=0", // non-numeric
            "FUZZ-RESULT iters=1 divergences=-1", // sign not allowed
            "FUZZ-RESULT iters=1 divergences=0 xx", // trailing junk
            "FUZZ-RESULT divergences=0 iters=1",  // wrong field order
            "FUZZ-RESULT iters=1",                // missing field
        ] {
            assert_eq!(parse_fuzz_result(bad), None, "should reject: {bad:?}");
        }
    }

    fn inputs(baseline: Option<&'static str>) -> GateInputs<'static> {
        GateInputs {
            candidate_binary: "targets/cjson/cand/cjson-bench",
            baseline_binary: baseline,
            fuzz_iters: 10_000,
        }
    }

    const TMPL: &str = "python3 scripts/diff-fuzz-cjson.py targets/cjson/fuzz-work \
                        {iters} {baseline} {candidate}";

    #[test]
    fn fuzz_command_substitutes_all_three_placeholders() {
        assert_eq!(
            fuzz_command(TMPL, 500, "base/bin", "cand/bin"),
            "python3 scripts/diff-fuzz-cjson.py targets/cjson/fuzz-work 500 base/bin cand/bin"
        );
        // Repeated placeholders all substitute; unknown braces are left alone.
        assert_eq!(
            fuzz_command("{candidate} {candidate} {iters} {other}", 2, "b", "c"),
            "c c 2 {other}"
        );
    }

    #[test]
    fn fuzz_skipped_when_no_command_declared() {
        assert_eq!(
            plan_fuzz(None, &inputs(Some("targets/cjson/baseline/cjson-bench"))),
            FuzzPlan::Skip("no fuzz command declared in target.toml [gates].fuzz")
        );
        // Absent command wins even without a baseline: nothing to run either way.
        assert_eq!(
            plan_fuzz(None, &inputs(None)),
            FuzzPlan::Skip(FUZZ_SKIP_NO_COMMAND)
        );
    }

    /// The regression this branch exists for: `just gates` (and CI) build only
    /// the working tree, so there is no pristine baseline. Before the fix the
    /// fuzz script silently hardcoded targets/<t>/baseline/… — a path nothing
    /// on that flow builds — and the gate hard-failed. Now it skips, loudly.
    #[test]
    fn fuzz_skipped_when_declared_but_no_baseline() {
        assert_eq!(
            plan_fuzz(Some(TMPL), &inputs(None)),
            FuzzPlan::Skip(
                "differential fuzz needs a pristine baseline binary; run the accept path \
                 (`just verdict`), which rebuilds one"
            )
        );
    }

    #[test]
    fn fuzz_runs_with_both_sides() {
        assert_eq!(
            plan_fuzz(
                Some(TMPL),
                &inputs(Some("targets/cjson/baseline/cjson-bench"))
            ),
            FuzzPlan::Run(
                "python3 scripts/diff-fuzz-cjson.py targets/cjson/fuzz-work 10000 \
                 targets/cjson/baseline/cjson-bench targets/cjson/cand/cjson-bench"
                    .to_string()
            )
        );
    }

    #[test]
    fn fuzz_gate_grades_clean_run_as_pass() {
        let (outcome, iters, diverged) =
            fuzz_gate_outcome("FUZZ-RESULT iters=10000 divergences=0\n", true, Some(0));
        assert_eq!(outcome, GateOutcome::Passed);
        // The ledger records iterations *actually executed*, not the budget.
        assert_eq!(iters, 10_000);
        assert!(!diverged);
    }

    #[test]
    fn fuzz_gate_fails_on_divergence_nonzero_exit_or_no_result_line() {
        // Divergence reported: fail, and the count is still recorded.
        let (outcome, iters, diverged) =
            fuzz_gate_outcome("FUZZ-RESULT iters=900 divergences=3\n", false, Some(1));
        assert!(
            matches!(outcome, GateOutcome::Failed { detail } if detail.contains("divergences=3"))
        );
        assert_eq!(iters, 900);
        assert!(diverged);

        // Zero divergences but a nonzero exit (crash/timeout) is NOT a pass.
        let (outcome, ..) =
            fuzz_gate_outcome("FUZZ-RESULT iters=10 divergences=0\n", false, Some(2));
        assert!(
            matches!(outcome, GateOutcome::Failed { detail } if detail.contains("exit=Some(2)"))
        );

        // A run that never reports the contract line fails closed (e.g. the
        // script died before printing — the old hardcoded-baseline breakage).
        let (outcome, iters, diverged) =
            fuzz_gate_outcome("Traceback (most recent call last):\n", false, Some(1));
        assert_eq!(
            outcome,
            GateOutcome::Failed {
                detail: "fuzz command did not report FUZZ-RESULT".into()
            }
        );
        assert_eq!(iters, 0);
        assert!(!diverged);
    }

    #[test]
    fn fp_tolerance_requires_golden_reference() {
        let err = fp_reference(None).unwrap_err();
        assert_eq!(
            err,
            GateOutcome::Failed {
                detail: "fp-tolerance policy requires corpus.golden_reference in target.toml"
                    .into()
            }
        );
        let path = PathBuf::from("corpora/matmul/GOLDEN.reference");
        assert_eq!(fp_reference(Some(&path)).unwrap(), &path);
    }

    #[test]
    fn fp_golden_outcome_applies_policy() {
        let policy = EquivalencePolicy::FpTolerance {
            abs: 1e-6,
            rel: 1e-5,
        };
        // Last-ULP reordering within tolerance passes.
        assert_eq!(
            fp_golden_outcome(&policy, "1.0000000 2.0", "1.0000001 2.0", true, Some(0)),
            GateOutcome::Passed
        );
        // A genuine wrong value fails with the divergence in the detail.
        assert!(matches!(
            fp_golden_outcome(&policy, "1.0 2.0", "1.0 2.5", true, Some(0)),
            GateOutcome::Failed { detail } if detail.contains("token/offset 1")
        ));
        // A failing replay command fails regardless of output.
        assert!(matches!(
            fp_golden_outcome(&policy, "1.0", "1.0", false, Some(3)),
            GateOutcome::Failed { detail } if detail.contains("exit=Some(3)")
        ));
    }
}
