//! Core gate sequence, callable from the diff-test CLI and from verdict:
//! corpus pin (refuses on mismatch) → upstream tests → golden replay.

use crate::target::{expected_golden_hash, TargetSpec};
use crate::{pin, GateLayer, GateOutcome};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::{Command, Stdio};

fn sh(root: &Path, cmd: &str) -> anyhow::Result<std::process::Output> {
    Ok(Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?)
}

/// Run the mechanical gates for `target`, testing `binary` (repo-root-
/// relative) in the golden replay. Corpus-pin mismatch is an error, not
/// a gate failure: nothing runs on a tampered corpus.
pub fn run_core_gates(
    root: &Path,
    spec: &TargetSpec,
    binary: &str,
) -> anyhow::Result<Vec<(GateLayer, GateOutcome)>> {
    let verified = pin::verify_manifest(
        &root.join(&spec.corpus.manifest),
        &root.join(&spec.corpus.root),
    )?;
    eprintln!(
        "corpus pin: {verified} files verified against {}",
        spec.corpus.manifest.display()
    );

    let mut results = Vec::new();

    let t = sh(root, &spec.gates.tests)?;
    results.push((
        GateLayer::UpstreamTests,
        if t.status.success() {
            GateOutcome::Passed
        } else {
            GateOutcome::Failed {
                detail: String::from_utf8_lossy(&t.stderr).chars().take(2000).collect(),
            }
        },
    ));

    let g = sh(root, &spec.gates.golden.replace("{binary}", binary))?;
    let expected = expected_golden_hash(&root.join(&spec.corpus.golden_sha256))?;
    let actual = format!("{:x}", Sha256::digest(&g.stdout));
    results.push((
        GateLayer::GoldenReplay,
        if g.status.success() && actual == expected {
            GateOutcome::Passed
        } else {
            GateOutcome::Failed {
                detail: format!("exit={:?} expected={expected} actual={actual}", g.status.code()),
            }
        },
    ));

    results.push((
        GateLayer::DifferentialFuzz,
        GateOutcome::Skipped {
            reason: "per-attempt manual step in Phase 1 (needs old/new pair)".into(),
        },
    ));
    results.push((
        GateLayer::Sanitizers,
        GateOutcome::Skipped {
            reason: "per-attempt manual step in Phase 1 (nightly toolchain)".into(),
        },
    ));
    Ok(results)
}
