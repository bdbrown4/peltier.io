#!/bin/sh
# coz causal profiling for a C/C++ target (SPEC §12). Unlike a flat
# profiler (callgrind ranks where time is SPENT), coz measures where a
# speedup would actually RAISE throughput — it virtually speeds up each
# line and reports the causal effect on the harness's progress points.
# Use it to rank playbook targets when the flat profile is ambiguous.
#
#   scripts/coz-profile.sh cjson [iters]
#
# Requires: coz on PATH (build: git clone github.com/plasma-umass/coz &&
# see scripts/install-coz.sh) and the target's harness built with
# -DHOTPATH_COZ (this script does that). Output: results/<target>/coz/
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
TARGET=${1:?usage: coz-profile.sh <target> [iters]}
ITERS=${2:-200}
OUT="$ROOT/results/$TARGET/coz"
mkdir -p "$OUT"

command -v coz >/dev/null 2>&1 || {
    echo "coz not on PATH — run scripts/install-coz.sh first" >&2
    exit 1
}

# coz needs debug line info and the progress point compiled in.
BIN="$OUT/cjson-bench-coz"
clang -O2 -g -gdwarf-4 -DHOTPATH_COZ \
    -I "$ROOT/targets/$TARGET/workspace" \
    -I "$(dirname "$(command -v coz)")/../include" \
    -o "$BIN" \
    "$ROOT/targets/$TARGET/harness.c" \
    "$ROOT/targets/$TARGET/workspace/cJSON.c" \
    -ldl

cd "$ROOT"
coz run --output "$OUT/profile.coz" --- \
    "$BIN" corpora/$TARGET/input/big.json "$ITERS" >/dev/null

echo "coz profile written: $OUT/profile.coz"
echo "view at https://plasma-umass.org/coz/ (load the .coz file) or:"
echo "  python3 scripts/coz-summary.py $OUT/profile.coz"
