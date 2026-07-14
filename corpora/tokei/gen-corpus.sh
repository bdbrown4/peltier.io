#!/bin/sh
# Regenerate the tokei bench input: 8 replicas of two pinned trees
# (progit @ 61833a5, comrak src/ @ 45c1995), .git removed. Verifies the
# manifest for copy1; copies 2-8 are byte-identical replicas of copy1.
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
rm -rf input
mkdir -p input/copy1
cp -r ../../targets/comrak/workspace/vendor/progit input/copy1/progit
cp -r ../../targets/comrak/workspace/src input/copy1/src
rm -rf input/copy1/progit/.git
if [ "$mode" = "pin" ]; then
    (cd input/copy1 && find . -type f -print0 | sort -z | xargs -0 sha256sum) > MANIFEST.sha256
    echo "re-pinned MANIFEST.sha256 — a deliberate human action (corpora/README.md);"
    echo "commit the new manifest with justification."
else
    (cd input/copy1 && sha256sum -c ../../MANIFEST.sha256 --quiet) || { echo "corpus pin FAILED"; exit 1; }
fi
for i in 2 3 4 5 6 7 8; do cp -r input/copy1 "input/copy$i"; done
