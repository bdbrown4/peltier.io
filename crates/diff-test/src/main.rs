//! Gate orchestration CLI: corpus pin → upstream tests → golden replay.
//! Differential fuzz and sanitizers remain per-attempt manual steps in
//! Phase 1 and are reported as skipped with a reason.

use anyhow::Result;
use clap::Parser;
use diff_test::orchestrate::run_core_gates;
use diff_test::target::{subst_out, TargetSpec};
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
    /// Binary to test in golden replay. If omitted, the current working
    /// tree is built to targets/<name>/gates-build and that is tested.
    #[arg(long)]
    binary: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cli.root.canonicalize()?;
    let spec = TargetSpec::load(&root, &cli.target)?;
    // build.binary is an {out}-templated path now; resolve it to a real
    // binary either from --binary or by building the current tree into an
    // isolation dir (so `just gates` tests what's on disk, self-contained).
    let binary = match &cli.binary {
        Some(b) => b.clone(),
        None => {
            let out = root
                .join(format!("targets/{}/gates-build", cli.target))
                .to_string_lossy()
                .into_owned();
            println!("building current tree for gates -> {out}");
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(subst_out(&spec.build.baseline, &out))
                .current_dir(&root)
                .status()?;
            anyhow::ensure!(status.success(), "gates build failed");
            subst_out(&spec.build.binary, &out)
        }
    };
    let results = run_core_gates(&root, &spec, &binary)?;
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
