//! Command execution and timing. Whole-program mode (hyperfine-style):
//! spawn the command, measure wall time, discard warm-ups per the
//! schedule. `perf stat` counters and RAPL energy are Phase 1 follow-ups
//! (tracked in README roadmap); wall time alone is enough to bring up
//! A/A calibration.

use crate::schedule::{interleaved, Side};
use anyhow::Context;
use std::process::{Command, Stdio};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct MeasuredSamples {
    pub baseline_s: Vec<f64>,
    pub candidate_s: Vec<f64>,
}

fn time_once(shell_cmd: &str) -> anyhow::Result<f64> {
    let start = Instant::now();
    let status = Command::new("sh")
        .arg("-c")
        .arg(shell_cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("failed to spawn: {shell_cmd}"))?;
    let elapsed = start.elapsed().as_secs_f64();
    anyhow::ensure!(status.success(), "command exited nonzero: {shell_cmd}");
    Ok(elapsed)
}

/// Run baseline and candidate commands on the interleaved schedule and
/// return measured wall times per side. For A/A mode, pass the same
/// command as both sides.
pub fn run_interleaved(
    baseline_cmd: &str,
    candidate_cmd: &str,
    runs_per_side: usize,
    warmup: usize,
) -> anyhow::Result<MeasuredSamples> {
    let mut out = MeasuredSamples {
        baseline_s: Vec::with_capacity(runs_per_side),
        candidate_s: Vec::with_capacity(runs_per_side),
    };
    for (side, measured) in interleaved(runs_per_side, warmup) {
        let cmd = match side {
            Side::Baseline => baseline_cmd,
            Side::Candidate => candidate_cmd,
        };
        let elapsed = time_once(cmd)?;
        if measured {
            match side {
                Side::Baseline => out.baseline_s.push(elapsed),
                Side::Candidate => out.candidate_s.push(elapsed),
            }
        }
    }
    Ok(out)
}
