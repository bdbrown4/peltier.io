# Phase 5 — the research forks (SPEC §13)

Phase 5 exercises the two frontier ideas the spec deliberately left out of
the core roadmap. Neither adds a new product; each is a stress test that
answers one question: **does the trust machinery still hold when the shape
of the work changes?** The answer, both times, is yes — the gates, the
interleaved A/B timer, and the append-only ledger transfer unchanged; only
the *gate policy* (5b) or the *ordering prior* (5a) is new.

Nothing here relaxes a non-negotiable. Both forks route their headline
result to `needs-human-review`, and the learned prior is advisory only —
the gates still decide every verdict.

---

## 5a — learned class-selection policy

**Artifact:** `crates/policy/`, output in `learned-policy.txt`.

The ledger is now 34 rows of `(playbook class, target) → verdict`. That is
a dataset. Phase 5a turns the *fixed* cheapest-first ordering of the
playbook into a *learned* prior: rank each optimization class by the
**Wilson score lower bound** of its observed shippable-win rate, and try the
best-evidenced class first on a new hotspot.

Why the Wilson lower bound and not the raw win rate: a class that went 1-for-1
should not outrank a class that went 3-for-16. The Wilson lower bound
penalizes small samples automatically — it is the same "be pessimistic
about thin evidence" instinct as the bootstrap CI lower bound the bench
uses to accept a speedup. Ties break *untried-before-failed* (an unexplored
class keeps its cheapest-first prior; a class with evidence *against* it
sinks), then cheapest-first.

Two guards keep it honest, both visible in the output footer:

- A "win" counts **only** a machine-sanitizer-verified accept. The
  overturned `comrak-010` false-accept and every pre-sanitizer-gate accept
  are excluded — the policy trusts the ledger's machine record, not the
  audit narrative that caught them.
- The tier-gated allocator win (mimalloc) shows as `held for human review`,
  not as a loss and not as a shippable win — an accurate third category.

Current ranking (all targets, 34 rows):

| rank | class | attempts | wins | wilson-lb | note |
|---|---|---|---|---|---|
| 1 | algorithmic | 16 | 3 | 0.066 | proven winner |
| 2 | alloc-churn | 4 | 1 | 0.046 | proven winner |
| 3 | build-config | 9 | 1 | 0.020 | proven winner |
| 4 | concurrency | 0 | 0 | 0.000 | untried — cheapest-first prior only |
| 5 | allocator | 2 | 0 | 0.000 | held for human review (tier-gated) |
| 6 | data-layout | 1 | 0 | 0.000 | evidence against |
| 7 | simd | 2 | 0 | 0.000 | evidence against |

The recommendation the agent would act on: **try algorithmic first** (best
evidence), then the next-ranked class whose profile preconditions match.
This is exactly the prior art direction (MLGO, AlphaDev, LLM Compiler) at
the scale the ledger currently supports — a ranked prior, sharpening as the
dataset grows, never overriding a gate.

Reproduce: `cargo run -p policy` (add `--target cjson` to scope to one
target).

---

## 5b — the kernel lane: FP-tolerance equivalence

**Artifact:** `targets/matmul/`, `crates/diff-test/src/bin/fp-compare.rs`,
demo in `kernel-lane.txt`.

Every target through Phase 4 was gated by **byte-identical** golden replay:
the optimized output must match the baseline bit for bit. That is the right
gate for a parser or an HTTP service. It is the *wrong* gate for a numerical
kernel, and the kernel lane is the case that proves it.

`targets/matmul/kernel.c` is a single-precision matrix multiply in two
implementations:

- `matmul_ref` — naive `i,j,k` with one sequential accumulator. The oracle.
- `matmul_opt` — transpose B (so the inner reduction walks contiguous
  memory: the cache win) **and** eight independent accumulators tree-combined
  at the end (breaking the float-add dependency chain: the ILP win). The
  second lever **reorders the reduction**, so the last few ULPs differ *by
  construction*. Bit-identical output is impossible.

The demo (`sh scripts/kernel-lane-demo.sh 512`) walks the whole argument:

1. **byte-identical FAILs** — 244,901 of 262,144 result values differ. Under
   the Phase 0–4 gate, this correct 3× speedup would be *rejected*.
2. **the FP-tolerance policy is EQUIVALENT** — `equivalence.toml` declares
   `mode = "fp-tolerance"`, `abs = 1e-4`, `rel = 1e-3`; every numeric token
   agrees within `abs + rel·|ref|`.
3. **a genuine wrong result is still caught** — perturbing one element by
   `+0.5` is REJECTED by the same tolerance. The gate is loose enough for
   last-ULP reordering, tight enough for a real bug.
4. **the speedup is measured with the same machinery** — interleaved A/B,
   bootstrap 95% CI: **median 3.23×, CI [3.16, 3.26]**.

The equivalence policy (`EquivalencePolicy::compare` in `diff-test`) is where
the new rule lives: byte-identical stays exact; fp-tolerance tokenizes,
compares numeric tokens within tolerance, handles NaN explicitly, and keeps
non-numeric tokens and token counts exact. `fp-compare` applies a target's
policy to two output files from the command line.

**Verdict: `needs-human-review`** (`phase5-matmul-opt` ledger row). Using
the FP-tolerance tier *at all* is a human-review signal per SPEC §8 — a
person confirms the declared tolerance is defensible for the workload,
exactly as with the mimalloc allocator swap. The machine measures and
gates; the human ratifies the tolerance.

### Why this is the GPU lane in miniature

The environment has no GPU, so the GPU fork is demonstrated on the CPU as
what it actually is: **the same trust machinery with a different timer.** A
Triton or CUDA kernel versus a reference kernel is the identical shape —
reference-kernel differential testing within a tolerance, interleaved timing
with a bootstrap CI, a ledger row routed to human review. Only the profiler
and the bench clock become kernel-time-aware, and the hardware changes. The
correctness story, the statistics, and the audit trail are already built and
already exercised here on real code.

Reproduce: `sh scripts/kernel-lane-demo.sh 512`.

---

## What Phase 5 settles

The core loop was already complete at Phase 4. Phase 5 answers the "but does
it generalize?" objection with the two hardest cases the spec named:

- A **learned prior** can be extracted from the ledger without weakening a
  single gate — it only reorders *what to try first*.
- A **numerically-reordered optimization** — the class byte-identical replay
  cannot handle — slots into the same harness by swapping the equivalence
  policy, with the correctness bar preserved (wrong results still caught) and
  the tolerance itself put under human review.

Zero shipped false accepts, still. The product is trust, and trust
transferred.

---

## Erratum (2026-07-13) — how far the kernel lane is actually wired

The write-up above describes the FP-tolerance gate as though `matmul` runs
through the standard pipeline. It does not, and the distinction matters:

- **The policy is in the pipeline.** `EquivalencePolicy` is honored by
  `diff-test`'s gate sequence (an fp-tolerance target needs a committed
  `[corpus].golden_reference`; the gate fails closed without one), and
  `crates/verdict` treats fp-tolerance equivalence as an automatic
  `needs-human-review` signal. That machinery is real and unit-tested.
- **`matmul` is not a pipeline target.** It ships `targets/matmul/kernel.c`
  and `targets/matmul/equivalence.toml` but **no `target.toml`** — the file
  `diff-test`/`verdict` load a target from. So the lane cannot currently be
  run via `just gates` or `just verdict` at all.
- **Therefore the demonstration is script-driven.**
  `scripts/kernel-lane-demo.sh` invokes the `fp-compare` binary and
  `bench-runner compare` directly, and the `phase5-matmul-opt` ledger row
  was recorded **outside** the automated verdict path.

Nothing in the measured result changes — the 3.23× [3.16, 3.26] speedup, the
244,901/262,144 differing values, the rejected +0.5 perturbation, and the
`needs-human-review` verdict all stand as printed. What was overstated is the
*integration*: the equivalence policy is closed; wiring this target into the
pipeline is open work. The original text stands unedited above; this note is
the correction.
