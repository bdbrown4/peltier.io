//! Apply a target's equivalence policy to two output files (SPEC §13
//! kernel lane). Loads `targets/<target>/equivalence.toml` (byte-identical
//! by default, or fp-tolerance) and reports whether the candidate output
//! is equivalent to the baseline under it.
//!
//!   fp-compare <target-dir> <baseline-file> <candidate-file>

use anyhow::{Context, Result};
use diff_test::policy::EquivalencePolicy;
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let target_dir = PathBuf::from(
        args.next()
            .context("usage: fp-compare <target-dir> <baseline> <candidate>")?,
    );
    let baseline_path = args.next().context("missing baseline file")?;
    let candidate_path = args.next().context("missing candidate file")?;

    let policy = EquivalencePolicy::load(&target_dir)?;
    let baseline = std::fs::read_to_string(&baseline_path)?;
    let candidate = std::fs::read_to_string(&candidate_path)?;

    println!("policy: {policy:?}");
    match policy.compare(&baseline, &candidate) {
        Ok(()) => {
            println!("EQUIVALENT under policy");
            Ok(())
        }
        Err(d) => {
            println!(
                "DIVERGENT at token/offset {}: baseline={:?} candidate={:?} — {}",
                d.token, d.baseline, d.candidate, d.reason
            );
            std::process::exit(1);
        }
    }
}
