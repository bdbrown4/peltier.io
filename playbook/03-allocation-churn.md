# Class 3 — Allocation churn

First source-touching class: reduce the *number* of allocations, not the
allocator servicing them.

## Preconditions
- Class 2 helped but allocator symbols still hot, or allocation shows up
  as cache misses from pointer-chasing freshly allocated objects.
- Profile signature: allocation inside hot loops; `Vec`/`String`
  reallocation churn; many short-lived objects of the same type.

## Procedure (cheapest first)
1. Hoist allocations out of loops; reuse buffers via `clear()` instead
   of re-creation.
2. `Vec::with_capacity` / `reserve` where the size is known or bounded.
3. Small-vector types (`smallvec`, fixed inline buffers) for collections
   that are almost always tiny — confirm the size distribution from the
   profile or a counting instrument first.
4. Arena allocation (`bumpalo`, typed arenas) for phase-scoped object
   graphs with a clear reset point.

## Verification notes
- Buffer reuse is where stale-data bugs come from: golden replay must
  cover inputs of *decreasing* size across a session so a dirty buffer
  would surface. Add such a sequence to the corpus if missing.
- Arenas change drop order; targets with observable `Drop` side effects
  (logging, flushing) can diverge on golden replay — that divergence is
  a real reject, not noise.

## Known failure modes
- `with_capacity` guesses that overshoot: RSS regression the wall-clock
  number won't show — check the captured max RSS.
- smallvec inline sizes chosen by vibes: an inline buffer bigger than
  the cache line it saves. Measure, don't guess.
- Reuse introducing borrow-checker contortions that force `unsafe` —
  `unsafe` introduction routes to `needs-human-review` (SPEC §8).
