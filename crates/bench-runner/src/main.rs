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
    };

    let fp = EnvFingerprint::collect("none", "system-default");
    eprintln!("env fingerprint: {}", serde_json::to_string(&fp)?);
    eprintln!(
        "plan: {} measured + {} warm-up runs per side, interleaved",
        cfg.runs_per_side, cfg.warmup_runs
    );

    let samples =
        exec::run_interleaved(&baseline, &candidate, cfg.runs_per_side, cfg.warmup_runs)?;
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
