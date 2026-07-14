//! The Unix daemon proper: stdio/socket serving, op dispatch, and the
//! detached verdict pipeline (setsid + Unix sockets keep this POSIX-only;
//! path vetting lives in the portable `allowlist` module).

use crate::allowlist;
use anyhow::{anyhow, ensure, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{BufRead, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

/// Serializes mutating ops (git apply, pending writes) across socket
/// connections; reads don't need it but the cost is nil at one agent.
static HANDLE_LOCK: Mutex<()> = Mutex::new(());

pub fn run() -> Result<()> {
    let root = std::env::current_dir()?.canonicalize()?;
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--socket") {
        let path = args
            .get(i + 1)
            .ok_or_else(|| anyhow!("--socket requires a path"))?;
        return serve_socket(&root, Path::new(path));
    }
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let resp = respond(&root, &line);
        writeln!(stdout, "{resp}")?;
        stdout.flush()?;
    }
    Ok(())
}

/// SPEC §10 socket mode: harnessd runs as the trusted uid and listens on a
/// Unix socket; the agent process runs as an unprivileged user whose only
/// write path into the repo is this daemon. Socket permissions (owner/group/
/// mode) are set by the supervisor script that starts us.
fn serve_socket(root: &Path, sock: &Path) -> Result<()> {
    if let Some(dir) = sock.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let _ = std::fs::remove_file(sock);
    let listener = UnixListener::bind(sock)?;
    eprintln!("harnessd listening on {}", sock.display());
    for conn in listener.incoming() {
        let stream = match conn {
            Ok(s) => s,
            Err(e) => {
                eprintln!("accept error: {e}");
                continue;
            }
        };
        let root = root.to_path_buf();
        std::thread::spawn(move || {
            let reader = match stream.try_clone() {
                Ok(s) => std::io::BufReader::new(s),
                Err(_) => return,
            };
            let mut writer = stream;
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                let resp = respond(&root, &line);
                if writeln!(writer, "{resp}").is_err() {
                    break;
                }
            }
        });
    }
    Ok(())
}

fn respond(root: &Path, line: &str) -> Value {
    // read_verdict long-polls the ledger (read-only) and must not hold
    // the lock that serializes mutating ops while it sleeps.
    let is_read_verdict = serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|v| {
            v.get("op")
                .and_then(Value::as_str)
                .map(|o| o == "read_verdict")
        })
        .unwrap_or(false);
    let _guard = if is_read_verdict {
        None
    } else {
        Some(HANDLE_LOCK.lock().unwrap_or_else(|p| p.into_inner()))
    };
    match handle(root, line) {
        Ok(v) => json!({"ok": true, "result": v}),
        Err(e) => json!({"ok": false, "error": e.to_string()}),
    }
}

/// Single-quote a string for safe embedding in a generated shell script.
fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// The exact probe `scripts/no-net.sh` runs before it decides whether to
/// isolate: can this host open an unprivileged network namespace? Run it here
/// too — same uid, same environment, same host as the wrapper we are about to
/// spawn — so the isolation note we record describes the run that actually
/// happened instead of the one we hoped for.
fn netns_available() -> bool {
    Command::new("unshare")
        .args(["--net", "--map-current-user", "true"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn field<'a>(req: &'a Value, key: &str) -> Result<&'a str> {
    req.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing string field: {key}"))
}

/// A target name must be a plain directory name — no separators, no dots.
fn check_target(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "invalid target name"
    );
    Ok(())
}

const RUNNING_LOCK: &str = "results/pending/RUNNING.lock";

/// The detached verdict pipeline benches the workspace tree it was launched
/// against; HANDLE_LOCK cannot cover it, so a lock file does. Refuse any op
/// that would swap that tree while a pipeline is in flight.
fn ensure_no_running_pipeline(root: &Path) -> Result<()> {
    let lock = root.join(RUNNING_LOCK);
    if lock.exists() {
        let rid = std::fs::read_to_string(&lock).unwrap_or_default();
        let rid = rid.trim();
        let rid = if rid.is_empty() { "unknown" } else { rid };
        anyhow::bail!(
            "verdict pipeline {rid} in flight ({RUNNING_LOCK} present — wait for \
             read_verdict to resolve it, or remove the lock if the pipeline is dead)"
        );
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
            let history = ledger.attempt_history(t)?;
            Ok(json!({
                "attempted_playbook_classes": classes,
                "attempts": history,
                "total_attempts": ledger.count()?,
                "note": "a class may be re-entered with a materially NEW hypothesis; \
                         never duplicate a (hotspot, class, hypothesis) that has a verdict",
            }))
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
                .find(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with(&format!("{class:02}-"))
                })
                .ok_or_else(|| anyhow!("no playbook file for class {class}"))?;
            Ok(json!({"markdown": std::fs::read_to_string(entry.path())?}))
        }
        "read_target_source" => {
            let t = field(&req, "target")?;
            let p = field(&req, "path")?;
            check_target(t)?;
            allowlist::check_rel_path(p)?;
            // Serve the PRISTINE (HEAD) content, not the working tree:
            // propose_patch applies each proposal against a reset
            // workspace, so reads must show the same base the diff needs
            // — otherwise reads after a proposal show the patched tree
            // and the next diff's context lines drift (phase2-comrak-005
            // burned turns rediscovering this).
            let ws = root.join(format!("targets/{t}/workspace"));
            let shown = Command::new("git")
                .args(["-C", ws.to_str().unwrap(), "show", &format!("HEAD:{p}")])
                .output()?;
            ensure!(
                shown.status.success(),
                "cannot read {p} at HEAD: {}",
                String::from_utf8_lossy(&shown.stderr).trim()
            );
            let text = String::from_utf8_lossy(&shown.stdout).into_owned();
            // Optional line window so large source files (100KB+) can be read in
            // chunks instead of overflowing one response — otherwise the agent
            // is tempted to reach for a shell it must not have.
            let lines: Vec<&str> = text.lines().collect();
            let total = lines.len();
            let offset = req.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
            let limit = req
                .get("limit")
                .and_then(Value::as_u64)
                .map(|v| v as usize)
                .unwrap_or(usize::MAX);
            let end = offset.saturating_add(limit).min(total);
            let start = offset.min(total);
            let slice = lines[start..end]
                .iter()
                .enumerate()
                .map(|(i, l)| format!("{}\t{}", start + i + 1, l))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(json!({
                "content": slice,
                "total_lines": total,
                "returned_lines": [start + 1, end],
                "truncated": end < total,
            }))
        }
        "propose_patch" => {
            let t = field(&req, "target")?;
            let diff = field(&req, "diff")?;
            let hypothesis = field(&req, "hypothesis")?;
            check_target(t)?;
            ensure_no_running_pipeline(root)?;
            // git rejects a diff without a final newline with an opaque
            // "corrupt patch" — say it plainly (phase2-comrak-004 burned
            // several turns rediscovering this).
            ensure!(
                diff.ends_with('\n'),
                "diff must end with a trailing newline (git would report 'corrupt patch' at EOF)"
            );
            // Allowlist: every path named by any diff header must be a safe
            // relative path (the git -C below roots them in the target
            // workspace; nothing outside it is reachable).
            allowlist::check_diff_paths(diff)?;
            let ws = root.join(format!("targets/{t}/workspace"));
            // Every proposal stands alone: reset the workspace to HEAD
            // first so probe/iteration patches cannot accumulate into a
            // tested tree the ledger's patch field doesn't describe
            // (phase2-comrak-004 audit finding).
            ensure!(
                Command::new("git")
                    .args(["-C", ws.to_str().unwrap(), "checkout", "--", "."])
                    .status()?
                    .success(),
                "workspace reset failed"
            );
            ensure!(
                Command::new("git")
                    .args(["-C", ws.to_str().unwrap(), "clean", "-fdq"])
                    .status()?
                    .success(),
                "workspace clean failed"
            );
            let run_git_apply = |check: bool| -> Result<std::process::Output> {
                let mut args = vec!["-C", ws.to_str().unwrap(), "apply"];
                if check {
                    args.push("--check");
                }
                args.push("-");
                let mut child = Command::new("git")
                    .args(&args)
                    .stdin(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()?;
                child.stdin.take().unwrap().write_all(diff.as_bytes())?;
                Ok(child.wait_with_output()?)
            };
            let checked = run_git_apply(true)?;
            ensure!(
                checked.status.success(),
                "diff does not apply cleanly against the pristine workspace \
                 (proposals are standalone; prior proposals were reset): {}",
                String::from_utf8_lossy(&checked.stderr).trim()
            );
            let applied = run_git_apply(false)?;
            ensure!(
                applied.status.success(),
                "git apply failed: {}",
                String::from_utf8_lossy(&applied.stderr).trim()
            );
            let patch_id = format!("{:x}", Sha256::digest(diff.as_bytes()))[..12].to_string();
            let pending = root.join("results/pending");
            std::fs::create_dir_all(&pending)?;
            std::fs::write(
                pending.join(format!("{patch_id}.json")),
                serde_json::to_string_pretty(
                    &json!({"target": t, "hypothesis": hypothesis, "diff": diff}),
                )?,
            )?;
            Ok(json!({"patch_id": patch_id}))
        }
        "run_verdict" => {
            let patch_id = field(&req, "patch_id")?;
            ensure!(
                patch_id.chars().all(|c| c.is_ascii_hexdigit()) && patch_id.len() == 12,
                "bad patch id"
            );
            let run_id = field(&req, "run_id")?;
            ensure!(
                run_id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "bad run id"
            );
            let class = field(&req, "playbook_class")?;
            let hotspot = field(&req, "hotspot")?;
            ensure_no_running_pipeline(root)?;
            let pending: Value = serde_json::from_str(&std::fs::read_to_string(
                root.join(format!("results/pending/{patch_id}.json")),
            )?)?;
            let t = pending["target"].as_str().unwrap();
            let diff = pending["diff"].as_str().unwrap();
            // Defense in depth: the pending file sat on disk since
            // propose_patch vetted it — vet again before applying.
            allowlist::check_diff_paths(diff)?;
            let spec = diff_test::target::TargetSpec::load(root, t)?;
            let diff_path = root.join(format!("results/pending/{patch_id}.json.diff"));
            std::fs::write(&diff_path, diff)?;
            // Re-establish the exact tree the pipeline must test: proposals
            // after this one may have reset/repatched the workspace.
            let ws = root.join(format!("targets/{t}/workspace"));
            let git_ws = |args: &[&str]| -> Result<()> {
                let out = Command::new("git").arg("-C").arg(&ws).args(args).output()?;
                ensure!(
                    out.status.success(),
                    "git {} failed in target workspace: {}",
                    args.join(" "),
                    String::from_utf8_lossy(&out.stderr).trim()
                );
                Ok(())
            };
            git_ws(&["checkout", "--", "."])?;
            git_ws(&["clean", "-fdq"])?;
            let dp = diff_path.to_str().unwrap();
            git_ws(&["apply", "--check", dp]).map_err(|e| {
                anyhow!(
                    "pending diff {patch_id} no longer applies to the pristine \
                     workspace — re-propose it: {e}"
                )
            })?;
            git_ws(&["apply", dp])?;
            // The build + verdict pipeline runs for minutes — far past the MCP
            // transport's per-call cap. Launch it detached, writing progress to
            // a log; the agent observes the result via the read_verdict op,
            // which reads the append-only ledger row once written.
            // {out} isolates the candidate build (cargo: CARGO_TARGET_DIR;
            // make: build+copy into it) — language-agnostic, mirrors the
            // baseline rebuild in verdict.
            let cand_dir = root
                .join(format!("targets/{t}/candidate-{patch_id}"))
                .to_string_lossy()
                .into_owned();
            let cand_build = diff_test::target::subst_out(&spec.build.baseline, &cand_dir);
            let cand_bin = diff_test::target::subst_out(&spec.build.binary, &cand_dir);
            let log = format!("results/pending/{run_id}.log");
            // Target code must not reach the network: the pipeline launches
            // through a wrapper (default scripts/no-net.sh, a network-namespace
            // exec); HOTPATH_VERDICT_WRAPPER overrides with a single executable
            // path for hosts where unshare is unavailable.
            let (wrapper, isolation_note) = match std::env::var("HOTPATH_VERDICT_WRAPPER") {
                Ok(w) => {
                    let note = w.clone();
                    (w, note)
                }
                Err(_) => {
                    let w = root
                        .join("scripts/no-net.sh")
                        .to_string_lossy()
                        .into_owned();
                    // Honesty: this note is echoed to the agent AND stamped into
                    // the ledger's env fingerprint. With HOTPATH_ALLOW_UNISOLATED=1
                    // set, no-net.sh execs the pipeline with FULL network access on
                    // any host where it cannot open a netns — recording a bare
                    // "no-net.sh" there would claim an isolation the run never had.
                    // Probe the same condition the wrapper will test, and say which
                    // of the two actually applies.
                    let allow_unisolated = std::env::var("HOTPATH_ALLOW_UNISOLATED")
                        .map(|v| v == "1")
                        .unwrap_or(false);
                    let note = if allow_unisolated && !netns_available() {
                        "no-net.sh (HOTPATH_ALLOW_UNISOLATED=1 — network NOT isolated)"
                    } else {
                        "no-net.sh"
                    };
                    (w, note.to_string())
                }
            };
            // Two scripts, because failure accounting has to survive a wrapper
            // that never execs its argv.
            //
            // INNER — runs inside the isolation wrapper. The guarded section is
            // a SUBSHELL, not the left operand of an `&&`: POSIX sh ignores
            // `set -e` inside any compound command that is an AND-OR operand
            // other than the last, so the previous
            //     { set -e; <build>; verdict; } && rm -f lock || { ...; }
            // shape left errexit inert. A failing candidate build fell straight
            // through to `cargo run -p verdict`, which then benched — and wrote
            // a ledger row for — a STALE or missing candidate binary. That is
            // the exact class of false evidence this project exists to prevent.
            // A standalone subshell scopes errexit and makes it live again, so a
            // failed build aborts before verdict can measure anything. The
            // subshell is the script's last command, so its status is the
            // script's status.
            let inner = format!(
                "printf %s {rid} > {lock}\n\
                 ( set -e\n{build}\ncargo run -q -p verdict -- {tgt} \
                 --rebuild-baseline --candidate-bin {cbin} --run-id {rid} --playbook-class {cls} \
                 --hypothesis {hyp} --hotspot {hs} --patch-file {pf} --isolation-note {iso}\n\
                 )\n",
                rid = shq(run_id),
                lock = shq(RUNNING_LOCK),
                build = cand_build,
                tgt = shq(t),
                cbin = shq(&cand_bin),
                cls = shq(class),
                hyp = shq(pending["hypothesis"].as_str().unwrap()),
                hs = shq(hotspot),
                pf = shq(diff_path.to_str().unwrap()),
                iso = shq(&isolation_note),
            );
            // OUTER — runs OUTSIDE the wrapper: invoke the wrapper, then account
            // for its exit status no matter what it did. scripts/no-net.sh exits
            // 97 WITHOUT exec'ing its argv when unprivileged netns are
            // unavailable, and a missing or non-executable HOTPATH_VERDICT_WRAPPER
            // exits 127/126 — in all three cases the inner script never runs at
            // all. While this accounting lived inside the wrapper, those cases
            // removed no lock and wrote no marker: read_verdict polled "running"
            // forever and every later propose_patch/run_verdict was refused with
            // "verdict pipeline in flight" until a human deleted the lock by hand.
            // Out here it always runs, so the run is reported failed and the
            // daemon stays usable. The wrapper's own stderr (no-net.sh's refusal
            // text) is already captured in the run log.
            let script = format!(
                "{wrapper} sh -c {inner}\n\
                 rc=$?\n\
                 rm -f {lock}\n\
                 [ \"$rc\" -eq 0 ] || echo \"HOTPATH-PIPELINE-FAILED (exit $rc)\"\n",
                wrapper = shq(&wrapper),
                inner = shq(&inner),
                lock = shq(RUNNING_LOCK),
            );
            let logf = std::fs::File::create(root.join(&log))?;
            let errf = logf.try_clone()?;
            // Write the lock before spawning: the detached script's own
            // write lands only after exec latency, and a propose_patch in
            // that window would swap the tree under the pipeline.
            let lock_path = root.join(RUNNING_LOCK);
            std::fs::write(&lock_path, run_id)?;
            // The wrapper is invoked BY the outer script (and is shq'd into it),
            // not by setsid — that is what puts the accounting outside it.
            let spawned = Command::new("setsid")
                .arg("sh")
                .arg("-c")
                .arg(&script)
                .current_dir(root)
                .stdout(std::process::Stdio::from(logf))
                .stderr(std::process::Stdio::from(errf))
                .spawn();
            if let Err(e) = spawned {
                let _ = std::fs::remove_file(&lock_path);
                return Err(anyhow!("failed to launch verdict pipeline: {e}"));
            }
            Ok(json!({"status": "started", "run_id": run_id, "log": log,
                       "isolation": isolation_note,
                       "note": "pipeline runs detached; poll read_verdict with this run_id"}))
        }
        "read_verdict" => {
            let run_id = field(&req, "run_id")?;
            // Long-poll: the pipeline runs for minutes and the MCP transport
            // caps a tool call at 60s. Waiting ~45s server-side per poll cuts
            // the agent's turn burn ~15x versus instant "running" replies.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
            loop {
                let ledger = ledger::Ledger::open(&root.join("results/ledger.sqlite"))?;
                if let Some(v) = ledger.verdict_summary(run_id)? {
                    return Ok(v);
                }
                drop(ledger);
                // A pipeline that died before the verdict binary ran never
                // writes a row; the launcher's failure accounting — which runs
                // outside the isolation wrapper, so it survives a wrapper that
                // refuses to exec — marks the log instead.
                if run_id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    let log = root.join(format!("results/pending/{run_id}.log"));
                    if let Ok(text) = std::fs::read_to_string(&log) {
                        if text.contains("HOTPATH-PIPELINE-FAILED") {
                            let tail: String = text
                                .lines()
                                .rev()
                                .take(12)
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .collect::<Vec<_>>()
                                .join("\n");
                            return Ok(json!({
                                "status": "failed",
                                "note": "the pipeline died before producing a verdict — usually a \
                                         candidate build failure, otherwise a crash or the isolation \
                                         wrapper refusing to run (scripts/no-net.sh exits 97 when it \
                                         cannot open a network namespace). The marker line carries the \
                                         exit status. No ledger row was or will be written for this \
                                         run_id, and the run lock has been released. Log tail follows.",
                                "log_tail": tail,
                            }));
                        }
                    }
                }
                if std::time::Instant::now() >= deadline {
                    return Ok(json!({"status": "running",
                                     "note": "no ledger row yet; the pipeline is still building/benching — poll again"}));
                }
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        }
        other => Err(anyhow!(
            "unknown op: {other} (seven ops, SPEC §3.5 + async read_verdict)"
        )),
    }
}
