//! Service-mode latency bench (SPEC §3.1 mode (c)). A coordinated-
//! omission-CORRECT, open-loop, fixed-rate load generator: requests are
//! scheduled at `start + i/rate` and each latency is measured from its
//! *intended* send time, not its actual send time. So when the server
//! falls behind, the requests that pile up behind it each carry the full
//! queueing delay a real client would see — the exact error (coordinated
//! omission) that closed-loop tools like `ab` hide. We store every
//! latency and take exact percentiles (no HdrHistogram bucketing error;
//! the sample is bounded).
//!
//! Interleaved A/B, like the wall-time bench: each session yields one
//! percentile (p50/p99) per side; the existing bootstrap ratio-CI machine
//! then works unchanged, treating "p99 latency" the way it treats "wall
//! time" — lower is better, speedup = baseline / candidate.

use anyhow::{anyhow, ensure, Result};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::stats::{self, RatioCi};

pub struct ServiceCfg {
    pub server_bin: String,
    pub doc: String,
    pub iters: u64,
    pub rate: f64,
    pub count: usize,
    pub warmup: usize,
    pub workers: usize,
    /// Prefix that pins the SERVER to a core (e.g. `taskset -c 2`); the
    /// load generator runs on the other cores.
    pub pin_prefix: String,
    /// Calibration only: inject this many µs of busy-wait per request
    /// (via HOTPATH_INJECT_US) to validate the harness detects a known
    /// latency regression. None in every real run.
    pub inject_us: Option<u64>,
}

/// Result of one interleaved A/B service compare.
pub struct CompareResult {
    pub ci50: RatioCi,
    pub ci99: RatioCi,
    pub base_p50: Vec<f64>,
    pub cand_p50: Vec<f64>,
    pub base_p99: Vec<f64>,
    pub cand_p99: Vec<f64>,
    pub drop_rate: f64,
}

/// Interleaved A/B: `rounds` alternating baseline/candidate sessions,
/// each yielding one p50 and one p99; bootstrap ratio-CI over the rounds.
#[allow(clippy::too_many_arguments)]
pub fn run_compare(
    base: &ServiceCfg,
    cand: &ServiceCfg,
    rounds: usize,
    bootstrap_iters: usize,
    confidence: f64,
    seed: u64,
    mut on_round: impl FnMut(usize, f64, f64, f64, f64),
) -> Result<CompareResult> {
    let (mut b50, mut b99, mut c50, mut c99) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut dropped = 0usize;
    for k in 0..rounds {
        let bs = run_session(base, free_port()?)?;
        let cs = run_session(cand, free_port()?)?;
        dropped += bs.dropped + cs.dropped;
        let (bp50, bp99) = (
            percentile(&bs.latencies, 0.50),
            percentile(&bs.latencies, 0.99),
        );
        let (cp50, cp99) = (
            percentile(&cs.latencies, 0.50),
            percentile(&cs.latencies, 0.99),
        );
        b50.push(bp50);
        b99.push(bp99);
        c50.push(cp50);
        c99.push(cp99);
        on_round(k, bp50, bp99, cp50, cp99);
    }
    let total = rounds * base.count * 2;
    let drop_rate = dropped as f64 / total as f64;
    let ci50 = stats::bootstrap_ratio_ci(&b50, &c50, bootstrap_iters, confidence, seed);
    let ci99 = stats::bootstrap_ratio_ci(&b99, &c99, bootstrap_iters, confidence, seed);
    Ok(CompareResult {
        ci50,
        ci99,
        base_p50: b50,
        cand_p50: c50,
        base_p99: b99,
        cand_p99: c99,
        drop_rate,
    })
}

pub struct SessionResult {
    pub latencies: Vec<f64>,
    pub dropped: usize,
}

/// Exact percentile (0.0–1.0) of a sorted-in-place copy.
pub fn percentile(latencies: &[f64], q: f64) -> f64 {
    if latencies.is_empty() {
        return f64::NAN;
    }
    let mut xs = latencies.to_vec();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((xs.len() as f64 - 1.0) * q).round() as usize;
    xs[idx.min(xs.len() - 1)]
}

fn spawn_server(cfg: &ServiceCfg, port: u16) -> Result<Child> {
    let inject = match cfg.inject_us {
        Some(us) => format!("HOTPATH_INJECT_US={us} "),
        None => String::new(),
    };
    let cmd = format!(
        "{inject}{pin} {bin} {port} {doc} {iters}",
        pin = cfg.pin_prefix,
        bin = cfg.server_bin,
        doc = cfg.doc,
        iters = cfg.iters,
    );
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd.trim())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    // Block until the server prints "READY <port>".
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("no server stdout"))?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    ensure!(
        line.starts_with("READY"),
        "server did not signal READY (got {line:?})"
    );
    Ok(child)
}

#[cfg(unix)]
fn set_linger_zero(s: &TcpStream) {
    // SO_LINGER with a 0 timeout → RST on close instead of FIN, so the
    // tens of thousands of short client connections a load session opens
    // don't pile up in TIME_WAIT and exhaust the ephemeral port range.
    // std::net::TcpStream::set_linger is still unstable, so go through
    // setsockopt directly. The response is fully read before close, so the
    // RST is harmless.
    use std::os::unix::io::AsRawFd;
    let l = libc::linger {
        l_onoff: 1,
        l_linger: 0,
    };
    unsafe {
        libc::setsockopt(
            s.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_LINGER,
            &l as *const libc::linger as *const libc::c_void,
            std::mem::size_of::<libc::linger>() as libc::socklen_t,
        );
    }
}

#[cfg(not(unix))]
fn set_linger_zero(_s: &TcpStream) {
    // Service mode is POSIX-only at runtime (`sh`, taskset); this stub
    // only keeps the crate compiling on non-Unix hosts.
}

fn one_request(port: u16) -> std::io::Result<()> {
    let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port))?;
    s.set_nodelay(true)?;
    set_linger_zero(&s);
    s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")?;
    let mut buf = [0u8; 512];
    // Read the full response (server sends Content-Length + Connection: close).
    loop {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// One open-loop load session against a freshly spawned server.
pub fn run_session(cfg: &ServiceCfg, port: u16) -> Result<SessionResult> {
    ensure!(
        cfg.rate.is_finite() && cfg.rate > 0.0,
        "arrival rate must be finite and positive (got {})",
        cfg.rate
    );
    ensure!(
        cfg.count > 0 && cfg.workers > 0,
        "count and workers must be > 0"
    );
    let mut child = spawn_server(cfg, port)?;
    // Warm up closed-loop (untimed) so the first timed request doesn't eat
    // page faults / cold caches.
    for _ in 0..cfg.warmup {
        let _ = one_request(port);
    }

    let period = Duration::from_secs_f64(1.0 / cfg.rate);
    let start = Instant::now();
    let idx = Arc::new(AtomicUsize::new(0));
    let out = Arc::new(Mutex::new((Vec::<f64>::with_capacity(cfg.count), 0usize)));
    let count = cfg.count;

    let mut handles = Vec::new();
    for _ in 0..cfg.workers {
        let idx = idx.clone();
        let out = out.clone();
        handles.push(std::thread::spawn(move || {
            let mut local = Vec::new();
            let mut dropped = 0usize;
            loop {
                let i = idx.fetch_add(1, Ordering::Relaxed);
                if i >= count {
                    break;
                }
                let target = start + period * (i as u32);
                let now = Instant::now();
                if now < target {
                    std::thread::sleep(target - now);
                }
                // Latency from the INTENDED send time — the CO correction.
                match one_request(port) {
                    Ok(()) => local.push(Instant::now().duration_since(target).as_secs_f64()),
                    Err(_) => dropped += 1,
                }
            }
            let mut g = out.lock().unwrap();
            g.0.extend(local);
            g.1 += dropped;
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    let _ = child.kill();
    let _ = child.wait();

    let (latencies, dropped) = Arc::try_unwrap(out).unwrap().into_inner().unwrap();
    Ok(SessionResult { latencies, dropped })
}

/// Pick a free loopback TCP port (small race between probe-close and the
/// server bind; harmless on loopback with SO_REUSEADDR).
pub fn free_port() -> Result<u16> {
    let l = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    Ok(l.local_addr()?.port())
}
