//! `policy rank` — the learned class-selection prior (SPEC §13).
//! Reads the append-only ledger and ranks playbook classes by the Wilson
//! lower bound of their observed shippable-win rate, degrading to
//! cheapest-first where evidence is thin. Advisory only: the gates still
//! decide every verdict. This just tells the agent where to look first.

use anyhow::Result;
use clap::Parser;
use policy::{class_stats, rank};
use std::path::PathBuf;

const CLASS_NAME: [&str; 8] = [
    "",
    "build-config",
    "allocator",
    "alloc-churn",
    "data-layout",
    "algorithmic",
    "simd",
    "concurrency",
];

#[derive(Parser)]
#[command(
    name = "policy",
    about = "Learned playbook-class ranking from the ledger (SPEC §13)"
)]
struct Cli {
    /// Restrict the evidence to one target (else pool all targets).
    #[arg(long)]
    target: Option<String>,
    #[arg(long, default_value = "results/ledger.sqlite")]
    db: PathBuf,
    /// z for the Wilson interval (1.96 = 95%).
    #[arg(long, default_value_t = 1.96)]
    z: f64,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ledger = ledger::Ledger::open(&cli.db)?;
    let outcomes = ledger.all_outcomes()?;
    let ranked = rank(class_stats(&outcomes, cli.target.as_deref(), cli.z));

    let scope = cli.target.as_deref().unwrap_or("all targets");
    println!(
        "learned class-selection ranking — evidence: {} ledger rows, scope: {scope}\n\
         (Wilson 95% lower bound of the shippable-win rate; ties break untried-before-failed, then cheapest-first)\n",
        outcomes.len()
    );
    println!(
        "{:>4}  {:<13} {:>8} {:>5} {:>5} {:>9} {:>10}  note",
        "rank", "class", "attempts", "wins", "held", "win-rate", "wilson-lb"
    );
    println!("{}", "-".repeat(78));
    for (i, s) in ranked.iter().enumerate() {
        println!(
            "{:>4}  {:>1} {:<11} {:>8} {:>5} {:>5} {:>8.0}% {:>10.3}  {}",
            i + 1,
            s.class,
            CLASS_NAME[s.class as usize],
            s.attempts,
            s.wins,
            s.held,
            s.win_rate * 100.0,
            s.wilson_lb,
            s.note(),
        );
    }

    // The actionable line: the first class with real supporting evidence,
    // else the cheapest untried class.
    let best_evidenced = ranked.iter().find(|s| s.wilson_lb > 0.0);
    let cheapest_untried = ranked
        .iter()
        .filter(|s| s.attempts == 0)
        .min_by_key(|s| s.class);
    println!();
    match (best_evidenced, cheapest_untried) {
        (Some(e), _) => println!(
            "recommendation: try class {} ({}) first — best evidence (Wilson lb {:.3}); \
             then the next-ranked class whose profile preconditions match.",
            e.class, CLASS_NAME[e.class as usize], e.wilson_lb
        ),
        (None, Some(u)) => println!(
            "recommendation: no class has supporting evidence yet; fall back to the \
             cheapest untried class {} ({}).",
            u.class, CLASS_NAME[u.class as usize]
        ),
        (None, None) => {
            println!("recommendation: no evidence and no untried class; use cheapest-first.")
        }
    }
    println!(
        "\nAdvisory only — the equivalence + significance gates decide every verdict. \
         This ranking sharpens as the ledger grows."
    );
    println!(
        "note: a 'win' counts only machine-sanitizer-verified accepts. Pre-sanitizer-gate \
         accepts and the overturned comrak-010 are conservatively excluded — the policy \
         trusts the ledger's machine record, not the external audit narrative."
    );
    Ok(())
}
