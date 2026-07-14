#!/bin/sh
# Reproduce a target workspace at its pinned commit (CI + fresh clones).
# Usage: targets/fetch.sh <name>   (run from repo root)
set -eu
name="$1"
toml="targets/$name/target.toml"
repo=$(grep -m1 '^repo = ' "$toml" | cut -d'"' -f2)
commit=$(grep -m1 '^commit = ' "$toml" | cut -d'"' -f2)
ws="targets/$name/workspace"
if [ ! -d "$ws/.git" ]; then
    git clone "$repo" "$ws"
fi
git -C "$ws" fetch -q origin "$commit" 2>/dev/null || git -C "$ws" fetch -q origin
git -C "$ws" checkout -q "$commit"
# Pinned submodules (comrak's bench corpus lives in one).
grep -A0 '^"vendor/' "$toml" | while IFS= read -r line; do
    sub=$(printf '%s' "$line" | cut -d'"' -f2)
    surl=$(printf '%s' "$line" | sed 's/.*repo = "\([^"]*\)".*/\1/')
    scommit=$(printf '%s' "$line" | sed 's/.*commit = "\([^"]*\)".*/\1/')
    [ -d "$ws/$sub/.git" ] || git clone "$surl" "$ws/$sub"
    git -C "$ws/$sub" fetch -q origin "$scommit" 2>/dev/null || true
    git -C "$ws/$sub" checkout -q "$scommit"
done
# Upstream test-suite pin: verify against repo root when present; a
# mismatch means the fetched suite is not the one that was audited.
suite="corpora/$name/TESTSUITE.sha256"
if [ -f "$suite" ]; then
    if ! sha256sum -c --quiet "$suite"; then
        echo "fetch: upstream test-suite pin MISMATCH for $name ($suite) — refusing" >&2
        exit 1
    fi
    echo "$name test suite verified against $suite"
else
    echo "note: no $suite — pin the suite with: scripts/pin-testsuite.sh $name <paths...>"
fi
echo "$name pinned at $commit"
