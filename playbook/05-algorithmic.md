# Class 5 — Algorithmic

Complexity class and primitive selection. Highest variance: the biggest
wins live here, and so do the most seductive wrong ideas.

## Preconditions
- Profile signature: one dominant frame that is *algorithm*, not memory
  (high IPC, low miss rate, still slow); superlinear scaling of runtime
  with input size; hot `contains`/`find` over linear structures; repeated
  recomputation of pure functions on repeated arguments.

## Procedure (cheapest first)
1. Better std/library primitive: `HashMap` for linear scans, `sort_unstable`
   over `sort`, `entry()` API over get-then-insert, buffered I/O,
   `memchr`-class scanning.
2. Precomputation: hoist invariant work out of loops; build lookup tables
   at startup when the domain is small.
3. Memoization for pure hot functions with high argument repeat rates —
   confirm the repeat rate from a counting instrument before adding a
   cache.
4. Complexity-class replacement (the real prize): O(n²) → O(n log n)
   with a known-correct algorithm. Prefer a well-tested library
   implementation over a hand-rolled one, always.

## Verification notes
- HashMap iteration order is not deterministic order: if results flow
  from iteration order into output, replay diverges — either sort at the
  boundary or reject the change.
- Memoization + interior mutability in multithreaded targets touches the
  concurrency review tier (SPEC §8).
- Differential fuzz is load-bearing here: algorithmic replacements have
  edge-case surfaces (empty input, duplicates, already-sorted, unicode)
  where "equivalent" algorithms differ. 10k iterations minimum, seeded
  with corpus-derived cases.

## Known failure modes
- Asymptotic win, constant-factor loss at real input sizes — the ledger
  records the workload; the verdict is measured at that workload.
- Caches without bounds: memoization that trades a 10% speedup for
  unbounded RSS growth. Check max RSS; bound the cache.
- "Equivalent" floating-point reformulations that aren't (different
  rounding paths) — FP output changes route to the tolerance policy or
  human review, never silently absorbed.
