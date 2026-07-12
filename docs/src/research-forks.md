# Research forks: kernel lane & learned policy

The core loop was complete at Phase 4 — profile → hypothesize → patch →
gated-verdict → ROI, running unattended across Rust and C, batch and
service, with zero shipped false accepts. SPEC §13 named two frontier ideas
that the roadmap deliberately excluded, precisely because they change the
*shape* of the work rather than extend the loop. Phase 5 built both, as
stress tests with one question each: **does the trust machinery still hold?**

Both times it does. Neither fork relaxes a non-negotiable; each routes its
headline result to `needs-human-review`, and the learned prior is advisory
only. What follows is the argument for each.

## The kernel lane: when byte-identical is the wrong gate

Every target through Phase 4 was gated by **byte-identical golden replay** —
the optimized output must match the baseline bit for bit. For a parser or an
HTTP service that is exactly right. For a numerical kernel it is wrong, and
the kernel lane is the case that proves it.

`targets/matmul/kernel.c` is a single-precision matrix multiply in two forms:

- **`matmul_ref`** — naive `i,j,k`, one sequential accumulator. The oracle.
- **`matmul_opt`** — transpose B so the inner reduction walks contiguous
  memory (the cache win), plus eight independent accumulators tree-combined
  at the end (the ILP win, breaking the float-add dependency chain). The
  second lever **reorders the reduction**, so the last few ULPs differ *by
  construction*. Bit-identical output is impossible.

The demonstration (`sh scripts/kernel-lane-demo.sh 512`) walks the argument:

1. **byte-identical FAILs** — 244,901 of 262,144 values differ. Under the
   Phase 0–4 gate this correct 3× speedup would be *rejected*.
2. **FP-tolerance is EQUIVALENT** — the target declares `mode =
   "fp-tolerance"`, `abs = 1e-4`, `rel = 1e-3`; every numeric token agrees
   within `abs + rel·|ref|`.
3. **a wrong result is still caught** — perturbing one element by `+0.5` is
   REJECTED by the same tolerance. Loose enough for last-ULP reordering,
   tight enough for a real bug.
4. **measured with the same machinery** — interleaved A/B, bootstrap 95% CI:
   **median 3.23×, CI [3.16, 3.26]**.

The new rule lives in one place — `EquivalencePolicy::compare` in
`diff-test`. Byte-identical stays exact; fp-tolerance tokenizes, compares
numeric tokens within tolerance, handles NaN explicitly, and keeps
non-numeric tokens and token counts exact. The `fp-compare` binary applies a
target's `equivalence.toml` to two output files from the command line.

The verdict is **`needs-human-review`**. Using the FP-tolerance tier *at
all* is a human-review signal (SPEC §8): a person confirms the declared
tolerance is defensible for the workload, exactly as with the mimalloc
allocator swap. The machine measures and gates; the human ratifies the
tolerance.

**This is the GPU lane in miniature.** The environment has no GPU, so the
GPU fork is shown on the CPU as what it actually is — the same trust
machinery with a different timer. A Triton or CUDA kernel versus a reference
kernel is the identical shape: reference-kernel differential testing within
a tolerance, interleaved timing with a bootstrap CI, a ledger row routed to
human review. Only the profiler and the bench clock become
kernel-time-aware, and the hardware changes. The correctness story, the
statistics, and the audit trail are already built and exercised here on real
code.

## The learned policy: the ledger as a training set

The playbook orders optimization classes cheapest-first — a fixed prior. But
the ledger is now dozens of rows of `(class, target) → verdict`. That is a
dataset, and it can turn the fixed ordering into a *learned* one.

`crates/policy` ranks each class by the **Wilson score lower bound** of its
observed shippable-win rate. The Wilson lower bound is the same instinct as
the bootstrap CI lower bound the bench uses to accept a speedup: be
pessimistic about thin evidence. A class that went 1-for-1 should not
outrank one that went 3-for-16 — and it does not, because the lower bound
penalizes small samples automatically. Ties break untried-before-failed (an
unexplored class keeps its cheapest-first prior; a class with evidence
*against* it sinks), then cheapest-first.

Two guards keep it honest, both printed in the output footer:

- A "win" counts **only** a machine-sanitizer-verified accept. The
  overturned `comrak-010` false-accept and every pre-sanitizer-gate accept
  are excluded — the policy trusts the ledger's machine record, not the
  audit narrative that caught them.
- The tier-gated mimalloc win shows as *held for human review* — a third
  category, neither a loss nor a shippable win.

On the current 34-row ledger the ranking recommends **algorithmic first**
(best evidence, Wilson lb 0.066), then the next-ranked class whose profile
preconditions match. It is advisory: the equivalence and significance gates
still decide every verdict. This is the MLGO / AlphaDev / LLM-Compiler
direction at the scale the ledger currently supports — a ranked prior that
sharpens as the dataset grows and never overrides a gate.

Run it with `cargo run -p policy` (add `--target <name>` to scope the
evidence to one target).

## What the forks settle

- A **learned prior** can be extracted from the ledger without weakening a
  single gate — it only reorders *what to try first*.
- A **numerically-reordered optimization** — the class byte-identical replay
  cannot handle — slots into the same harness by swapping the equivalence
  policy, with the correctness bar preserved and the tolerance itself put
  under human review.

The full write-up, with reproduction commands and the raw tool output, is in
[`results/phase5/`](https://github.com/bdbrown4/peltier.io/tree/main/results/phase5).
