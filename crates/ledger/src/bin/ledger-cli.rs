//! Manual ledger entry for Phase 0 dry runs: reads one JSON `Attempt`
//! from stdin, appends it, prints the running count. The DB is
//! append-only (triggers); there is deliberately no edit or delete
//! subcommand to grow here.

use ledger::{Attempt, Ledger};
use std::io::Read;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let db = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("results/ledger.sqlite"));
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw)?;
    let attempt: Attempt = serde_json::from_str(&raw)?;
    let ledger = Ledger::open(&db)?;
    ledger.record(&attempt)?;
    println!(
        "recorded {} verdict={} ({} attempts total)",
        attempt.run_id,
        attempt.verdict.as_str(),
        ledger.count()?
    );
    Ok(())
}
