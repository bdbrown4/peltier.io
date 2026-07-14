//! Opt-in `perf stat` PMU counter capture. Diagnostics only — counters
//! never feed the accept decision (wall time does, SPEC §3.1). Each
//! timed run is wrapped `perf stat -x, -e <events> -o <tmpfile> -- sh -c
//! <cmd>`; the CSV file is parsed after every run and per-side medians
//! are printed with the standard caveats. Unix-only at runtime; the
//! parser is portable and unit-tested.

use crate::exec::MeasuredSamples;
use crate::schedule::{interleaved, Side};
use crate::stats;
use anyhow::Context;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

pub const PERF_EVENTS: [&str; 4] = ["cycles", "instructions", "cache-misses", "branch-misses"];

/// One side's counter samples: `samples[i]` holds one value per measured
/// run for `PERF_EVENTS[i]`; runs where perf reported `<not counted>` or
/// `<not supported>` contribute nothing.
#[derive(Debug, Clone, Default)]
pub struct PerfSide {
    pub samples: [Vec<u64>; 4],
}

#[derive(Debug, Clone, Default)]
pub struct PerfReport {
    pub baseline: PerfSide,
    pub candidate: PerfSide,
}

/// Parse `perf stat -x,` CSV: field 0 is the counter value, field 2 the
/// event name (perf may append a modifier, e.g. `cycles:u`). Comment
/// lines, blank lines, and non-numeric values (`<not counted>`,
/// `<not supported>`) are skipped. Returns (event, count) in file order.
pub fn parse_perf_csv(text: &str) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split(',');
        let value = fields.next().unwrap_or("");
        let _unit = fields.next();
        let Some(event) = fields.next() else {
            continue;
        };
        let Ok(v) = value.trim().parse::<u64>() else {
            continue;
        };
        out.push((event.trim().to_string(), v));
    }
    out
}

/// Index into PERF_EVENTS, tolerating a perf modifier suffix
/// (`cycles:u` when perf_event_paranoid restricts kernel counting).
fn event_index(event: &str) -> Option<usize> {
    PERF_EVENTS.iter().position(|name| {
        event == *name || (event.starts_with(name) && event[name.len()..].starts_with(':'))
    })
}

impl PerfSide {
    fn absorb(&mut self, parsed: &[(String, u64)]) {
        for (event, value) in parsed {
            if let Some(i) = event_index(event) {
                self.samples[i].push(*value);
            }
        }
    }
}

impl PerfReport {
    /// Per-side medians, each with its command (workload) and run count.
    pub fn print(&self, baseline_cmd: &str, candidate_cmd: &str) {
        println!("perf-stat per-side medians:");
        for (label, cmd, side) in [
            ("baseline ", baseline_cmd, &self.baseline),
            ("candidate", candidate_cmd, &self.candidate),
        ] {
            let cols: Vec<String> = PERF_EVENTS
                .iter()
                .zip(&side.samples)
                .map(|(name, xs)| {
                    if xs.is_empty() {
                        format!("{name}=n/a")
                    } else {
                        let v: Vec<f64> = xs.iter().map(|&x| x as f64).collect();
                        format!(
                            "{name}={:.0} (median of {} runs)",
                            stats::median(&v),
                            xs.len()
                        )
                    }
                })
                .collect();
            println!("  {label} `{cmd}`: {}", cols.join(", "));
        }
        println!(
            "  note: PMU counters unavailable in most VMs — flag is opt-in diagnostics, \
             not part of the accept decision"
        );
    }
}

fn time_once_perf(shell_cmd: &str, out_file: &Path) -> anyhow::Result<(f64, Vec<(String, u64)>)> {
    let start = Instant::now();
    let status = Command::new("perf")
        .args(["stat", "-x,", "-e", &PERF_EVENTS.join(",")])
        .arg("-o")
        .arg(out_file)
        .args(["--", "sh", "-c", shell_cmd])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("failed to spawn perf stat for: {shell_cmd}"))?;
    let elapsed = start.elapsed().as_secs_f64();
    anyhow::ensure!(
        status.success(),
        "perf stat run exited nonzero: {shell_cmd}"
    );
    let text = std::fs::read_to_string(out_file)
        .with_context(|| format!("perf stat output missing: {}", out_file.display()))?;
    Ok((elapsed, parse_perf_csv(&text)))
}

/// Interleaved A/B with each timed run wrapped in `perf stat`. The perf
/// wrapper overhead lands on both sides identically, but this path is
/// diagnostics only — verdicts always measure through the unwrapped
/// `exec::run_interleaved`.
pub fn run_interleaved_perf(
    baseline_cmd: &str,
    candidate_cmd: &str,
    runs_per_side: usize,
    warmup: usize,
) -> anyhow::Result<(MeasuredSamples, PerfReport)> {
    anyhow::ensure!(cfg!(unix), "--perf-stat requires a Unix host with perf(1)");
    let tmp = std::env::temp_dir().join(format!("bench-runner-perf-{}.csv", std::process::id()));
    let mut samples = MeasuredSamples {
        baseline_s: Vec::with_capacity(runs_per_side),
        candidate_s: Vec::with_capacity(runs_per_side),
        baseline_max_rss_kib: Vec::new(),
        candidate_max_rss_kib: Vec::new(),
    };
    let mut report = PerfReport::default();
    for (side, measured) in interleaved(runs_per_side, warmup) {
        let cmd = match side {
            Side::Baseline => baseline_cmd,
            Side::Candidate => candidate_cmd,
        };
        let (elapsed, parsed) = time_once_perf(cmd, &tmp)?;
        if measured {
            match side {
                Side::Baseline => {
                    samples.baseline_s.push(elapsed);
                    report.baseline.absorb(&parsed);
                }
                Side::Candidate => {
                    samples.candidate_s.push(elapsed);
                    report.candidate.absorb(&parsed);
                }
            }
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Ok((samples, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANNED: &str = "\
# started on Mon Jul 13 12:00:00 2026

123456789,,cycles,401000000,100.00,,
234567890,,instructions:u,401000000,100.00,1.90,insn per cycle
<not supported>,,cache-misses,0,100.00,,
<not counted>,,branch-misses,0,0.00,,
garbage line without commas
999,,unrelated-event,1,100.00,,
";

    #[test]
    fn parses_values_and_skips_junk() {
        let parsed = parse_perf_csv(CANNED);
        assert_eq!(
            parsed,
            vec![
                ("cycles".to_string(), 123_456_789),
                ("instructions:u".to_string(), 234_567_890),
                ("unrelated-event".to_string(), 999),
            ]
        );
    }

    #[test]
    fn event_index_tolerates_modifier_suffix_only() {
        assert_eq!(event_index("cycles"), Some(0));
        assert_eq!(event_index("instructions:u"), Some(1));
        assert_eq!(event_index("cache-misses"), Some(2));
        assert_eq!(event_index("cycle"), None);
        assert_eq!(event_index("cycles_total"), None);
        assert_eq!(event_index("unrelated-event"), None);
    }

    #[test]
    fn absorb_buckets_by_event() {
        let mut side = PerfSide::default();
        side.absorb(&parse_perf_csv(CANNED));
        assert_eq!(side.samples[0], vec![123_456_789]);
        assert_eq!(side.samples[1], vec![234_567_890]);
        assert!(side.samples[2].is_empty(), "<not supported> is skipped");
        assert!(side.samples[3].is_empty(), "<not counted> is skipped");
    }
}
