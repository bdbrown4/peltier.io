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

fn time_once(shell_cmd: &str, inject_spin_s: f64) -> anyhow::Result<f64> {
    let start = Instant::now();
    let status = Command::new("sh")
        .arg("-c")
        .arg(shell_cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("failed to spawn: {shell_cmd}"))?;
    if inject_spin_s > 0.0 {
        // Calibration-only: a real busy-wait inside the timed window, so an
        // injected slowdown flows through the entire measurement pipeline
        // (SPEC §3.1 regression-injection self-test).
        let spin_until = start.elapsed().as_secs_f64() + inject_spin_s;
        while start.elapsed().as_secs_f64() < spin_until {
            std::hint::spin_loop();
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    anyhow::ensure!(status.success(), "command exited nonzero: {shell_cmd}");
    Ok(elapsed)
}

/// Run baseline and candidate commands on the interleaved schedule and
/// return measured wall times per side. For A/A mode, pass the same
/// command as both sides. `inject_candidate_spin_s` adds a synthetic
/// busy-wait to candidate runs only (calibration self-tests; 0.0 in
/// normal operation).
pub fn run_interleaved(
    baseline_cmd: &str,
    candidate_cmd: &str,
    runs_per_side: usize,
    warmup: usize,
    inject_candidate_spin_s: f64,
) -> anyhow::Result<MeasuredSamples> {
    let mut out = MeasuredSamples {
        baseline_s: Vec::with_capacity(runs_per_side),
        candidate_s: Vec::with_capacity(runs_per_side),
    };
    for (side, measured) in interleaved(runs_per_side, warmup) {
        let (cmd, spin) = match side {
            Side::Baseline => (baseline_cmd, 0.0),
            Side::Candidate => (candidate_cmd, inject_candidate_spin_s),
        };
        let elapsed = time_once(cmd, spin)?;
        if measured {
            match side {
                Side::Baseline => out.baseline_s.push(elapsed),
                Side::Candidate => out.candidate_s.push(elapsed),
            }
        }
    }
    Ok(out)
}
