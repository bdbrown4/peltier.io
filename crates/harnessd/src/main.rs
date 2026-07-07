//! harnessd — the only door between the agent and the trust layer
//! (SPEC §3.5, §10). One JSON request per stdin line, one JSON response
//! per stdout line. The agent gets exactly six operations; everything
//! else (shell, trust-layer writes, patches outside the target
//! workspace) simply has no code path here. Phase 1 caveat, recorded:
//! same-uid filesystem read-only enforcement still requires the
//! separate-user/container setup (Phase 1 infra gap).

use anyhow::{anyhow, ensure, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{BufRead, Write};
use std::path::{Component, Path};
use std::process::Command;

fn main() -> Result<()> {
    let root = std::env::current_dir()?.canonicalize()?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let resp = match handle(&root, &line) {
            Ok(v) => json!({"ok": true, "result": v}),
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        };
        writeln!(stdout, "{resp}")?;
        stdout.flush()?;
    }
    Ok(())
}

fn field<'a>(req: &'a Value, key: &str) -> Result<&'a str> {
    req.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing string field: {key}"))
}

/// A target name must be a plain directory name — no separators, no dots.
fn check_target(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "invalid target name"
    );
    Ok(())
}

/// Relative paths only; no parent components, no .git, no absolutes.
fn check_rel_path(p: &str) -> Result<()> {
    let path = Path::new(p);
    ensure!(!path.is_absolute(), "absolute paths forbidden");
    for c in path.components() {
        match c {
            Component::Normal(seg) => ensure!(seg != ".git", ".git access forbidden"),
            Component::CurDir => {}
            _ => anyhow::bail!("path escapes workspace: {p}"),
        }
    }
    Ok(())
}

fn handle(root: &Path, line: &str) -> Result<Value> {
    let req: Value = serde_json::from_str(line)?;
    match field(&req, "op")? {
        "read_profile" => {
            let t = field(&req, "target")?;
            check_target(t)?;
            let text = std::fs::read_to_string(root.join(format!("results/{t}/hotspots.txt")))?;
            Ok(json!({"hotspots_text": text}))
        }
        "read_ledger" => {
            let t = field(&req, "target")?;
            check_target(t)?;
            // Read-only surface over the append-only DB.
            let ledger = ledger::Ledger::open(&root.join("results/ledger.sqlite"))?;
            let classes = ledger.attempted_classes(t)?;
            Ok(json!({"attempted_playbook_classes": classes, "total_attempts": ledger.count()?}))
        }
        "read_playbook" => {
            let class = match req.get("class") {
                Some(Value::Number(n)) => n.as_u64().ok_or_else(|| anyhow!("bad class"))? as u8,
                Some(Value::String(s)) => s.parse()?,
                _ => anyhow::bail!("missing class"),
            };
            ensure!((1..=7).contains(&class), "playbook class must be 1-7");
            let dir = root.join("playbook");
            let entry = std::fs::read_dir(&dir)?
                .filter_map(|e| e.ok())
                .find(|e| e.file_name().to_string_lossy().starts_with(&format!("{class:02}-")))
                .ok_or_else(|| anyhow!("no playbook file for class {class}"))?;
            Ok(json!({"markdown": std::fs::read_to_string(entry.path())?}))
        }
        "read_target_source" => {
            let t = field(&req, "target")?;
            let p = field(&req, "path")?;
            check_target(t)?;
            check_rel_path(p)?;
            let full = root.join(format!("targets/{t}/workspace")).join(p);
            Ok(json!({"content": std::fs::read_to_string(full)?}))
        }
        "propose_patch" => {
            let t = field(&req, "target")?;
            let diff = field(&req, "diff")?;
            let hypothesis = field(&req, "hypothesis")?;
            check_target(t)?;
            // Allowlist: every path named by the diff must be a safe
            // relative path (the git -C below roots them in the target
            // workspace; nothing outside it is reachable).
            for l in diff.lines() {
                if let Some(p) = l.strip_prefix("--- a/").or_else(|| l.strip_prefix("+++ b/")) {
                    check_rel_path(p.trim())?;
                }
                ensure!(!l.starts_with("--- /") && !l.starts_with("+++ /"), "absolute diff paths forbidden");
            }
            let ws = root.join(format!("targets/{t}/workspace"));
            let mut check = Command::new("git")
                .args(["-C", ws.to_str().unwrap(), "apply", "--check", "-"])
                .stdin(std::process::Stdio::piped())
                .spawn()?;
            check.stdin.take().unwrap().write_all(diff.as_bytes())?;
            ensure!(check.wait()?.success(), "diff does not apply cleanly");
            let mut apply = Command::new("git")
                .args(["-C", ws.to_str().unwrap(), "apply", "-"])
                .stdin(std::process::Stdio::piped())
                .spawn()?;
            apply.stdin.take().unwrap().write_all(diff.as_bytes())?;
            ensure!(apply.wait()?.success(), "git apply failed");
            let patch_id = format!("{:x}", Sha256::digest(diff.as_bytes()))[..12].to_string();
            let pending = root.join("results/pending");
            std::fs::create_dir_all(&pending)?;
            std::fs::write(
                pending.join(format!("{patch_id}.json")),
                serde_json::to_string_pretty(&json!({"target": t, "hypothesis": hypothesis, "diff": diff}))?,
            )?;
            Ok(json!({"patch_id": patch_id}))
        }
        "run_verdict" => {
            let patch_id = field(&req, "patch_id")?;
            ensure!(patch_id.chars().all(|c| c.is_ascii_hexdigit()) && patch_id.len() == 12, "bad patch id");
            let run_id = field(&req, "run_id")?;
            let class = field(&req, "playbook_class")?;
            let hotspot = field(&req, "hotspot")?;
            let pending: Value = serde_json::from_str(&std::fs::read_to_string(
                root.join(format!("results/pending/{patch_id}.json")),
            )?)?;
            let t = pending["target"].as_str().unwrap();
            let spec = diff_test::target::TargetSpec::load(root, t)?;
            // Build the candidate from the patched workspace into an
            // isolated dir, then hand off to the verdict pipeline.
            let cand_dir = root.join(format!("targets/{t}/candidate-{patch_id}"));
            let build = Command::new("sh")
                .arg("-c").arg(&spec.build.baseline)
                .current_dir(root)
                .env("CARGO_TARGET_DIR", &cand_dir)
                .status()?;
            ensure!(build.success(), "candidate build failed");
            let bin = Path::new(&spec.build.binary).file_name().unwrap().to_str().unwrap();
            let diff_path = root.join(format!("results/pending/{patch_id}.json.diff"));
            std::fs::write(&diff_path, pending["diff"].as_str().unwrap())?;
            let out = Command::new("cargo")
                .args([
                    "run", "-q", "-p", "verdict", "--", t,
                    "--rebuild-baseline",
                    "--candidate-bin", &format!("targets/{t}/candidate-{patch_id}/release/{bin}"),
                    "--run-id", run_id,
                    "--playbook-class", class,
                    "--hypothesis", pending["hypothesis"].as_str().unwrap(),
                    "--hotspot", hotspot,
                    "--patch-file", diff_path.to_str().unwrap(),
                ])
                .current_dir(root)
                .output()?;
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            Ok(json!({"exit_ok": out.status.success(), "report": stdout,
                       "stderr_tail": String::from_utf8_lossy(&out.stderr).chars().rev().take(500).collect::<String>().chars().rev().collect::<String>()}))
        }
        other => Err(anyhow!("unknown op: {other} (six ops only, SPEC §3.5)")),
    }
}
