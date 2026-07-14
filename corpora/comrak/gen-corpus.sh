#!/bin/sh
# Regenerate the comrak bench corpus from the pinned progit checkout
# (targets/comrak/target.toml [source.submodules]), then verify the pin.
# Default/--check verifies MANIFEST.sha256 (never rewrites it); --pin
# deliberately rewrites it.
set -eu
mode="check"
case "${1:-}" in
    "" | --check) ;;
    --pin) mode="pin" ;;
    *) echo "usage: gen-corpus.sh [--check|--pin]" >&2; exit 2 ;;
esac
cd "$(dirname "$0")"
cat ../../targets/comrak/workspace/vendor/progit/*/*/*.markdown > progit.md
if [ "$mode" = "pin" ]; then
    sha256sum progit.md > MANIFEST.sha256
    echo "re-pinned MANIFEST.sha256 — a deliberate human action (corpora/README.md);"
    echo "commit the new manifest with justification."
else
    sha256sum -c MANIFEST.sha256
fi
