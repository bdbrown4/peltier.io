//! Command execution and timing. Whole-program mode (hyperfine-style):
//! spawn the command, measure wall time, discard warm-ups per the
//! schedule. Max RSS is captured per run via `wait4` on Unix; opt-in
//! `perf stat` counters live in `counters` (diagnostics only).

use crate::schedule::{interleaved, Side};
use anyhow::Context;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct MeasuredSamples {
    pub baseline_s: Vec<f64>,
    pub candidate_s: Vec<f64>,
    /// Per-run max resident set size, normalized to KiB from `wait4`
    /// ru_maxrss (whose own unit is platform-defined — see
    /// `maxrss_to_kib`). Empty on non-Unix hosts.
    pub baseline_max_rss_kib: Vec<u64>,
    pub candidate_max_rss_kib: Vec<u64>,
}

/// Raw `ru_maxrss` → KiB.
///
/// The unit of `ru_maxrss` is *platform-defined*, and the platforms
/// disagree: Linux and the BSDs report kilobytes, while Apple platforms
/// (macOS et al.) report **bytes**. Taking the raw value as KiB everywhere
/// — as this did — silently inflates every macOS reading by 1024×, and the
/// number is not cosmetic: verdict records its median as
/// `env_fingerprint.max_rss_kib` in the ledger, where an allocator-swap
/// attempt is judged partly on memory. Everything downstream is KiB, so the
/// raw value is normalized here, at the syscall edge.
///
/// `raw_is_bytes` is the platform predicate (passed at the one call site, so
/// this stays a pure function testable on every host). Bytes round *up*: a
/// nonzero RSS must never record as 0 KiB. Negative values — a kernel that
/// reports nothing — clamp to 0 rather than wrapping into the exabytes.
#[cfg(any(unix, test))]
fn maxrss_to_kib(raw: i64, raw_is_bytes: bool) -> u64 {
    let raw = raw.max(0) as u64;
    if raw_is_bytes {
        raw.div_ceil(1024)
    } else {
        raw
    }
}

/// Reap the child, returning its exit status and max RSS in KiB when the
/// platform reports it.
#[cfg(unix)]
fn wait_with_rusage(child: &mut Child) -> anyhow::Result<(ExitStatus, Option<u64>)> {
    use std::os::unix::process::ExitStatusExt;
    let pid = child.id() as libc::pid_t;
    let mut status: libc::c_int = 0;
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    loop {
        let r = unsafe { libc::wait4(pid, &mut status, 0, &mut usage) };
        if r == pid {
            break;
        }
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::EINTR) {
            return Err(err).context("wait4 failed");
        }
    }
    // Apple's getrusage reports ru_maxrss in bytes; Linux and the BSDs in KiB.
    let max_rss_kib = maxrss_to_kib(usage.ru_maxrss as i64, cfg!(target_vendor = "apple"));
    Ok((ExitStatus::from_raw(status), Some(max_rss_kib)))
}

#[cfg(not(unix))]
fn wait_with_rusage(child: &mut Child) -> anyhow::Result<(ExitStatus, Option<u64>)> {
    Ok((child.wait()?, None))
}

fn time_once(shell_cmd: &str, inject_spin_s: f64) -> anyhow::Result<(f64, Option<u64>)> {
    let start = Instant::now();
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(shell_cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to spawn: {shell_cmd}"))?;
    let (status, max_rss_kib) = wait_with_rusage(&mut child)?;
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
    Ok((elapsed, max_rss_kib))
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
        baseline_max_rss_kib: Vec::new(),
        candidate_max_rss_kib: Vec::new(),
    };
    for (side, measured) in interleaved(runs_per_side, warmup) {
        let (cmd, spin) = match side {
            Side::Baseline => (baseline_cmd, 0.0),
            Side::Candidate => (candidate_cmd, inject_candidate_spin_s),
        };
        let (elapsed, max_rss_kib) = time_once(cmd, spin)?;
        if measured {
            match side {
                Side::Baseline => {
                    out.baseline_s.push(elapsed);
                    out.baseline_max_rss_kib.extend(max_rss_kib);
                }
                Side::Candidate => {
                    out.candidate_s.push(elapsed);
                    out.candidate_max_rss_kib.extend(max_rss_kib);
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::maxrss_to_kib;

    #[test]
    fn maxrss_kib_passthrough_when_platform_reports_kib() {
        // Linux / BSD: already KiB.
        assert_eq!(maxrss_to_kib(2048, false), 2048);
        assert_eq!(maxrss_to_kib(0, false), 0);
    }

    #[test]
    fn maxrss_bytes_are_converted_not_recorded_raw() {
        // Apple: bytes. 2 MiB is 2048 KiB, not 2_097_152 — the 1024×
        // inflation this guards against.
        assert_eq!(maxrss_to_kib(2_097_152, true), 2048);
        // Rounds up: a nonzero RSS never records as 0 KiB.
        assert_eq!(maxrss_to_kib(1, true), 1);
        assert_eq!(maxrss_to_kib(1025, true), 2);
        assert_eq!(maxrss_to_kib(0, true), 0);
    }

    #[test]
    fn maxrss_negative_clamps_instead_of_wrapping() {
        assert_eq!(maxrss_to_kib(-1, false), 0);
        assert_eq!(maxrss_to_kib(-1, true), 0);
    }
}
