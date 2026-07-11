use anyhow::Result;
use bench_runner::{config::AcceptConfig, decide, exec, fingerprint::EnvFingerprint, stats};
use clap::{Parser, Subcommand};
use ledger::Verdict;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "bench-runner",
    about = "Interleaved A/B benchmark harness (hotpath trust layer)"
)]
struct Cli {
    /// Path to accept.toml; built-in defaults if omitted.
    #[arg(long)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Interleaved A/B comparison of two shell commands.
    Compare {
        #[arg(long)]
        baseline: String,
        #[arg(long)]
        candidate: String,
    },
    /// A/A self-test: same command both sides; must yield a null verdict.
    Aa {
        #[arg(long)]
        cmd: String,
    },
    /// Automated calibration (SPEC §3.1): N A/A sessions must show a
    /// false-positive rate <5%, and N sessions with a synthetic
    /// slowdown injected into the candidate must detect it ≥95% of the
    /// time. Writes a JSON evidence file.
    Calibrate {
        #[arg(long)]
        cmd: String,
        #[arg(long, default_value_t = 20)]
        sessions: usize,
        /// Injected relative slowdown for the detection test.
        #[arg(long, default_value_t = 0.05)]
        slowdown: f64,
        /// Output JSON evidence path.
        #[arg(long)]
        out: PathBuf,
    },
    /// Service mode (SPEC §3.1 mode c): interleaved A/B of two HTTP
    /// server binaries under a coordinated-omission-correct open-loop
    /// load. Reports p50 and p99 latency speedup (baseline/candidate)
    /// with bootstrap CIs. `--aa` runs the same binary both sides.
    Service {
        #[arg(long)]
        baseline_bin: String,
        #[arg(long)]
        candidate_bin: String,
        #[arg(long)]
        doc: String,
        #[arg(long, default_value_t = 1)]
        iters: u64,
        /// Open-loop arrival rate (requests/sec).
        #[arg(long, default_value_t = 200.0)]
        rate: f64,
        /// Timed requests per session.
        #[arg(long, default_value_t = 1000)]
        count: usize,
        /// Interleaved sessions per side.
        #[arg(long, default_value_t = 12)]
        sessions: usize,
        #[arg(long, default_value_t = 200)]
        warmup: usize,
        #[arg(long, default_value_t = 32)]
        workers: usize,
        /// Prefix pinning the server to a core (e.g. "taskset -c 2").
        #[arg(long, default_value = "")]
        pin: String,
        /// Same binary both sides — must yield a null verdict on p99.
        #[arg(long, default_value_t = false)]
        aa: bool,
        /// Optional JSON output path (latency percentiles + CIs).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Service-mode calibration: A/A false-positive rate + injected
    /// latency-regression detection rate on p50 (SPEC §3.1).
    ServiceCalibrate {
        #[arg(long)]
        server_bin: String,
        #[arg(long)]
        doc: String,
        #[arg(long, default_value_t = 1)]
        iters: u64,
        #[arg(long, default_value_t = 150.0)]
        rate: f64,
        #[arg(long, default_value_t = 500)]
        count: usize,
        /// Interleaved rounds per compare.
        #[arg(long, default_value_t = 6)]
        rounds: usize,
        /// Calibration compares (A/A + injection each).
        #[arg(long, default_value_t = 10)]
        sessions: usize,
        #[arg(long, default_value_t = 100)]
        warmup: usize,
        #[arg(long, default_value_t = 32)]
        workers: usize,
        #[arg(long, default_value = "")]
        pin: String,
        #[arg(long, default_value_t = 0.05)]
        slowdown: f64,
        #[arg(long)]
        out: PathBuf,
    },
}

#[allow(clippy::too_many_arguments)]
fn service_bench(
    cfg: &AcceptConfig,
    baseline_bin: &str,
    candidate_bin: &str,
    doc: &str,
    iters: u64,
    rate: f64,
    count: usize,
    sessions: usize,
    warmup: usize,
    workers: usize,
    pin: &str,
    aa: bool,
    out: &Option<PathBuf>,
) -> Result<()> {
    use bench_runner::service::{run_compare, ServiceCfg};

    let mk = |bin: &str| ServiceCfg {
        server_bin: bin.to_string(),
        doc: doc.to_string(),
        iters,
        rate,
        count,
        warmup,
        workers,
        pin_prefix: pin.to_string(),
        inject_us: None,
    };
    let base_cfg = mk(baseline_bin);
    let cand_cfg = mk(candidate_bin);

    eprintln!(
        "service: {sessions} interleaved sessions/side, {count} req @ {rate} rps, \
         CO-correct open loop, server pin='{pin}'"
    );

    let r = run_compare(
        &base_cfg,
        &cand_cfg,
        sessions,
        cfg.bootstrap_iters,
        cfg.confidence,
        cfg.bootstrap_seed,
        |k, bp50, bp99, cp50, cp99| {
            eprintln!(
                "  round {}/{sessions}: base p50={:.3}ms p99={:.3}ms | cand p50={:.3}ms p99={:.3}ms",
                k + 1,
                bp50 * 1e3,
                bp99 * 1e3,
                cp50 * 1e3,
                cp99 * 1e3
            );
        },
    )?;
    let (ci50, ci99, drop_rate) = (r.ci50, r.ci99, r.drop_rate);
    let (b_p50, c_p50, b_p99, c_p99) = (&r.base_p50, &r.cand_p50, &r.base_p99, &r.cand_p99);

    // Drop rate must be negligible or the load was above capacity and the
    // percentiles are unstable — refuse to report a number we don't trust.
    let total_req = sessions * count * 2;
    let total_dropped = (drop_rate * total_req as f64).round() as usize;
    anyhow::ensure!(
        drop_rate < 0.005,
        "drop rate {:.2}% exceeds 0.5% — load is above server capacity; lower --rate",
        drop_rate * 100.0
    );

    println!(
        "p50 latency speedup (baseline/candidate): median {:.4}, {:.0}% CI [{:.4}, {:.4}]",
        ci50.median,
        cfg.confidence * 100.0,
        ci50.lo,
        ci50.hi
    );
    println!(
        "p99 latency speedup (baseline/candidate): median {:.4}, {:.0}% CI [{:.4}, {:.4}]",
        ci99.median,
        cfg.confidence * 100.0,
        ci99.lo,
        ci99.hi
    );
    println!(
        "drop rate: {:.3}% ({total_dropped}/{total_req})",
        drop_rate * 100.0
    );

    if let Some(path) = out {
        let ev = serde_json::json!({
            "mode": "service-latency",
            "workload": format!("{doc}, {iters} parse+print/req, {rate} rps open-loop, CO-correct"),
            "sessions": sessions, "count_per_session": count, "rate_rps": rate,
            "p50_speedup_median": ci50.median, "p50_speedup_ci": [ci50.lo, ci50.hi],
            "p99_speedup_median": ci99.median, "p99_speedup_ci": [ci99.lo, ci99.hi],
            "baseline_p50_ms_median": stats::median(b_p50) * 1e3,
            "candidate_p50_ms_median": stats::median(c_p50) * 1e3,
            "baseline_p99_ms_median": stats::median(b_p99) * 1e3,
            "candidate_p99_ms_median": stats::median(c_p99) * 1e3,
            "drop_rate": drop_rate,
        });
        std::fs::write(path, serde_json::to_string_pretty(&ev)?)?;
    }

    if aa {
        // Null verdict required on BOTH percentiles.
        for (label, ci) in [("p50", ci50), ("p99", ci99)] {
            if decide(ci, cfg.threshold) == Verdict::Accepted {
                anyhow::bail!(
                    "service A/A FAILED on {label}: speedup claimed from identical servers"
                );
            }
        }
        println!("service A/A self-test passed (null verdict on p50 and p99)");
    } else {
        println!("p50 verdict: {}", decide(ci50, cfg.threshold).as_str());
        println!("p99 verdict: {}", decide(ci99, cfg.threshold).as_str());
    }
    Ok(())
}

/// Service-mode calibration (SPEC §3.1): N A/A compares (false-positive
/// rate on p50 must be <5%) + N injection compares (a per-request
/// busy-wait ≈ `slowdown` of measured service time in the candidate; the
/// harness must resolve the regression — p50 CI entirely < 1 — ≥95%).
#[allow(clippy::too_many_arguments)]
fn service_calibrate(
    cfg: &AcceptConfig,
    server_bin: &str,
    doc: &str,
    iters: u64,
    rate: f64,
    count: usize,
    rounds: usize,
    n: usize,
    warmup: usize,
    workers: usize,
    pin: &str,
    slowdown: f64,
    out: &PathBuf,
) -> Result<()> {
    use bench_runner::service::{run_compare, run_session, ServiceCfg};

    let mk = |inject: Option<u64>| ServiceCfg {
        server_bin: server_bin.to_string(),
        doc: doc.to_string(),
        iters,
        rate,
        count,
        warmup,
        workers,
        pin_prefix: pin.to_string(),
        inject_us: inject,
    };

    // Size the injected busy-wait from a probe of the service's p50.
    let probe = run_session(&mk(None), bench_runner::service::free_port()?)?;
    let p50 = bench_runner::service::percentile(&probe.latencies, 0.50);
    let inject_us = (p50 * slowdown * 1e6).round() as u64;
    eprintln!(
        "service-calibrate: probe p50={:.3}ms, injecting {inject_us}µs (~{:.0}%) for detection",
        p50 * 1e3,
        slowdown * 100.0
    );

    let (mut fp, mut det) = (0usize, 0usize);
    let (mut aa_rows, mut inj_rows) = (Vec::new(), Vec::new());
    for i in 0..n {
        let seed = cfg.bootstrap_seed.wrapping_add(i as u64);
        let aa = run_compare(
            &mk(None),
            &mk(None),
            rounds,
            cfg.bootstrap_iters,
            cfg.confidence,
            seed,
            |_, _, _, _, _| {},
        )?;
        if decide(aa.ci50, cfg.threshold) == Verdict::Accepted {
            fp += 1;
        }
        let inj = run_compare(
            &mk(None),
            &mk(Some(inject_us)),
            rounds,
            cfg.bootstrap_iters,
            cfg.confidence,
            seed,
            |_, _, _, _, _| {},
        )?;
        if inj.ci50.hi < 1.0 {
            det += 1;
        }
        aa_rows.push(serde_json::json!({"lo": aa.ci50.lo, "hi": aa.ci50.hi}));
        inj_rows.push(serde_json::json!({"lo": inj.ci50.lo, "hi": inj.ci50.hi}));
        eprintln!(
            "session {}/{n}: A/A p50 [{:.4},{:.4}] fp={fp}; inj p50 [{:.4},{:.4}] det={det}",
            i + 1,
            aa.ci50.lo,
            aa.ci50.hi,
            inj.ci50.lo,
            inj.ci50.hi
        );
    }
    let fp_rate = fp as f64 / n as f64;
    let det_rate = det as f64 / n as f64;
    let pass = fp_rate < 0.05 && det_rate >= 0.95;
    let ev = serde_json::json!({
        "mode": "service-latency-calibration",
        "server_bin": server_bin, "workload": doc, "rate_rps": rate,
        "count_per_session": count, "rounds_per_compare": rounds, "sessions": n,
        "injected_slowdown": slowdown, "injected_us": inject_us,
        "aa_false_positive_rate": fp_rate, "aa_sessions": aa_rows,
        "injection_detection_rate": det_rate, "injection_sessions": inj_rows,
        "acceptance": {"fp_lt": 0.05, "detection_ge": 0.95, "pass": pass},
    });
    std::fs::write(out, serde_json::to_string_pretty(&ev)?)?;
    println!(
        "service calibration: A/A false-positive {fp_rate:.3} (<0.05), \
         injection detection {det_rate:.3} (>=0.95) -> {}",
        if pass { "PASS" } else { "FAIL" }
    );
    anyhow::ensure!(pass, "service calibration acceptance criteria not met");
    Ok(())
}

fn calibrate(
    cfg: &bench_runner::config::AcceptConfig,
    cmd: &str,
    sessions: usize,
    slowdown: f64,
    out: &PathBuf,
) -> Result<()> {
    let fp = EnvFingerprint::collect("caller-provided (wrap cmd in taskset)", "system-default");
    // Baseline median to size the injected busy-wait.
    let probe = exec::run_interleaved(cmd, cmd, 5, 1, 0.0)?;
    let inject_s = stats::median(&probe.baseline_s) * slowdown;

    let mut aa_sessions = Vec::new();
    let mut inj_sessions = Vec::new();
    let (mut false_positives, mut detections) = (0usize, 0usize);
    for i in 0..sessions {
        // Vary the bootstrap seed per session; resampling with one fixed
        // seed across sessions would correlate the very statistic we are
        // trying to validate.
        let seed = cfg.bootstrap_seed.wrapping_add(i as u64);
        let aa = exec::run_interleaved(cmd, cmd, cfg.runs_per_side, cfg.warmup_runs, 0.0)?;
        let aa_ci = stats::bootstrap_ratio_ci(
            &aa.baseline_s,
            &aa.candidate_s,
            cfg.bootstrap_iters,
            cfg.confidence,
            seed,
        );
        if decide(aa_ci, cfg.threshold) == Verdict::Accepted {
            false_positives += 1;
        }
        aa_sessions
            .push(serde_json::json!({"median": aa_ci.median, "lo": aa_ci.lo, "hi": aa_ci.hi}));

        let inj = exec::run_interleaved(cmd, cmd, cfg.runs_per_side, cfg.warmup_runs, inject_s)?;
        let inj_ci = stats::bootstrap_ratio_ci(
            &inj.baseline_s,
            &inj.candidate_s,
            cfg.bootstrap_iters,
            cfg.confidence,
            seed,
        );
        // Detected = the harness resolves a regression: CI entirely < 1.
        if inj_ci.hi < 1.0 {
            detections += 1;
        }
        inj_sessions
            .push(serde_json::json!({"median": inj_ci.median, "lo": inj_ci.lo, "hi": inj_ci.hi}));
        eprintln!(
            "session {}/{sessions}: A/A [{:.4},{:.4}] fp={false_positives}; inj [{:.4},{:.4}] det={detections}",
            i + 1, aa_ci.lo, aa_ci.hi, inj_ci.lo, inj_ci.hi
        );
    }

    let fp_rate = false_positives as f64 / sessions as f64;
    let det_rate = detections as f64 / sessions as f64;
    let pass = fp_rate < 0.05 && det_rate >= 0.95;
    let evidence = serde_json::json!({
        "workload_cmd": cmd,
        "sessions": sessions,
        "runs_per_side": cfg.runs_per_side,
        "threshold": cfg.threshold,
        "injected_slowdown": slowdown,
        "injected_busy_wait_s": inject_s,
        "aa_false_positive_rate": fp_rate,
        "aa_sessions": aa_sessions,
        "injection_detection_rate": det_rate,
        "injection_sessions": inj_sessions,
        "acceptance": {"fp_lt": 0.05, "detection_ge": 0.95, "pass": pass},
        "env_fingerprint": fp,
    });
    std::fs::write(out, serde_json::to_string_pretty(&evidence)?)?;
    println!(
        "calibration: A/A false-positive rate {fp_rate:.3} (<0.05 required), \
         injected-{:.0}%-slowdown detection rate {det_rate:.3} (>=0.95 required) -> {}",
        slowdown * 100.0,
        if pass { "PASS" } else { "FAIL" }
    );
    anyhow::ensure!(pass, "calibration acceptance criteria not met");
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = match &cli.config {
        Some(p) => AcceptConfig::load(p)?,
        None => AcceptConfig::default(),
    };

    let (baseline, candidate, aa_mode) = match &cli.command {
        Cmd::Compare {
            baseline,
            candidate,
        } => (baseline.clone(), candidate.clone(), false),
        Cmd::Aa { cmd } => (cmd.clone(), cmd.clone(), true),
        Cmd::Calibrate {
            cmd,
            sessions,
            slowdown,
            out,
        } => {
            return calibrate(&cfg, cmd, *sessions, *slowdown, out);
        }
        Cmd::Service {
            baseline_bin,
            candidate_bin,
            doc,
            iters,
            rate,
            count,
            sessions,
            warmup,
            workers,
            pin,
            aa,
            out,
        } => {
            #[allow(clippy::used_underscore_items)]
            return service_bench(
                &cfg,
                baseline_bin,
                candidate_bin,
                doc,
                *iters,
                *rate,
                *count,
                *sessions,
                *warmup,
                *workers,
                pin,
                *aa,
                out,
            );
        }
        Cmd::ServiceCalibrate {
            server_bin,
            doc,
            iters,
            rate,
            count,
            rounds,
            sessions,
            warmup,
            workers,
            pin,
            slowdown,
            out,
        } => {
            return service_calibrate(
                &cfg, server_bin, doc, *iters, *rate, *count, *rounds, *sessions, *warmup,
                *workers, pin, *slowdown, out,
            );
        }
    };

    let fp = EnvFingerprint::collect("none", "system-default");
    eprintln!("env fingerprint: {}", serde_json::to_string(&fp)?);
    eprintln!(
        "plan: {} measured + {} warm-up runs per side, interleaved",
        cfg.runs_per_side, cfg.warmup_runs
    );

    let samples = exec::run_interleaved(
        &baseline,
        &candidate,
        cfg.runs_per_side,
        cfg.warmup_runs,
        0.0,
    )?;
    let ci = stats::bootstrap_ratio_ci(
        &samples.baseline_s,
        &samples.candidate_s,
        cfg.bootstrap_iters,
        cfg.confidence,
        cfg.bootstrap_seed,
    );

    println!(
        "speedup (baseline/candidate): median {:.4}, {:.0}% CI [{:.4}, {:.4}]",
        ci.median,
        cfg.confidence * 100.0,
        ci.lo,
        ci.hi
    );

    if aa_mode {
        // Null verdict required: an "accept" here is a calibration failure.
        let verdict = decide(ci, cfg.threshold);
        if verdict == Verdict::Accepted {
            anyhow::bail!("A/A self-test FAILED: harness claims a speedup from identical binaries");
        }
        println!("A/A self-test passed (null verdict)");
    } else {
        println!("verdict: {}", decide(ci, cfg.threshold).as_str());
    }
    Ok(())
}
