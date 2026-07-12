//! Append-only attempt ledger (SPEC §3.3).
//!
//! One row per optimization attempt. Nothing is ever deleted or updated —
//! enforced with SQLite triggers, not convention. The ledger is the audit
//! trail, the anti-double-attempt memory, and the future training set.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Accepted,
    RejectedGate,
    RejectedBench,
    NeedsHumanReview,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Accepted => "accepted",
            Verdict::RejectedGate => "rejected-gate",
            Verdict::RejectedBench => "rejected-bench",
            Verdict::NeedsHumanReview => "needs-human-review",
        }
    }
}

/// Gate outcomes for one attempt (SPEC §3.2).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateResults {
    pub upstream_tests: bool,
    pub golden_replay: bool,
    pub fuzz_iters: u64,
    pub fuzz_divergence: bool,
    pub sanitizers_clean: bool,
}

/// Benchmark evidence for one attempt. All times in seconds; CIs are
/// bootstrap 95% intervals over the ratio baseline/candidate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BenchEvidence {
    pub baseline_median: f64,
    pub baseline_ci: (f64, f64),
    pub candidate_median: f64,
    pub candidate_ci: (f64, f64),
    pub speedup_median: f64,
    pub speedup_ci: (f64, f64),
    pub env_fingerprint: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    pub run_id: String,
    /// RFC 3339, supplied by the harness (the ledger records, it does not clock).
    pub timestamp: String,
    pub target: String,
    pub target_commit: String,
    pub phase: u8,
    /// Symbol plus its share of the profile, e.g. "png_read_filter_row (31.4%)".
    pub hotspot: String,
    /// Playbook class 1–7 (SPEC §4).
    pub playbook_class: u8,
    pub hypothesis: String,
    /// Unified diff as proposed by the agent, pre-application.
    pub patch: String,
    pub gates: GateResults,
    pub bench: Option<BenchEvidence>,
    pub verdict: Verdict,
    pub tokens_spent: u64,
    pub wall_time_s: f64,
}

pub struct Ledger {
    conn: Connection,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS attempts (
    run_id         TEXT PRIMARY KEY,
    timestamp      TEXT NOT NULL,
    target         TEXT NOT NULL,
    target_commit  TEXT NOT NULL,
    phase          INTEGER NOT NULL,
    hotspot        TEXT NOT NULL,
    playbook_class INTEGER NOT NULL,
    hypothesis     TEXT NOT NULL,
    patch          TEXT NOT NULL,
    gates          TEXT NOT NULL,  -- JSON GateResults
    bench          TEXT,           -- JSON BenchEvidence, NULL if gates failed first
    verdict        TEXT NOT NULL CHECK (verdict IN
                     ('accepted','rejected-gate','rejected-bench','needs-human-review')),
    tokens_spent   INTEGER NOT NULL,
    wall_time_s    REAL NOT NULL
);

-- Append-only, structurally: mutation raises instead of succeeding.
CREATE TRIGGER IF NOT EXISTS attempts_no_update
BEFORE UPDATE ON attempts
BEGIN SELECT RAISE(ABORT, 'ledger is append-only'); END;

CREATE TRIGGER IF NOT EXISTS attempts_no_delete
BEFORE DELETE ON attempts
BEGIN SELECT RAISE(ABORT, 'ledger is append-only'); END;

CREATE INDEX IF NOT EXISTS attempts_by_target ON attempts (target, playbook_class);
"#;

impl Ledger {
    pub fn open(path: &Path) -> Result<Self, LedgerError> {
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self, LedgerError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub fn record(&self, a: &Attempt) -> Result<(), LedgerError> {
        let bench_json = a.bench.as_ref().map(serde_json::to_string).transpose()?;
        self.conn.execute(
            "INSERT INTO attempts (run_id, timestamp, target, target_commit, phase, hotspot,
                 playbook_class, hypothesis, patch, gates, bench, verdict, tokens_spent, wall_time_s)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            rusqlite::params![
                a.run_id,
                a.timestamp,
                a.target,
                a.target_commit,
                a.phase,
                a.hotspot,
                a.playbook_class,
                a.hypothesis,
                a.patch,
                serde_json::to_string(&a.gates)?,
                bench_json,
                a.verdict.as_str(),
                a.tokens_spent,
                a.wall_time_s,
            ],
        )?;
        Ok(())
    }

    /// Playbook classes already attempted against a target — the agent's
    /// anti-double-attempt memory (SPEC §3.5 prompting spine, rule 2).
    pub fn attempted_classes(&self, target: &str) -> Result<Vec<u8>, LedgerError> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT playbook_class FROM attempts WHERE target = ?1 ORDER BY playbook_class",
        )?;
        let rows = stmt.query_map([target], |r| r.get::<_, u8>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Per-attempt history for a target: class, hotspot, hypothesis,
    /// verdict. The agent's anti-duplicate memory at hypothesis
    /// granularity — a class may be re-entered with a materially new
    /// hypothesis, but never the same (hotspot, class, hypothesis).
    pub fn attempt_history(&self, target: &str) -> Result<Vec<serde_json::Value>, LedgerError> {
        let mut stmt = self.conn.prepare(
            "SELECT run_id, playbook_class, hotspot, hypothesis, verdict \
             FROM attempts WHERE target = ?1 ORDER BY rowid",
        )?;
        let rows = stmt.query_map([target], |r| {
            Ok(serde_json::json!({
                "run_id": r.get::<_, String>(0)?,
                "playbook_class": r.get::<_, u8>(1)?,
                "hotspot": r.get::<_, String>(2)?,
                "hypothesis": r.get::<_, String>(3)?,
                "verdict": r.get::<_, String>(4)?,
            }))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Read-only verdict summary for one run — the agent's only window
    /// onto a completed attempt (harnessd `read_verdict`). Returns the
    /// verdict plus the bench CIs; never enough to game, since the row
    /// is already written and immutable.
    /// Full row data the ROI report generator needs (SPEC §9). Includes
    /// the workload string pulled out of the bench evidence.
    pub fn report_row(&self, run_id: &str) -> Result<Option<serde_json::Value>, LedgerError> {
        let row = self.conn.query_row(
            "SELECT target, playbook_class, hypothesis, verdict, bench, timestamp \
             FROM attempts WHERE run_id = ?1",
            [run_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, u8>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        );
        match row {
            Ok((target, class, hypothesis, verdict, bench, ts)) => {
                let bench: Option<serde_json::Value> =
                    bench.map(|b| serde_json::from_str(&b)).transpose()?;
                let workload = bench
                    .as_ref()
                    .and_then(|b| b.get("env_fingerprint"))
                    .and_then(|f| f.get("workload"))
                    .cloned();
                // Pull the sanitizer gate out so the report can refuse to
                // mint a clean ROI for an `accepted` row that predates the
                // machine-enforced sanitizer gate and was overturned by
                // audit (phase2-comrak-010). The gates JSON is stored
                // separately; read it here.
                let sanitizers_clean = self
                    .conn
                    .query_row(
                        "SELECT gates FROM attempts WHERE run_id = ?1",
                        [run_id],
                        |r| r.get::<_, Option<String>>(0),
                    )
                    .ok()
                    .flatten()
                    .and_then(|g| serde_json::from_str::<serde_json::Value>(&g).ok())
                    .and_then(|g| g.get("sanitizers_clean").and_then(|v| v.as_bool()))
                    .unwrap_or(false);
                Ok(Some(serde_json::json!({
                    "run_id": run_id,
                    "target": target,
                    "playbook_class": class,
                    "hypothesis": hypothesis,
                    "verdict": verdict,
                    "sanitizers_clean": sanitizers_clean,
                    "timestamp": ts,
                    "speedup_median": bench.as_ref().and_then(|b| b.get("speedup_median").cloned()),
                    "speedup_ci": bench.as_ref().and_then(|b| b.get("speedup_ci").cloned()),
                    "workload": workload,
                })))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn verdict_summary(&self, run_id: &str) -> Result<Option<serde_json::Value>, LedgerError> {
        let row = self.conn.query_row(
            "SELECT verdict, bench FROM attempts WHERE run_id = ?1",
            [run_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
        );
        match row {
            Ok((verdict, bench)) => {
                let bench: Option<serde_json::Value> =
                    bench.map(|b| serde_json::from_str(&b)).transpose()?;
                Ok(Some(serde_json::json!({
                    "run_id": run_id,
                    "verdict": verdict,
                    "speedup_median": bench.as_ref().and_then(|b| b.get("speedup_median").cloned()),
                    "speedup_ci": bench.as_ref().and_then(|b| b.get("speedup_ci").cloned()),
                })))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn count(&self) -> Result<u64, LedgerError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM attempts", [], |r| r.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_attempt(run_id: &str) -> Attempt {
        Attempt {
            run_id: run_id.into(),
            timestamp: "2026-07-06T00:00:00Z".into(),
            target: "example".into(),
            target_commit: "deadbeef".into(),
            phase: 1,
            hotspot: "hot_fn (42.0%)".into(),
            playbook_class: 2,
            hypothesis: "mimalloc reduces allocator contention".into(),
            patch: "--- a/Cargo.toml\n+++ b/Cargo.toml\n".into(),
            gates: GateResults::default(),
            bench: None,
            verdict: Verdict::RejectedGate,
            tokens_spent: 1000,
            wall_time_s: 12.5,
        }
    }

    #[test]
    fn record_and_query() {
        let ledger = Ledger::open_in_memory().unwrap();
        ledger.record(&sample_attempt("r1")).unwrap();
        assert_eq!(ledger.count().unwrap(), 1);
        assert_eq!(ledger.attempted_classes("example").unwrap(), vec![2]);
    }

    #[test]
    fn append_only_is_enforced() {
        let ledger = Ledger::open_in_memory().unwrap();
        ledger.record(&sample_attempt("r1")).unwrap();
        let upd = ledger
            .conn
            .execute("UPDATE attempts SET verdict = 'accepted'", []);
        assert!(upd.is_err(), "UPDATE must be rejected by trigger");
        let del = ledger.conn.execute("DELETE FROM attempts", []);
        assert!(del.is_err(), "DELETE must be rejected by trigger");
        assert_eq!(ledger.count().unwrap(), 1);
    }
}
