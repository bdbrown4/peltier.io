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
echo "$name pinned at $commit"
