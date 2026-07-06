# Class 6 — SIMD

Autovectorization enablement first, intrinsics last (SPEC §4).

## Preconditions
- Profile signature: hot, trip-count-heavy inner loops over contiguous
  numeric/byte data; scalar math or byte-wise processing in the top
  frames; disassembly of the hot loop shows scalar ops where vector ops
  are plausible.

## Procedure (cheapest first)
1. Read the optimizer's mind: check the hot loop's assembly (or
   `-Cremark`/`-Rpass=loop-vectorize`) for *why* vectorization failed.
2. Remove the blocker, in order of cheapness:
   - Bounds checks: restructure with slices/iterators or hoist a single
     length check (`&x[..n]` pattern) — not `unsafe` indexing.
   - Aliasing: split borrows, `split_at_mut`, pass slices instead of
     overlapping raw pointers.
   - Loop-carried dependencies: reassociate integer reductions, unroll
     accumulators.
3. Widen the ISA baseline if class 1 hasn't already (`target-cpu`), since
   SSE2-only baselines strangle the vectorizer.
4. `std::simd` / portable-SIMD explicit vectorization.
5. Raw intrinsics only when all of the above fail and the win is large;
   isolate behind a scalar-fallback path selected at runtime.

## Verification notes
- **FP flags are a hard line**: any `-ffast-math`-class flag, FP
  reassociation, or FMA-contraction change routes to
  `needs-human-review` — no exceptions, even for a 2x win (SPEC §8).
  Integer/byte SIMD has no such taint and can auto-accept.
- Intrinsics imply `unsafe` → automatic `needs-human-review` + MIRI
  where feasible + differential fuzz against the scalar path with
  adversarial lengths (0, 1, lane-1, lane, lane+1, unaligned offsets).

## Known failure modes
- Tail handling: the classic SIMD bug class. Fuzz lengths around lane
  boundaries explicitly.
- Alignment assumptions that hold on the bench box and fault (or crawl)
  elsewhere.
- Runtime feature detection done per-call in the hot loop, eating the
  win; hoist detection to init.
- Integer reduction reassociation is safe; float reduction reassociation
  changes results — do not confuse the two.
