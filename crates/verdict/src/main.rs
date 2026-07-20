//! `just verdict`: the whole attempt pipeline in one command.
//! Gates on the candidate → interleaved A/B bench vs the baseline →
//! verdict by the CI-lower-bound rule → append-only ledger row.
//! Any gate failure is rejected-gate and the bench is skipped. A verdict
//! is only ever capped, never upgraded: the lexical risk classifier
//! (src/risk.rs) and fp-tolerance equivalence route any would-be accept
//! to needs-human-review, and the --needs-human-review flag forces the
//! same cap by hand (SPEC §8).
//!
//! This is also the only flow that can run differential fuzz, because it is
//! the only one holding both sides of the comparison: it rebuilds a pristine
//! baseline and passes it to the gates alongside the candidate. Hard rule —
//! a machine `accepted` requires that fuzz gate to have actually Passed; a
//! skipped or failed one caps the verdict at needs-human-review.

mod risk;

use anyhow::Result;
use bench_runner::{config::AcceptConfig, decide, exec, fingerprint::EnvFingerprint, stats};
use clap::Parser;
use diff_test::{
    orchestrate::run_core_gates, target::TargetSpec, GateInputs, GateLayer, GateOutcome,
};
use ledger::{Attempt, BenchEvidence, GateResults, Ledger, Verdict};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "verdict",
    about = "gates + bench + ledger row for one optimization attempt"
)]
struct Cli {
    /// Target name under targets/.
    target: String,
    /// Pristine baseline binary (repo-root-relative). Prefer
    /// --rebuild-baseline: workspace paths get silently rebuilt from the
    /// patched tree by test runs (observed: phase1-verdict-smoke-001).
    #[arg(long, conflicts_with = "rebuild_baseline")]
    baseline_bin: Option<String>,
    /// Rebuild the baseline from the pristine checkout (patch stashed,
    /// built to targets/<name>/baseline/, patch restored). SPEC §3.1.
    #[arg(long)]
    rebuild_baseline: bool,
    /// Candidate binary (repo-root-relative); also used in golden replay.
    #[arg(long)]
    candidate_bin: String,
    #[arg(long)]
    run_id: String,
    #[arg(long)]
    playbook_class: u8,
    #[arg(long)]
    hypothesis: String,
    #[arg(long)]
    hotspot: String,
    /// Unified-diff file recorded in the ledger (optional for build-config attempts).
    #[arg(long)]
    patch_file: Option<PathBuf>,
    /// Force the needs-human-review verdict tier (FP/concurrency/UB-adjacent).
    #[arg(long)]
    needs_human_review: bool,
    /// How this run was isolated, recorded verbatim in the env
    /// fingerprint (e.g. "no-net.sh"); absent = "unwrapped-host".
    #[arg(long)]
    isolation_note: Option<String>,
    #[arg(long, default_value = "results/ledger.sqlite")]
    db: PathBuf,
    #[arg(long, default_value = "config/accept.toml")]
    config: PathBuf,
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

/// Stash any workspace patch, build the baseline into the isolated
/// targets/<name>/baseline/ dir, restore the patch. Serialized within
/// this process — nothing else may touch the tree while this runs.
fn rebuild_pristine_baseline(
    root: &std::path::Path,
    target: &str,
    spec: &TargetSpec,
) -> Result<String> {
    let ws = format!("targets/{target}/workspace");
    let dirty = !std::process::Command::new("git")
        .args(["-C", &ws, "status", "--porcelain"])
        .output()?
        .stdout
        .is_empty();
    if dirty {
        anyhow::ensure!(
            std::process::Command::new("git")
                .args(["-C", &ws, "stash", "--include-untracked", "-q"])
                .status()?
                .success(),
            "git stash failed"
        );
    }
    let out_dir = root
        .join(format!("targets/{target}/baseline"))
        .to_string_lossy()
        .into_owned();
    let build = std::process::Command::new("sh")
        .arg("-c")
        .arg(diff_test::target::subst_out(&spec.build.baseline, &out_dir))
        .current_dir(root)
        .status();
    if dirty {
        anyhow::ensure!(
            std::process::Command::new("git")
                .args(["-C", &ws, "stash", "pop", "-q"])
                .status()?
                .success(),
            "git stash pop failed — workspace patch may be stranded in stash"
        );
    }
    anyhow::ensure!(build?.success(), "baseline build failed");
    let baseline = diff_test::target::subst_out(&spec.build.binary, &out_dir);
    anyhow::ensure!(
        std::path::Path::new(&baseline).exists(),
        "baseline binary missing: {baseline}"
    );
    println!("baseline rebuilt from pristine checkout: {baseline}");
    Ok(baseline)
}

/// A run is flagged on nonzero exit OR any of these stderr markers.
/// halt_on_error makes sanitizer hits exit nonzero; the markers catch
/// builds whose runtime still exits 0 (and never a bare "ERROR:" from
/// the target's own logging, the old false-flag risk).
const SANITIZER_MARKERS: &[&str] = &[
    "ERROR: AddressSanitizer",
    "ERROR: LeakSanitizer",
    "ERROR: ThreadSanitizer",
    "WARNING: ThreadSanitizer",
    "runtime error:",
];

/// One instrumented build+run lane: templates, isolation subdir, and the
/// halt-on-error runtime options for its sanitizer family.
struct SanLane<'a> {
    label: &'a str,
    out_subdir: &'a str,
    build_tpl: &'a str,
    bin_tpl: &'a str,
    envs: &'a [(&'a str, &'a str)],
}

/// Build one instrumented lane and run the pinned bench workload under
/// it. Returns Some(clean), or None when the instrumented binary cannot
/// be built — an infrastructure failure, not a code defect: the caller
/// caps the accept at needs-human-review instead of crashing a verdict
/// that already carries a measured bench.
fn run_san_lane(
    root: &std::path::Path,
    target: &str,
    spec: &TargetSpec,
    lane: &SanLane,
) -> Result<Option<bool>> {
    let label = lane.label;
    let out_dir = root
        .join(format!("targets/{target}/{}", lane.out_subdir))
        .to_string_lossy()
        .into_owned();
    println!("{label}: instrumented build of the patched tree…");
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(diff_test::target::subst_out(lane.build_tpl, &out_dir))
        .current_dir(root)
        .status()?;
    if !status.success() {
        println!("{label}: BUILD FAILED — cannot verify; capping accept at needs-human-review");
        return Ok(None);
    }
    let bin = diff_test::target::subst_out(lane.bin_tpl, &out_dir);
    let mut run = std::process::Command::new("sh");
    run.arg("-c")
        .arg(spec.bench.command.replace("{binary}", &bin))
        .current_dir(root);
    for (k, v) in lane.envs {
        run.env(k, v);
    }
    let out = run.output()?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    let clean = out.status.success() && !SANITIZER_MARKERS.iter().any(|m| stderr.contains(m));
    if clean {
        println!("{label}: clean");
    } else {
        let lines: Vec<&str> = stderr.lines().filter(|l| !l.is_empty()).collect();
        let start = lines.len().saturating_sub(8);
        println!("{label}: FLAGGED (rc={:?})", out.status.code());
        for l in &lines[start..] {
            println!("  {l}");
        }
    }
    Ok(Some(clean))
}

/// SPEC §8: an accept is final only if ASan+LSan are clean on the patched
/// tree over the pinned workload. Stop-the-line fix after phase2-comrak-010:
/// the pipeline auto-accepted an LSan-flagged teardown patch (sanitizers
/// were per-attempt manual) and the human audit had to overturn it — the
/// accept path now runs the check itself and caps flagged wins at
/// needs-human-review. Rejections skip it: they ship nothing.
/// Returns Some(clean) if the sanitizer gate ran, or None if the target
/// declares no sanitizer build (an accept then cannot be verified and is
/// capped at needs-human-review by the caller).
fn sanitizer_check(
    root: &std::path::Path,
    target: &str,
    spec: &TargetSpec,
) -> Result<Option<bool>> {
    let (Some(build_tpl), Some(bin_tpl)) = (&spec.build.sanitizer, &spec.build.sanitizer_binary)
    else {
        println!("sanitizers: target declares no sanitizer build — cannot verify an accept");
        return Ok(None);
    };
    run_san_lane(
        root,
        target,
        spec,
        &SanLane {
            label: "sanitizers",
            out_subdir: "asan",
            build_tpl,
            bin_tpl,
            envs: &[
                ("ASAN_OPTIONS", "halt_on_error=1:abort_on_error=1"),
                ("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1"),
            ],
        },
    )
}

/// TSan lane (SPEC §8), run on the accept path when the target declares
/// a TSan build — call only when spec.build.tsan is Some. Returns None
/// when the lane cannot run (missing tsan_binary, build failure); the
/// caller caps the accept, mirroring the ASan posture.
fn tsan_check(root: &std::path::Path, target: &str, spec: &TargetSpec) -> Result<Option<bool>> {
    let (Some(build_tpl), Some(bin_tpl)) = (&spec.build.tsan, &spec.build.tsan_binary) else {
        println!("tsan: build.tsan declared without build.tsan_binary — cannot verify");
        return Ok(None);
    };
    run_san_lane(
        root,
        target,
        spec,
        &SanLane {
            label: "tsan",
            out_subdir: "tsan",
            build_tpl,
            bin_tpl,
            envs: &[("TSAN_OPTIONS", "halt_on_error=1")],
        },
    )
}

/// One word for the state of the DifferentialFuzz gate, for the cap message.
fn fuzz_state(outcome: Option<&GateOutcome>) -> &'static str {
    match outcome {
        Some(GateOutcome::Passed) => "passed",
        Some(GateOutcome::Failed { .. }) => "failed",
        Some(GateOutcome::Skipped { .. }) => "skipped",
        None => "not run",
    }
}

/// Median of per-run max-RSS samples (KiB); an even count averages the
/// two middle values.
fn median_u64(xs: &[u64]) -> u64 {
    let mut v = xs.to_vec();
    v.sort_unstable();
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2
    }
}

fn now_utc() -> String {
    let out = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "unknown".to_string(),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cli.root.canonicalize()?;
    std::env::set_current_dir(&root)?;
    let spec = TargetSpec::load(&root, &cli.target)?;
    let cfg = AcceptConfig::load(&root.join(&cli.config))?;
    let started = std::time::Instant::now();

    let baseline_bin = if cli.rebuild_baseline {
        rebuild_pristine_baseline(&root, &cli.target, &spec)?
    } else {
        cli.baseline_bin.clone().ok_or_else(|| {
            anyhow::anyhow!("pass --rebuild-baseline (preferred) or --baseline-bin")
        })?
    };

    let patch = match &cli.patch_file {
        Some(p) => std::fs::read_to_string(p)?,
        None => "(no source patch)".into(),
    };

    // Gates run on the candidate binary — except differential fuzz, which
    // differs it against the pristine baseline built just above. This is the
    // only flow that has both sides, and therefore the only flow that can
    // run the fuzz gate at all (hence the accept rule enforced below).
    let report = run_core_gates(
        &root,
        &spec,
        &GateInputs {
            candidate_binary: &cli.candidate_bin,
            baseline_binary: Some(baseline_bin.as_str()),
            fuzz_iters: cfg.fuzz_iters,
        },
    )?;
    for (layer, outcome) in &report.gates {
        match outcome {
            GateOutcome::Passed => println!("{layer:?}: PASS"),
            GateOutcome::Failed { detail } => println!("{layer:?}: FAIL — {detail}"),
            GateOutcome::Skipped { reason } => println!("{layer:?}: skipped — {reason}"),
        }
    }
    let passed = |layer| matches!(report.outcome(layer), Some(GateOutcome::Passed));
    let mut gate_results = GateResults {
        upstream_tests: passed(GateLayer::UpstreamTests),
        golden_replay: passed(GateLayer::GoldenReplay),
        fuzz_iters: report.fuzz_iters,
        fuzz_divergence: report.fuzz_divergence,
        sanitizers_clean: false,
        tsan_clean: None,
        risk_signals: risk::classify(&patch, report.equivalence_mode == "fp-tolerance"),
        equivalence_mode: Some(report.equivalence_mode.clone()),
    };
    if !gate_results.risk_signals.is_empty() {
        println!(
            "risk: lexical signals [{}] — a would-be accept routes to needs-human-review (SPEC §8)",
            gate_results.risk_signals.join(", ")
        );
    }
    // Announced up front, like the risk signals, so the rule is in the
    // transcript even when another cap fires first below.
    let fuzz_passed = passed(GateLayer::DifferentialFuzz);
    if !fuzz_passed {
        println!(
            "fuzz: differential-fuzz gate {} — a would-be accept routes to needs-human-review; \
             no machine accept without a real differential-fuzz run (SPEC §8)",
            fuzz_state(report.outcome(GateLayer::DifferentialFuzz))
        );
    }

    let pin = &cfg.pin_prefix;
    let wrap = |bin: &str| {
        let cmd = spec.bench.command.replace("{binary}", bin);
        if pin.is_empty() {
            format!("sh -c \"{cmd}\"")
        } else {
            format!("{pin} sh -c \"{cmd}\"")
        }
    };

    let (verdict, bench) = if !report.all_passed() {
        println!("verdict: rejected-gate (bench skipped)");
        (Verdict::RejectedGate, None)
    } else {
        println!(
            "bench: {} measured + {} warm-up runs/side, interleaved, pin='{}'",
            cfg.runs_per_side, cfg.warmup_runs, pin
        );
        let samples = exec::run_interleaved(
            &wrap(&baseline_bin),
            &wrap(&cli.candidate_bin),
            cfg.runs_per_side,
            cfg.warmup_runs,
            0.0,
        )?;
        let ratio = stats::bootstrap_ratio_ci(
            &samples.baseline_s,
            &samples.candidate_s,
            cfg.bootstrap_iters,
            cfg.confidence,
            cfg.bootstrap_seed,
        );
        let (bm, blo, bhi) = stats::bootstrap_median_ci(
            &samples.baseline_s,
            cfg.bootstrap_iters,
            cfg.confidence,
            cfg.bootstrap_seed,
        );
        let (cm, clo, chi) = stats::bootstrap_median_ci(
            &samples.candidate_s,
            cfg.bootstrap_iters,
            cfg.confidence,
            cfg.bootstrap_seed,
        );
        println!(
            "speedup (baseline/candidate): median {:.4}, {:.0}% CI [{:.4}, {:.4}] | workload: {}",
            ratio.median,
            cfg.confidence * 100.0,
            ratio.lo,
            ratio.hi,
            spec.bench.workload
        );
        let mut v = decide(ratio, cfg.threshold);
        if cli.needs_human_review && v == Verdict::Accepted {
            v = Verdict::NeedsHumanReview;
        }
        if v == Verdict::Accepted {
            match sanitizer_check(&root, &cli.target, &spec)? {
                Some(true) => gate_results.sanitizers_clean = true,
                Some(false) => {
                    println!(
                        "verdict: sanitizer flag caps the accept at needs-human-review (SPEC §8)"
                    );
                    v = Verdict::NeedsHumanReview;
                }
                None => {
                    println!("verdict: no sanitizer build declared; capping accept at needs-human-review (SPEC §8)");
                    v = Verdict::NeedsHumanReview;
                }
            }
            if spec.build.tsan.is_some() {
                match tsan_check(&root, &cli.target, &spec)? {
                    Some(clean) => {
                        gate_results.tsan_clean = Some(clean);
                        if !clean {
                            println!(
                                "verdict: TSan flag caps the accept at needs-human-review (SPEC §8)"
                            );
                            v = Verdict::NeedsHumanReview;
                        }
                    }
                    None => {
                        println!("verdict: TSan lane declared but not verifiable; capping accept at needs-human-review (SPEC §8)");
                        v = Verdict::NeedsHumanReview;
                    }
                }
            }
            if v == Verdict::Accepted && !gate_results.risk_signals.is_empty() {
                println!(
                    "verdict: risk signals [{}] route the accept to needs-human-review (SPEC §8)",
                    gate_results.risk_signals.join(", ")
                );
                v = Verdict::NeedsHumanReview;
            }
            // Hard rule (audit remediation): differential fuzz is the gate
            // that catches behavioral divergence the fixed corpus misses, so
            // an accept minted without it is an unverified claim. The audit
            // found accepted rows carrying fuzz_iters=0 — fuzz had run
            // out-of-band, or not at all. A skipped or failed fuzz gate can
            // no longer produce a machine accept, only needs-human-review.
            if v == Verdict::Accepted && !fuzz_passed {
                println!(
                    "verdict: differential fuzz {} — no accept without a real differential-fuzz \
                     run against the pristine baseline; capping at needs-human-review. The target \
                     must declare [gates].fuzz in targets/{}/target.toml, and the run must report \
                     FUZZ-RESULT with zero divergences (SPEC §8)",
                    fuzz_state(report.outcome(GateLayer::DifferentialFuzz)),
                    cli.target
                );
                v = Verdict::NeedsHumanReview;
            }
        }
        let fp = EnvFingerprint::collect(pin, "system-default");
        let mut env_fingerprint = serde_json::json!({
            "fingerprint": fp,
            "workload": spec.bench.workload,
            "target_commit": spec.source.commit,
            "isolation": cli.isolation_note.as_deref().unwrap_or("unwrapped-host"),
            // The accept bar in force for THIS run, recorded so the row is
            // self-contained: `explain` must never guess a historical
            // threshold from today's config (SPEC §3.7).
            "accept_threshold": cfg.threshold,
            "gates_detail": "diff-fuzz gated in-pipeline against the pristine baseline (an accept requires it to have passed); ASan+UBSan enforced on the accept path, TSan when configured",
        });
        if !samples.baseline_max_rss_kib.is_empty() && !samples.candidate_max_rss_kib.is_empty() {
            env_fingerprint["max_rss_kib"] = serde_json::json!({
                "baseline_median": median_u64(&samples.baseline_max_rss_kib),
                "candidate_median": median_u64(&samples.candidate_max_rss_kib),
            });
        }
        (
            v,
            Some(BenchEvidence {
                baseline_median: bm,
                baseline_ci: (blo, bhi),
                candidate_median: cm,
                candidate_ci: (clo, chi),
                speedup_median: ratio.median,
                speedup_ci: (ratio.lo, ratio.hi),
                env_fingerprint,
            }),
        )
    };

    // Derive the phase from the run_id namespace (phaseN-*) so the column
    // matches the run_id prefix; the historical hardcoded `1` left every
    // verdict-written row tagged phase 1 regardless of namespace (found in
    // the phase2-final-audit sweep). Falls back to 1 for un-prefixed ids.
    let phase = cli
        .run_id
        .strip_prefix("phase")
        .and_then(|r| r.split('-').next())
        .and_then(|n| n.parse::<u8>().ok())
        .unwrap_or(1);
    let attempt = Attempt {
        run_id: cli.run_id.clone(),
        timestamp: now_utc(),
        target: cli.target.clone(),
        target_commit: spec.source.commit.clone(),
        phase,
        hotspot: cli.hotspot,
        playbook_class: cli.playbook_class,
        hypothesis: cli.hypothesis,
        patch,
        gates: gate_results,
        bench,
        verdict,
        tokens_spent: 0,
        wall_time_s: started.elapsed().as_secs_f64(),
    };
    Ledger::open(&root.join(&cli.db))?.record(&attempt)?;
    println!(
        "verdict: {} (ledger row {})",
        attempt.verdict.as_str(),
        attempt.run_id
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{fuzz_state, median_u64};
    use diff_test::GateOutcome;

    #[test]
    fn median_u64_odd_and_even() {
        assert_eq!(median_u64(&[5]), 5);
        assert_eq!(median_u64(&[9, 1, 5]), 5);
        assert_eq!(median_u64(&[4, 2, 8, 6]), 5);
    }

    /// Only `Passed` clears the accept bar; every other state (including a
    /// gate that never ran) is a cap to needs-human-review.
    #[test]
    fn fuzz_state_names_every_outcome() {
        assert_eq!(fuzz_state(Some(&GateOutcome::Passed)), "passed");
        assert_eq!(
            fuzz_state(Some(&GateOutcome::Failed {
                detail: "divergences=3".into()
            })),
            "failed"
        );
        assert_eq!(
            fuzz_state(Some(&GateOutcome::Skipped {
                reason: "no baseline".into()
            })),
            "skipped"
        );
        assert_eq!(fuzz_state(None), "not run");
    }
}
