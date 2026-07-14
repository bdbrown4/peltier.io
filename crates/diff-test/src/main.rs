//! Gate orchestration CLI: corpus + test-suite pins → upstream tests →
//! policy-aware golden replay → differential fuzz. Sanitizers run on the
//! accept path in verdict and are reported as skipped with a reason.
//!
//! Differential fuzz is skipped here too, and for the same structural
//! reason: it differs a *pristine baseline* against the candidate, and this
//! flow builds only the working tree — there is nothing to differ against.
//! The accept path (`just verdict`) rebuilds a pristine baseline, passes it
//! in, and refuses to mint an accept unless the fuzz gate actually passed.

use anyhow::Result;
use clap::Parser;
use diff_test::orchestrate::run_core_gates;
use diff_test::target::{subst_out, TargetSpec};
use diff_test::{GateInputs, GateLayer, GateOutcome};
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
    /// Differential-fuzz iteration budget, substituted for {iters} in the
    /// target's [gates].fuzz command. Inert on this flow: with no pristine
    /// baseline to differ against, the fuzz gate is skipped. The accept path
    /// (`just verdict`) rebuilds one and takes its budget from
    /// config/accept.toml.
    #[arg(long, default_value_t = 10_000)]
    fuzz_iters: u64,
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
    let report = run_core_gates(
        &root,
        &spec,
        &GateInputs {
            candidate_binary: &binary,
            // No pristine baseline on this flow — see the module doc. The
            // fuzz gate reports Skipped with that reason rather than
            // comparing the candidate against itself.
            baseline_binary: None,
            fuzz_iters: cli.fuzz_iters,
        },
    )?;
    for (layer, outcome) in &report.gates {
        match outcome {
            GateOutcome::Passed if *layer == GateLayer::DifferentialFuzz => {
                println!("{layer:?}: PASS ({} iters)", report.fuzz_iters)
            }
            GateOutcome::Passed => println!("{layer:?}: PASS"),
            GateOutcome::Failed { detail } => println!("{layer:?}: FAIL — {detail}"),
            GateOutcome::Skipped { reason } => println!("{layer:?}: skipped — {reason}"),
        }
    }
    if report.all_passed() {
        // Inside all_passed(), the fuzz gate is Passed or Skipped — never
        // Failed. Say so plainly: a PASS here must not read as a verified
        // differential-fuzz run, which is exactly the confusion that let
        // accepted rows carry fuzz_iters=0.
        if !matches!(
            report.outcome(GateLayer::DifferentialFuzz),
            Some(GateOutcome::Passed)
        ) {
            println!(
                "note: differential fuzz did not run — `just gates` is not an accept path; only \
                 `just verdict` (pristine baseline + fuzz gate) can accept a change"
            );
        }
        println!("gates: PASS ({})", cli.target);
        Ok(())
    } else {
        anyhow::bail!("gates: FAIL ({})", cli.target)
    }
}
