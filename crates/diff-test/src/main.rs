//! Gate orchestration: corpus pin → upstream tests → golden replay.
//! Refuses to run anything if the corpus manifest mismatches (SPEC §3.2).
//! Differential fuzz and sanitizers remain per-attempt manual steps in
//! Phase 1 and are reported as skipped with a reason.

use anyhow::Result;
use clap::Parser;
use diff_test::target::{expected_golden_hash, TargetSpec};
use diff_test::{all_passed, pin, GateLayer, GateOutcome};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "diff-test", about = "hotpath equivalence-gate orchestrator")]
struct Cli {
    /// Target name under targets/.
    target: String,
    /// Repository root (defaults to current directory).
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

fn sh(root: &PathBuf, cmd: &str) -> Result<std::process::Output> {
    Ok(Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cli.root.canonicalize()?;
    let spec = TargetSpec::load(&root, &cli.target)?;
    let mut results: Vec<(GateLayer, GateOutcome)> = Vec::new();

    // Corpus pin is a precondition, not a gate: mismatch = refuse to run.
    let verified = pin::verify_manifest(&root.join(&spec.corpus.manifest), &root.join(&spec.corpus.root))?;
    println!("corpus pin: {} files verified against {}", verified, spec.corpus.manifest.display());

    print!("upstream tests ... ");
    let t = sh(&root, &spec.gates.tests)?;
    let tests_outcome = if t.status.success() {
        println!("PASS");
        GateOutcome::Passed
    } else {
        println!("FAIL");
        GateOutcome::Failed {
            detail: String::from_utf8_lossy(&t.stderr).chars().take(2000).collect(),
        }
    };
    results.push((GateLayer::UpstreamTests, tests_outcome));

    print!("golden replay ... ");
    let golden_cmd = spec.gates.golden.replace("{binary}", &spec.build.binary);
    let g = sh(&root, &golden_cmd)?;
    let expected = expected_golden_hash(&root.join(&spec.corpus.golden_sha256))?;
    let actual = format!("{:x}", Sha256::digest(&g.stdout));
    let golden_outcome = if g.status.success() && actual == expected {
        println!("PASS (stdout sha256 = pinned golden)");
        GateOutcome::Passed
    } else {
        println!("FAIL");
        GateOutcome::Failed {
            detail: format!("exit={:?} expected={expected} actual={actual}", g.status.code()),
        }
    };
    results.push((GateLayer::GoldenReplay, golden_outcome));

    results.push((
        GateLayer::DifferentialFuzz,
        GateOutcome::Skipped { reason: "per-attempt manual step in Phase 1 (needs old/new pair)".into() },
    ));
    results.push((
        GateLayer::Sanitizers,
        GateOutcome::Skipped { reason: "per-attempt manual step in Phase 1 (nightly toolchain)".into() },
    ));

    for (layer, outcome) in &results {
        if let GateOutcome::Skipped { reason } = outcome {
            println!("{layer:?}: skipped — {reason}");
        }
    }
    if all_passed(&results) {
        println!("gates: PASS ({})", cli.target);
        Ok(())
    } else {
        anyhow::bail!("gates: FAIL ({})", cli.target)
    }
}
