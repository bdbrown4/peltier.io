//! Gate orchestration CLI: corpus pin → upstream tests → golden replay.
//! Differential fuzz and sanitizers remain per-attempt manual steps in
//! Phase 1 and are reported as skipped with a reason.

use anyhow::Result;
use clap::Parser;
use diff_test::orchestrate::run_core_gates;
use diff_test::target::TargetSpec;
use diff_test::{all_passed, GateOutcome};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "diff-test", about = "hotpath equivalence-gate orchestrator")]
struct Cli {
    /// Target name under targets/.
    target: String,
    /// Repository root (defaults to current directory).
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cli.root.canonicalize()?;
    let spec = TargetSpec::load(&root, &cli.target)?;
    let results = run_core_gates(&root, &spec, &spec.build.binary)?;
    for (layer, outcome) in &results {
        match outcome {
            GateOutcome::Passed => println!("{layer:?}: PASS"),
            GateOutcome::Failed { detail } => println!("{layer:?}: FAIL — {detail}"),
            GateOutcome::Skipped { reason } => println!("{layer:?}: skipped — {reason}"),
        }
    }
    if all_passed(&results) {
        println!("gates: PASS ({})", cli.target);
        Ok(())
    } else {
        anyhow::bail!("gates: FAIL ({})", cli.target)
    }
}
