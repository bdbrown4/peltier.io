//! `just explain <run-id>` — advisory post-verdict diagnosis (SPEC §3.7).
//! Reads ONE ledger row and prints why the attempt won or lost. Never on the
//! accept path: the verdict this reads was final before this binary ran.

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use explain::{explain, Facts};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "explain",
    about = "Advisory diagnosis of a ledger row (SPEC §3.7) — derived solely from the machine record"
)]
struct Cli {
    /// Ledger run_id to explain.
    #[arg(long)]
    run_id: String,
    #[arg(long, default_value = "results/ledger.sqlite")]
    db: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ledger = ledger::Ledger::open(&cli.db)
        .with_context(|| format!("opening ledger {}", cli.db.display()))?;
    let row = ledger
        .attempt_row(&cli.run_id)?
        .ok_or_else(|| anyhow!("no ledger row with run_id '{}'", cli.run_id))?;
    let facts = Facts::from_row(&row)
        .ok_or_else(|| anyhow!("row '{}' is missing required fields", cli.run_id))?;
    print!("{}", explain(&facts));
    Ok(())
}
