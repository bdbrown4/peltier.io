//! Diff-header path vetting: the allowlist between agent-authored diffs
//! and `git apply` in the target workspace. Portable (no Unix
//! dependencies) so the escape-regression tests run on every development
//! platform, not just the daemon's POSIX hosts.

use anyhow::{anyhow, ensure, Result};
use std::path::{Component, Path};

/// Relative paths only; no parent components, no .git, no absolutes.
pub fn check_rel_path(p: &str) -> Result<()> {
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

const RENAME_COPY_HEADERS: [&str; 4] = ["rename from ", "rename to ", "copy from ", "copy to "];

/// Appended to every `---`/`+++` rejection. That rule is the one place where a
/// hunk BODY line can be taken for a header — `-` and `+` are the body markers
/// themselves — and the bare error ("lacks the a/ or b/ prefix", "path escapes
/// workspace") is baffling if all you did was delete a comment. Say the cause
/// out loud. No other header rule can collide: a body line always carries a
/// `-`, `+` or space marker, so it can never begin with `diff --git ` or
/// `rename `.
const HEADER_COLLISION_NOTE: &str = "\
    note: this line was vetted as a `---`/`+++` file header. The scanner does not track hunk \
    boundaries — deliberate, and it stays that way: a real header must never slip past it as body \
    text — so a hunk body line reaches this rule too. REMOVING a line whose own text begins with \
    `-- ` renders at column 0 as `--- ...` (an unindented SQL/Lua `-- comment`), and ADDING one \
    that begins with `++ ` renders as `+++ ...`. If that is what this line is, the patch is being \
    rejected on a false positive. Workaround: leave such lines untouched as context and edit \
    around them. Rewording or indenting the line does NOT help — the removal itself is what \
    renders as `--- `. Mind the space: `--x`/`++x` (C decrement/increment) never collide.";

/// Vet every path-bearing header line of a unified git diff: `diff --git`,
/// `---`, `+++`, and the rename/copy extended headers. The whole diff is
/// rejected if any extracted path is absolute, contains a `..` component,
/// is quoted (git's C-style quoting can encode `..` invisibly), or — for
/// `---`/`+++` lines — lacks the standard `a/`/`b/` prefix (`--no-prefix`
/// output). Every line is scanned without tracking hunk boundaries, so a
/// content line colliding with a header prefix is vetted too: over-rejection
/// is the safe direction, and `git apply` remains the backstop.
pub fn check_diff_paths(diff: &str) -> Result<()> {
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            let (a, b) = split_git_header_paths(rest).ok_or_else(|| {
                anyhow!(
                    "cannot parse diff header {line:?}: expected \
                     `diff --git a/<path> b/<path>` with plain unquoted paths"
                )
            })?;
            vet_extracted(a)?;
            vet_extracted(b)?;
        } else if line.starts_with("--- ") || line.starts_with("+++ ") {
            vet_file_header(&line[4..]).map_err(|e| {
                anyhow!("{e}\n  offending line: {line:?}\n  {HEADER_COLLISION_NOTE}")
            })?;
        } else if let Some(rest) = RENAME_COPY_HEADERS
            .iter()
            .find_map(|pre| line.strip_prefix(pre))
        {
            vet_extracted(header_path(rest))?;
        }
    }
    Ok(())
}

/// Vet the path on a `---`/`+++` file-header line; `rest` is the text after
/// the four-byte marker. Errors are wrapped by the caller with
/// `HEADER_COLLISION_NOTE`, since any line reaching here *might* be a hunk
/// body line rather than a header.
fn vet_file_header(rest: &str) -> Result<()> {
    let p = header_path(rest);
    if p == "/dev/null" {
        return Ok(());
    }
    ensure!(
        !p.starts_with('"'),
        "quoted diff paths forbidden: {p} (rewrite the patch with plain ASCII paths)"
    );
    let stripped = p
        .strip_prefix("a/")
        .or_else(|| p.strip_prefix("b/"))
        .ok_or_else(|| {
            anyhow!(
                "diff path {p:?} lacks the a/ or b/ prefix — regenerate with \
                 standard `git diff` output (--no-prefix diffs are rejected)"
            )
        })?;
    vet_extracted(stripped)
}

/// Header paths may carry a traditional-diff `\t<timestamp>` suffix.
fn header_path(rest: &str) -> &str {
    rest.split('\t').next().unwrap_or(rest).trim_end()
}

/// `diff --git a/<x> b/<y>`: split at the LAST ` b/` so a ` b/` embedded
/// in <x> cannot truncate <y>; splitting only ever removes the three
/// bytes ` b/`, so a `..` component in either real path always survives
/// intact into one of the two fragments.
fn split_git_header_paths(rest: &str) -> Option<(&str, &str)> {
    let a_rest = rest.strip_prefix("a/")?;
    let idx = a_rest.rfind(" b/")?;
    Some((&a_rest[..idx], a_rest[idx + 3..].trim_end()))
}

fn vet_extracted(p: &str) -> Result<()> {
    if p == "/dev/null" {
        return Ok(());
    }
    ensure!(!p.is_empty(), "empty path in diff header");
    ensure!(
        !p.starts_with('"'),
        "quoted diff paths forbidden: {p} (rewrite the patch with plain ASCII paths)"
    );
    ensure!(
        !p.starts_with('/'),
        "absolute diff paths forbidden (only /dev/null is allowed, for new/deleted files): {p}"
    );
    check_rel_path(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_path_rules() {
        assert!(check_rel_path("src/lib.rs").is_ok());
        assert!(check_rel_path("./src/lib.rs").is_ok());
        assert!(check_rel_path("../outside").is_err());
        assert!(check_rel_path("a/../b").is_err());
        assert!(check_rel_path("/etc/passwd").is_err());
        assert!(check_rel_path(".git/config").is_err());
        assert!(check_rel_path("a/.git/hooks").is_err());
    }

    #[test]
    fn normal_diff_passes() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n\
                    index 1111111..2222222 100644\n\
                    --- a/src/lib.rs\n\
                    +++ b/src/lib.rs\n\
                    @@ -1,2 +1,2 @@\n\
                    -old\n\
                    +new\n";
        assert!(check_diff_paths(diff).is_ok());
    }

    #[test]
    fn new_and_deleted_file_dev_null_passes() {
        let diff = "diff --git a/newfile.rs b/newfile.rs\n\
                    new file mode 100644\n\
                    --- /dev/null\n\
                    +++ b/newfile.rs\n\
                    @@ -0,0 +1 @@\n\
                    +hello\n\
                    diff --git a/gone.rs b/gone.rs\n\
                    deleted file mode 100644\n\
                    --- a/gone.rs\n\
                    +++ /dev/null\n\
                    @@ -1 +0,0 @@\n\
                    -bye\n";
        assert!(check_diff_paths(diff).is_ok());
    }

    #[test]
    fn in_workspace_rename_passes() {
        let diff = "diff --git a/old.rs b/nested/new.rs\n\
                    similarity index 100%\n\
                    rename from old.rs\n\
                    rename to nested/new.rs\n";
        assert!(check_diff_paths(diff).is_ok());
    }

    /// The exact fixture from the apply-escape audit: a rename whose
    /// destination climbs out of the workspace. `---`/`+++` scanning alone
    /// never sees it — the paths live only in the `diff --git` and
    /// rename headers.
    #[test]
    fn rename_escape_fixture_rejected() {
        let diff = "diff --git a/src.txt b/../outside/escaped.txt\n\
                    similarity index 100%\n\
                    rename from src.txt\n\
                    rename to ../outside/escaped.txt\n";
        assert!(check_diff_paths(diff).is_err());
    }

    #[test]
    fn rename_headers_rejected_without_git_header_line() {
        let diff = "similarity index 100%\n\
                    rename from src.txt\n\
                    rename to ../outside/escaped.txt\n";
        assert!(check_diff_paths(diff).is_err());
    }

    #[test]
    fn copy_escape_rejected() {
        let diff = "diff --git a/src.txt b/src2.txt\n\
                    copy from src.txt\n\
                    copy to ../outside/copied.txt\n";
        assert!(check_diff_paths(diff).is_err());
    }

    #[test]
    fn no_prefix_diff_rejected() {
        let diff = "diff --git src/lib.rs src/lib.rs\n\
                    index 1111111..2222222 100644\n\
                    --- src/lib.rs\n\
                    +++ src/lib.rs\n\
                    @@ -1 +1 @@\n\
                    -old\n\
                    +new\n";
        let err = check_diff_paths(diff).unwrap_err().to_string();
        assert!(
            err.contains("diff --git") || err.contains("a/ or b/ prefix"),
            "{err}"
        );
    }

    #[test]
    fn absolute_path_diff_rejected() {
        let diff = "diff --git a/x b/x\n\
                    rename from x\n\
                    rename to /etc/passwd\n";
        assert!(check_diff_paths(diff).is_err());
        let plus = "--- a/x\n\
                    +++ /etc/passwd\n";
        assert!(check_diff_paths(plus).is_err());
    }

    #[test]
    fn parent_component_in_file_headers_rejected() {
        assert!(check_diff_paths("--- a/../escape.rs\n+++ b/escape.rs\n").is_err());
        assert!(check_diff_paths("--- a/ok.rs\n+++ b/nested/../../escape.rs\n").is_err());
        assert!(check_diff_paths("diff --git a/ok.rs b/../escape.rs\n").is_err());
    }

    #[test]
    fn dot_git_path_rejected() {
        let diff = "diff --git a/.git/hooks/pre-commit b/.git/hooks/pre-commit\n\
                    --- a/.git/hooks/pre-commit\n\
                    +++ b/.git/hooks/pre-commit\n";
        assert!(check_diff_paths(diff).is_err());
    }

    #[test]
    fn quoted_paths_rejected() {
        assert!(check_diff_paths("diff --git \"a/x\" \"b/y\"\n").is_err());
        assert!(check_diff_paths("--- \"a/x\"\n+++ b/y\n").is_err());
        assert!(check_diff_paths("rename to \"\\056\\056/evil\"\n").is_err());
    }

    #[test]
    fn embedded_b_slash_cannot_hide_escape() {
        // Whatever way ` b/` splits this header, the `..` lands in a fragment.
        assert!(check_diff_paths("diff --git a/x b/y b/../evil\n").is_err());
        assert!(check_diff_paths("diff --git a/../evil b/y b/z\n").is_err());
    }

    /// A removed body line that renders as `--- ` at column 0 — deleting an
    /// unindented SQL/Lua `-- comment` — is still REJECTED (fail-closed is the
    /// point; tracking hunk boundaries here would risk desyncing from git's own
    /// parser). But the error must name that cause instead of leaving an agent
    /// staring at a complaint about a/ and b/ prefixes it never wrote.
    #[test]
    fn body_line_colliding_with_header_prefix_explains_itself() {
        let diff = "diff --git a/q.sql b/q.sql\n\
                    --- a/q.sql\n\
                    +++ b/q.sql\n\
                    @@ -1,2 +1 @@\n\
                    --- drop this comment\n\
                    \x20SELECT 1;\n";
        let err = check_diff_paths(diff).unwrap_err().to_string();
        assert!(err.contains("--- drop this comment"), "{err}");
        assert!(err.contains("hunk body line"), "{err}");
        assert!(err.contains("context"), "{err}");
    }

    /// The collision needs the SPACE. A removed `--x;` renders as `---x;` and an
    /// added `++x;` as `+++x;` — neither is a `--- `/`+++ ` header, and both
    /// must sail through. Guards against anyone "fixing" the note above by
    /// widening the prefix match.
    #[test]
    fn body_line_without_the_space_does_not_collide() {
        let diff = "diff --git a/x.c b/x.c\n\
                    --- a/x.c\n\
                    +++ b/x.c\n\
                    @@ -1,2 +1,2 @@\n\
                    ---x;\n\
                    +++x;\n";
        assert!(check_diff_paths(diff).is_ok());
    }

    #[test]
    fn timestamp_suffix_tolerated() {
        let diff = "--- a/src/lib.rs\t2026-07-13 00:00:00\n\
                    +++ b/src/lib.rs\t2026-07-13 00:00:00\n";
        assert!(check_diff_paths(diff).is_ok());
    }
}
