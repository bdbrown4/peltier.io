//! Per-target spec (`targets/<name>/target.toml`). All commands and
//! paths are relative to the repository root; `{binary}` in commands is
//! substituted with the built binary path.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct TargetSpec {
    pub source: Source,
    pub build: Build,
    pub bench: Bench,
    pub gates: Gates,
    pub corpus: Corpus,
}

#[derive(Debug, Deserialize)]
pub struct Bench {
    /// Bench command; `{binary}` substituted per side. Run from repo root.
    pub command: String,
    /// Human-readable workload statement, printed with every number.
    pub workload: String,
}

#[derive(Debug, Deserialize)]
pub struct Source {
    pub repo: String,
    pub commit: String,
    pub license: String,
    #[serde(default)]
    pub submodules: toml::value::Table,
}

#[derive(Debug, Deserialize)]
pub struct Build {
    /// Build command, run from repo root. `{out}` is substituted with an
    /// isolation directory the harness provides (per-baseline, per-
    /// candidate, per-sanitizer) — the command MUST place all artifacts
    /// under it. A cargo target sets `CARGO_TARGET_DIR={out}`; a make
    /// target builds and copies the binary to `{out}`. Language-agnostic:
    /// nothing here assumes cargo.
    pub baseline: String,
    /// Built binary path with `{out}` substituted — where `baseline`
    /// leaves the binary given an isolation dir.
    pub binary: String,
    /// ASan+LSan (or ASan+UBSan for C/C++) build command; `{out}`
    /// substituted. Run on every would-be accept (SPEC §8). If absent,
    /// an accept cannot be sanitizer-verified and is capped at
    /// needs-human-review.
    #[serde(default)]
    pub sanitizer: Option<String>,
    /// Sanitizer binary path with `{out}` substituted.
    #[serde(default)]
    pub sanitizer_binary: Option<String>,
}

/// Substitute the `{out}` isolation-dir placeholder in a build command or
/// binary-path template.
pub fn subst_out(template: &str, out_dir: &str) -> String {
    template.replace("{out}", out_dir)
}

#[derive(Debug, Deserialize)]
pub struct Gates {
    /// Upstream test suite command, run from repo root.
    pub tests: String,
    /// Golden replay command; stdout is hashed. `{binary}` substituted.
    pub golden: String,
}

#[derive(Debug, Deserialize)]
pub struct Corpus {
    /// sha256sum-format manifest of the corpus inputs.
    pub manifest: PathBuf,
    /// Directory the manifest's relative paths resolve against.
    pub root: PathBuf,
    /// File whose last whitespace-delimited-first-field line is the
    /// expected sha256 of the golden command's stdout.
    pub golden_sha256: PathBuf,
}

impl TargetSpec {
    pub fn load(repo_root: &Path, name: &str) -> anyhow::Result<Self> {
        let path = repo_root.join("targets").join(name).join("target.toml");
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
        Ok(toml::from_str(&raw)?)
    }
}

/// Parse the expected stdout hash out of a GOLDEN.sha256 file
/// (comment lines starting with '#' ignored).
pub fn expected_golden_hash(path: &Path) -> anyhow::Result<String> {
    let raw = std::fs::read_to_string(path)?;
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_whitespace().next())
        .next_back()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("no hash entry in {}", path.display()))
}
