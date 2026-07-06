//! Corpus hash-pinning (SPEC §3.2, §10): corpora live outside
//! agent-writable paths and are pinned by a SHA-256 manifest. diff-test
//! refuses to run if any hash mismatches — a tampered corpus is a
//! stop-the-line event, not a warning.

use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum PinError {
    #[error("io error reading {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("malformed manifest line {line}: {content:?}")]
    MalformedManifest { line: usize, content: String },
    #[error("HASH MISMATCH for {path}: manifest {expected}, actual {actual} — refusing to run")]
    Mismatch {
        path: String,
        expected: String,
        actual: String,
    },
}

pub fn sha256_hex(path: &Path) -> Result<String, PinError> {
    let bytes = std::fs::read(path).map_err(|source| PinError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

/// Verify every entry of a `sha256sum`-format manifest
/// (`<hex-digest>  <relative-path>` per line, paths relative to `root`).
/// Returns the number of files verified; errors on the first mismatch.
pub fn verify_manifest(manifest: &Path, root: &Path) -> Result<usize, PinError> {
    let raw = std::fs::read_to_string(manifest).map_err(|source| PinError::Io {
        path: manifest.display().to_string(),
        source,
    })?;
    let mut verified = 0;
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (expected, rel) = line
            .split_once("  ")
            .ok_or_else(|| PinError::MalformedManifest {
                line: i + 1,
                content: line.to_string(),
            })?;
        let path = root.join(rel);
        let actual = sha256_hex(&path)?;
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(PinError::Mismatch {
                path: path.display().to_string(),
                expected: expected.to_string(),
                actual,
            });
        }
        verified += 1;
    }
    Ok(verified)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn verifies_and_refuses() {
        let dir = std::env::temp_dir().join("hotpath-pin-test");
        fs::create_dir_all(&dir).unwrap();
        let corpus = dir.join("input.bin");
        fs::write(&corpus, b"golden input").unwrap();
        let digest = sha256_hex(&corpus).unwrap();

        let manifest = dir.join("MANIFEST.sha256");
        fs::write(&manifest, format!("{digest}  input.bin\n")).unwrap();
        assert_eq!(verify_manifest(&manifest, &dir).unwrap(), 1);

        // Tamper: verification must refuse.
        fs::write(&corpus, b"tampered input").unwrap();
        let err = verify_manifest(&manifest, &dir).unwrap_err();
        assert!(matches!(err, PinError::Mismatch { .. }));
    }
}
