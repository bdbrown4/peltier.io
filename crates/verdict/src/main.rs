//! `just verdict`: the whole attempt pipeline in one command.
//! Gates on the candidate → interleaved A/B bench vs the baseline →
//! verdict by the CI-lower-bound rule → append-only ledger row.
//! Any gate failure is rejected-gate and the bench is skipped; the
//! --needs-human-review flag caps the verdict for FP/concurrency/
//! sanitizer-flagged attempts (SPEC §8) — it never upgrades one.

use anyhow::Result;
use bench_runner::{config::AcceptConfig, decide, exec, fingerprint::EnvFingerprint, stats};
use clap::Parser;
use diff_test::{
    all_passed, orchestrate::run_core_gates, target::TargetSpec, GateLayer, GateOutcome,
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
    let out_dir = root
        .join(format!("targets/{target}/asan"))
        .to_string_lossy()
        .into_owned();
    println!("sanitizers: ASan build of the patched tree…");
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(diff_test::target::subst_out(build_tpl, &out_dir))
        .current_dir(root)
        .status()?;
    anyhow::ensure!(status.success(), "sanitizer build failed");
    let asan_bin = diff_test::target::subst_out(bin_tpl, &out_dir);
    let cmd = spec.bench.command.replace("{binary}", &asan_bin);
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        // detect_leaks is a no-op for the C/C++ UBSan combo but harmless.
        .env("ASAN_OPTIONS", "detect_leaks=1")
        .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1")
        .current_dir(root)
        .output()?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    let clean =
        out.status.success() && !stderr.contains("ERROR:") && !stderr.contains("runtime error:");
    if clean {
        println!("sanitizers: clean");
    } else {
        let lines: Vec<&str> = stderr.lines().filter(|l| !l.is_empty()).collect();
        let start = lines.len().saturating_sub(8);
        println!("sanitizers: FLAGGED (rc={:?})", out.status.code());
        for l in &lines[start..] {
            println!("  {l}");
        }
    }
    Ok(Some(clean))
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

    // Gates run on the candidate binary.
    let gates = run_core_gates(&root, &spec, &cli.candidate_bin)?;
    for (layer, outcome) in &gates {
        match outcome {
            GateOutcome::Passed => println!("{layer:?}: PASS"),
            GateOutcome::Failed { detail } => println!("{layer:?}: FAIL — {detail}"),
            GateOutcome::Skipped { reason } => println!("{layer:?}: skipped — {reason}"),
        }
    }
    let mut gate_results = GateResults {
        upstream_tests: matches!(gates[0].1, GateOutcome::Passed),
        golden_replay: matches!(gates[1].1, GateOutcome::Passed),
        fuzz_iters: 0,
        fuzz_divergence: false,
        sanitizers_clean: false,
    };
    debug_assert!(matches!(gates[0].0, GateLayer::UpstreamTests));

    let pin = &cfg.pin_prefix;
    let wrap = |bin: &str| {
        let cmd = spec.bench.command.replace("{binary}", bin);
        if pin.is_empty() {
            format!("sh -c \"{cmd}\"")
        } else {
            format!("{pin} sh -c \"{cmd}\"")
        }
    };

    let (verdict, bench) = if !all_passed(&gates) {
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
        }
        let fp = EnvFingerprint::collect(pin, "system-default");
        (
            v,
            Some(BenchEvidence {
                baseline_median: bm,
                baseline_ci: (blo, bhi),
                candidate_median: cm,
                candidate_ci: (clo, chi),
                speedup_median: ratio.median,
                speedup_ci: (ratio.lo, ratio.hi),
                env_fingerprint: serde_json::json!({
                    "fingerprint": fp,
                    "workload": spec.bench.workload,
                    "target_commit": spec.source.commit,
                    "gates_detail": "diff-fuzz per-attempt manual; ASan+LSan enforced on the accept path",
                }),
            }),
        )
    };

    let patch = match &cli.patch_file {
        Some(p) => std::fs::read_to_string(p)?,
        None => "(no source patch)".into(),
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
