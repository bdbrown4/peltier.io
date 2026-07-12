//! `just report <run-id>` — the pitch artifact (SPEC §9). Reads a ledger
//! row (throughput speedup + CI) and, optionally, a service-latency JSON
//! (percentile speedups + CIs), and renders ONE mechanical report:
//! throughput → cores → dollars/year and/or latency percentile deltas,
//! every figure with its 95% CI and workload, the methodology printed
//! inline so the number survives hostile review. No hand-editing: the
//! numbers come only from the ledger and the calibrated bench.

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use report::{pricing::Pricing, roi_from_speedup_ci};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "report",
    about = "Mechanical ROI report from a ledger row (SPEC §9)"
)]
struct Cli {
    /// Ledger run_id to report on (throughput ROI).
    #[arg(long)]
    run_id: String,
    /// Fleet size the workload runs on, in cores (engagement input).
    #[arg(long, default_value_t = 1000.0)]
    fleet_cores: f64,
    /// Optional service-latency JSON (from `bench-runner service --out`)
    /// to add the latency-percentile ROI section.
    #[arg(long)]
    service_json: Option<PathBuf>,
    #[arg(long, default_value = "results/ledger.sqlite")]
    db: PathBuf,
    #[arg(long, default_value = "config/pricing.toml")]
    pricing: PathBuf,
    /// Output markdown path; stdout if omitted.
    #[arg(long)]
    out: Option<PathBuf>,
}

fn fmt_usd(x: f64) -> String {
    if x.abs() >= 1000.0 {
        format!("${:.0}", x)
    } else {
        format!("${:.2}", x)
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ledger = ledger::Ledger::open(&cli.db)?;
    let row = ledger
        .report_row(&cli.run_id)?
        .ok_or_else(|| anyhow!("no ledger row for run_id {}", cli.run_id))?;
    let pricing = Pricing::load(&cli.pricing).context("load pricing.toml")?;

    let target = row["target"].as_str().unwrap_or("?");
    let verdict = row["verdict"].as_str().unwrap_or("?");
    let class = row["playbook_class"].as_u64().unwrap_or(0);
    let workload = row["workload"]
        .as_str()
        .unwrap_or("(workload not recorded)");
    let speedup = row["speedup_median"].as_f64();
    let ci = row["speedup_ci"]
        .as_array()
        .and_then(|a| Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)));

    let mut md = String::new();
    md.push_str(&format!("# ROI report — `{}`\n\n", cli.run_id));
    md.push_str(&format!(
        "**Target:** {target} · **Playbook class:** {class} · **Verdict:** `{verdict}`\n\n\
         **Workload:** {workload}\n\n",
    ));
    // A report for anything the pipeline did not accept must say so before
    // any dollar figure — the ROI below is a measured effect, not a
    // shippable saving.
    if verdict != "accepted" {
        md.push_str(&format!(
            "> ⚠️ **Not shipped.** This attempt's verdict is `{verdict}`, not `accepted`. Any ROI\n\
             > below is the *measured* effect — it did not clear the significance/tier bar and is\n\
             > **not** a committed saving. Shown for completeness; the ledger records it as-is.\n\n",
        ));
    }

    match (speedup, ci) {
        (Some(sp), Some((lo, hi))) => {
            md.push_str("## Throughput → cores → dollars\n\n");
            md.push_str(&format!(
                "Measured speedup (baseline/candidate): **{:.4}**, 95% bootstrap CI **[{:.4}, {:.4}]**.\n\n",
                sp, lo, hi
            ));
            let roi = roi_from_speedup_ci(
                cli.fleet_cores,
                sp,
                (lo, hi),
                pricing.dollars_per_core_hour,
                pricing.hours_per_year,
            );
            md.push_str(&format!(
                "On a **{:.0}-core** fleet running this workload:\n\n\
                 | metric | median | 95% CI |\n|---|---|---|\n\
                 | cores returned | {:.1} | [{:.1}, {:.1}] |\n\
                 | annualized saving | {} | [{}, {}] |\n\n",
                cli.fleet_cores,
                roi.cores_median,
                roi.cores_lo,
                roi.cores_hi,
                fmt_usd(roi.dollars_median),
                fmt_usd(roi.dollars_lo),
                fmt_usd(roi.dollars_hi),
            ));
            md.push_str(&format!(
                "Pricing: **{}/core-hour**, **{:.0} h/year**. Source: _{}_\n\n\
                 The **CI lower bound is the number to quote** — {:.1} cores / {} per year is\n\
                 the saving that survives the 95% interval; the point estimate is not a promise.\n\n",
                fmt_usd(pricing.dollars_per_core_hour), pricing.hours_per_year, pricing.rate_source,
                roi.cores_lo, fmt_usd(roi.dollars_lo),
            ));
        }
        _ => {
            md.push_str(&format!(
                "## Throughput\n\nNo bench speedup recorded (verdict `{verdict}`); this attempt\n\
                 produced no throughput ROI. The row is in the ledger as a complete outcome.\n\n"
            ));
        }
    }

    if let Some(path) = &cli.service_json {
        let svc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).context("read service json")?)?;
        let g = |k: &str| svc.get(k).and_then(serde_json::Value::as_f64);
        let ga = |k: &str| {
            svc.get(k)
                .and_then(|v| v.as_array())
                .and_then(|a| Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)))
        };
        md.push_str("## Latency under replayed load\n\n");
        md.push_str(&format!(
            "Workload: {}. Coordinated-omission-correct open-loop replay.\n\n",
            svc.get("workload")
                .and_then(|v| v.as_str())
                .unwrap_or("(service)")
        ));
        md.push_str(
            "| percentile | baseline | candidate | speedup | 95% CI |\n|---|---|---|---|---|\n",
        );
        if let (Some(bp50), Some(cp50), Some(s50), Some((l50, h50))) = (
            g("baseline_p50_ms_median"),
            g("candidate_p50_ms_median"),
            g("p50_speedup_median"),
            ga("p50_speedup_ci"),
        ) {
            md.push_str(&format!(
                "| p50 | {:.3} ms | {:.3} ms | {:.4} | [{:.4}, {:.4}] |\n",
                bp50, cp50, s50, l50, h50
            ));
        }
        if let (Some(bp99), Some(cp99), Some(s99), Some((l99, h99))) = (
            g("baseline_p99_ms_median"),
            g("candidate_p99_ms_median"),
            g("p99_speedup_median"),
            ga("p99_speedup_ci"),
        ) {
            md.push_str(&format!(
                "| p99 | {:.3} ms | {:.3} ms | {:.4} | [{:.4}, {:.4}] |\n",
                bp99, cp99, s99, l99, h99
            ));
        }
        md.push('\n');
    }

    // Methodology ships INSIDE the report (SPEC §9) — the differentiation.
    md.push_str(
        "## Methodology (ships with the number)\n\n\
         - **Interleaved A/B**, baseline rebuilt from a pristine checkout, never the agent's\n\
           workspace. Candidate and baseline alternate to control thermal/background drift.\n\
         - **Bootstrap 95% CI** of the ratio-of-medians; an effect is accepted only if the CI\n\
           lower bound clears the threshold in `config/accept.toml` (2%).\n\
         - **Equivalence gates** passed before any number was trusted: upstream test suite,\n\
           byte-identical golden replay, differential fuzzing, and ASan/UBSan sanitizers.\n\
         - **Calibrated hardware**: the workload passed A/A (false-positive <5%) and injected-\n\
           regression (≥95% detection) self-tests, recorded in `results/calibration/`.\n\
         - **Coordinated-omission correct** latency (service mode): each request's latency is\n\
           measured from its intended send time, so queueing is counted, not hidden.\n\n\
         ## Caveats\n\n\
         Every figure is specific to the workload and hardware named above. Throughput→cores\n\
         assumes the fleet is CPU-bound on this path and scales linearly; latency figures are\n\
         for the stated arrival rate. The saving to commit to is the **CI lower bound**, not\n\
         the median.\n",
    );

    match &cli.out {
        Some(p) => {
            std::fs::write(p, &md)?;
            eprintln!("wrote {}", p.display());
        }
        None => print!("{md}"),
    }
    Ok(())
}
