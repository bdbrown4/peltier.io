use anyhow::Result;
use bench_runner::{config::AcceptConfig, decide, exec, fingerprint::EnvFingerprint, stats};
use clap::{Parser, Subcommand};
use ledger::Verdict;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "bench-runner", about = "Interleaved A/B benchmark harness (hotpath trust layer)")]
struct Cli {
    /// Path to accept.toml; built-in defaults if omitted.
    #[arg(long)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Interleaved A/B comparison of two shell commands.
    Compare {
        #[arg(long)]
        baseline: String,
        #[arg(long)]
        candidate: String,
    },
    /// A/A self-test: same command both sides; must yield a null verdict.
    Aa {
        #[arg(long)]
        cmd: String,
    },
    /// Automated calibration (SPEC §3.1): N A/A sessions must show a
    /// false-positive rate <5%, and N sessions with a synthetic
    /// slowdown injected into the candidate must detect it ≥95% of the
    /// time. Writes a JSON evidence file.
    Calibrate {
        #[arg(long)]
        cmd: String,
        #[arg(long, default_value_t = 20)]
        sessions: usize,
        /// Injected relative slowdown for the detection test.
        #[arg(long, default_value_t = 0.05)]
        slowdown: f64,
        /// Output JSON evidence path.
        #[arg(long)]
        out: PathBuf,
    },
}

fn calibrate(
    cfg: &bench_runner::config::AcceptConfig,
    cmd: &str,
    sessions: usize,
    slowdown: f64,
    out: &PathBuf,
) -> Result<()> {
    let fp = EnvFingerprint::collect("caller-provided (wrap cmd in taskset)", "system-default");
    // Baseline median to size the injected busy-wait.
    let probe = exec::run_interleaved(cmd, cmd, 5, 1, 0.0)?;
    let inject_s = stats::median(&probe.baseline_s) * slowdown;

    let mut aa_sessions = Vec::new();
    let mut inj_sessions = Vec::new();
    let (mut false_positives, mut detections) = (0usize, 0usize);
    for i in 0..sessions {
        // Vary the bootstrap seed per session; resampling with one fixed
        // seed across sessions would correlate the very statistic we are
        // trying to validate.
        let seed = cfg.bootstrap_seed.wrapping_add(i as u64);
        let aa = exec::run_interleaved(cmd, cmd, cfg.runs_per_side, cfg.warmup_runs, 0.0)?;
        let aa_ci = stats::bootstrap_ratio_ci(
            &aa.baseline_s, &aa.candidate_s, cfg.bootstrap_iters, cfg.confidence, seed,
        );
        if decide(aa_ci, cfg.threshold) == Verdict::Accepted {
            false_positives += 1;
        }
        aa_sessions.push(serde_json::json!({"median": aa_ci.median, "lo": aa_ci.lo, "hi": aa_ci.hi}));

        let inj =
            exec::run_interleaved(cmd, cmd, cfg.runs_per_side, cfg.warmup_runs, inject_s)?;
        let inj_ci = stats::bootstrap_ratio_ci(
            &inj.baseline_s, &inj.candidate_s, cfg.bootstrap_iters, cfg.confidence, seed,
        );
        // Detected = the harness resolves a regression: CI entirely < 1.
        if inj_ci.hi < 1.0 {
            detections += 1;
        }
        inj_sessions.push(serde_json::json!({"median": inj_ci.median, "lo": inj_ci.lo, "hi": inj_ci.hi}));
        eprintln!(
            "session {}/{sessions}: A/A [{:.4},{:.4}] fp={false_positives}; inj [{:.4},{:.4}] det={detections}",
            i + 1, aa_ci.lo, aa_ci.hi, inj_ci.lo, inj_ci.hi
        );
    }

    let fp_rate = false_positives as f64 / sessions as f64;
    let det_rate = detections as f64 / sessions as f64;
    let pass = fp_rate < 0.05 && det_rate >= 0.95;
    let evidence = serde_json::json!({
        "workload_cmd": cmd,
        "sessions": sessions,
        "runs_per_side": cfg.runs_per_side,
        "threshold": cfg.threshold,
        "injected_slowdown": slowdown,
        "injected_busy_wait_s": inject_s,
        "aa_false_positive_rate": fp_rate,
        "aa_sessions": aa_sessions,
        "injection_detection_rate": det_rate,
        "injection_sessions": inj_sessions,
        "acceptance": {"fp_lt": 0.05, "detection_ge": 0.95, "pass": pass},
        "env_fingerprint": fp,
    });
    std::fs::write(out, serde_json::to_string_pretty(&evidence)?)?;
    println!(
        "calibration: A/A false-positive rate {fp_rate:.3} (<0.05 required), \
         injected-{:.0}%-slowdown detection rate {det_rate:.3} (>=0.95 required) -> {}",
        slowdown * 100.0,
        if pass { "PASS" } else { "FAIL" }
    );
    anyhow::ensure!(pass, "calibration acceptance criteria not met");
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = match &cli.config {
        Some(p) => AcceptConfig::load(p)?,
        None => AcceptConfig::default(),
    };

    let (baseline, candidate, aa_mode) = match &cli.command {
        Cmd::Compare { baseline, candidate } => (baseline.clone(), candidate.clone(), false),
        Cmd::Aa { cmd } => (cmd.clone(), cmd.clone(), true),
        Cmd::Calibrate { cmd, sessions, slowdown, out } => {
            return calibrate(&cfg, cmd, *sessions, *slowdown, out);
        }
    };

    let fp = EnvFingerprint::collect("none", "system-default");
    eprintln!("env fingerprint: {}", serde_json::to_string(&fp)?);
    eprintln!(
        "plan: {} measured + {} warm-up runs per side, interleaved",
        cfg.runs_per_side, cfg.warmup_runs
    );

    let samples =
        exec::run_interleaved(&baseline, &candidate, cfg.runs_per_side, cfg.warmup_runs, 0.0)?;
    let ci = stats::bootstrap_ratio_ci(
        &samples.baseline_s,
        &samples.candidate_s,
        cfg.bootstrap_iters,
        cfg.confidence,
        cfg.bootstrap_seed,
    );

    println!(
        "speedup (baseline/candidate): median {:.4}, {:.0}% CI [{:.4}, {:.4}]",
        ci.median,
        cfg.confidence * 100.0,
        ci.lo,
        ci.hi
    );

    if aa_mode {
        // Null verdict required: an "accept" here is a calibration failure.
        let verdict = decide(ci, cfg.threshold);
        if verdict == Verdict::Accepted {
            anyhow::bail!("A/A self-test FAILED: harness claims a speedup from identical binaries");
        }
        println!("A/A self-test passed (null verdict)");
    } else {
        println!("verdict: {}", decide(ci, cfg.threshold).as_str());
    }
    Ok(())
}
