#!/bin/sh
# Kernel-lane demonstration (SPEC §13 research fork). Shows the one thing
# the parser/service targets never needed: an optimization that reorders
# floating-point accumulation, so byte-identical golden replay is the WRONG
# gate and a floating-point TOLERANCE gate is required — while a genuine
# wrong result is still caught. Measured with the same interleaved A/B +
# bootstrap CI as every other target.
#
#   sh scripts/kernel-lane-demo.sh [n]
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
N=${1:-512}
OUT="$ROOT/results/phase5"
mkdir -p "$OUT" /tmp/kl

cd "$ROOT"
clang -O2 -o /tmp/kl/kernel targets/matmul/kernel.c -lm
cargo build -q -p diff-test -p bench-runner

{
echo "# Kernel-lane demonstration — matmul (n=$N, f32)"
echo
echo "## 1. Equivalence: byte-identical is the wrong gate"
/tmp/kl/kernel "$N" emit ref > /tmp/kl/ref.txt
/tmp/kl/kernel "$N" emit opt > /tmp/kl/opt.txt
if diff -q /tmp/kl/ref.txt /tmp/kl/opt.txt >/dev/null; then
    echo "byte-identical: MATCH (unexpected — no reorder)"
else
    d=$(diff /tmp/kl/ref.txt /tmp/kl/opt.txt | grep -c '^<' || true)
    echo "byte-identical: FAIL — $d of $((N*N)) result values differ (accumulation reordered)"
fi
echo
echo "diff-test FP-tolerance policy (targets/matmul/equivalence.toml):"
"$ROOT/target/debug/fp-compare" targets/matmul /tmp/kl/ref.txt /tmp/kl/opt.txt | sed 's/^/  /'
echo
echo "in-process reference differential test:"
/tmp/kl/kernel "$N" check 1e-4 1e-3 | sed 's/^/  /'
echo
echo "control — a genuine wrong result IS caught by the tolerance:"
awk 'NR==1{$1=$1+0.5} {print}' /tmp/kl/opt.txt > /tmp/kl/wrong.txt
if "$ROOT/target/debug/fp-compare" targets/matmul /tmp/kl/ref.txt /tmp/kl/wrong.txt >/dev/null 2>&1; then
    echo "  !! tolerance accepted a wrong result (bug)"
else
    echo "  tolerance correctly REJECTS a +0.5 perturbation of one element"
fi
echo
echo "## 2. Speedup: interleaved A/B, bootstrap 95% CI"
cargo run -q -p bench-runner -- --config config/accept.toml compare \
    --baseline "taskset -c 2 /tmp/kl/kernel $N run ref" \
    --candidate "taskset -c 2 /tmp/kl/kernel $N run opt" 2>/dev/null \
    | grep -E "speedup|verdict" | sed 's/^/  /'
echo
echo "## What it demonstrates"
echo "A real matmul speedup (transpose for cache locality + eight-accumulator"
echo "ILP) that reorders the reduction. Byte-identical rejects it; the"
echo "FP-tolerance gate accepts it (abs 1e-4 + rel 1e-3) while still catching"
echo "a wrong result; the bench measures it with the same CI machinery as"
echo "every other target. The GPU extension (Triton/CUDA vs a reference"
echo "kernel) is the same shape — only the timer and the hardware change."
} | tee "$OUT/kernel-lane.txt"
