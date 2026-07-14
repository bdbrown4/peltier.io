#!/bin/sh
# Pin an upstream test suite: hash every file under the given repo-root-
# relative paths (typically targets/<t>/workspace/tests/...) into
# corpora/<target>/TESTSUITE.sha256, sha256sum two-space format. diff-test
# verifies this pin before running upstream tests; targets/fetch.sh
# verifies it after every fetch. Run from repo root:
#   scripts/pin-testsuite.sh <target> <path>...
set -eu
if [ $# -lt 2 ]; then
    echo "usage: scripts/pin-testsuite.sh <target> <path>..." >&2
    exit 2
fi
target="$1"
shift
out="corpora/$target/TESTSUITE.sha256"
if [ ! -d "corpora/$target" ]; then
    echo "pin-testsuite: corpora/$target does not exist (run from repo root?)" >&2
    exit 1
fi
for p in "$@"; do
    if [ ! -e "$p" ]; then
        echo "pin-testsuite: $p not found (paths are repo-root-relative)" >&2
        exit 1
    fi
done

# Build the pin out-of-place and install it only on success: a run that fails
# anywhere below must not truncate, replace, or half-write an existing pin —
# that pin is what diff-test refuses to run against on mismatch.
list="$out.files.$$"
tmp="$out.new.$$"
trap 'rm -f "$list" "$tmp"' EXIT INT TERM

# LC_ALL=C: byte-order sort, so the same tree pins identically on every
# machine. -print0/-z: filenames with spaces or newlines survive intact.
find "$@" -type f -print0 | LC_ALL=C sort -z > "$list"
count=$(tr -dc '\0' < "$list" | wc -c | tr -d ' ')
if [ "$count" -eq 0 ]; then
    # Was: `find ... | xargs -0 sha256sum > "$out"` — with no matches, xargs
    # still ran sha256sum with zero file arguments, so it hashed STDIN (the
    # empty pipe) and wrote a bogus "<sha-of-empty>  -" entry, then reported
    # SUCCESS. That is a pin of nothing that later "verifies" by reading
    # stdin. Refuse instead, and leave any existing pin untouched.
    echo "pin-testsuite: no regular files under: $* — refusing to write $out" >&2
    exit 1
fi

# --text is GNU's default on Linux (a no-op there); it is spelled out because
# sha256sum defaults to *binary* mode on some platforms, emitting "<hash> *path"
# — a one-space-plus-asterisk form that diff-test's pin verifier (which splits
# on the two-space separator) does not parse. Paths stay repo-root-relative:
# find was handed repo-root-relative paths and we run from the repo root.
xargs -0 sha256sum --text -- < "$list" > "$tmp"

lines=$(wc -l < "$tmp" | tr -d ' ')
if [ "$lines" -ne "$count" ]; then
    echo "pin-testsuite: hashed $lines of $count files — refusing to write a partial pin" >&2
    exit 1
fi
mv "$tmp" "$out"
echo "wrote $out ($count files)"
echo "pinning a test suite is a deliberate human action (corpora/README.md):"
echo "commit the new pin with justification."
