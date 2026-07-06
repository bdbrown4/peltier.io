#!/bin/sh
# Regenerate the tokei bench input: 8 replicas of two pinned trees
# (progit @ 61833a5, comrak src/ @ 45c1995), .git removed. Verifies the
# manifest for copy1; copies 2-8 are byte-identical replicas of copy1.
set -eu
cd "$(dirname "$0")"
rm -rf input
mkdir -p input/copy1
cp -r ../../targets/comrak/workspace/vendor/progit input/copy1/progit
cp -r ../../targets/comrak/workspace/src input/copy1/src
rm -rf input/copy1/progit/.git
(cd input/copy1 && sha256sum -c ../../MANIFEST.sha256 --quiet) || { echo "corpus pin FAILED"; exit 1; }
for i in 2 3 4 5 6 7 8; do cp -r input/copy1 "input/copy$i"; done
