# Class 2 — Allocator swap

Link-line only. With class 1, forms the "your 30% was a linker flag"
tier that is always exhausted before touching source.

## Preconditions
- Profile signature: `malloc`/`free`/`operator new` or allocator-internal
  symbols (`_int_malloc`, `tcache_*`, page-fault handling) ≥5% of
  exclusive samples; or high syscall time in `mmap`/`brk`; or
  multithreaded target with allocator lock contention visible.

## Procedure
1. Rust: add `mimalloc` (then `jemallocator`) as `#[global_allocator]` —
   two-line diff, still effectively link-level.
2. C/C++: `LD_PRELOAD` trial first (free experiment, no rebuild), then
   proper linking of the winner.
3. Bench each candidate allocator separately; do not stack with other
   changes.

## Verification notes
- Behavior-preserving by contract, so gates should pass trivially — but
  run them all anyway: allocator swaps change address-space layout and
  can unmask latent UB (use-after-free that "worked" under glibc). A
  sanitizer failure here is a pre-existing target bug: record it,
  route to `needs-human-review`, do not ship.
- Max RSS is captured per run: an allocator that wins time but doubles
  memory is a trade-off the report must state, not hide.

## Known failure modes
- Wins on the bench box's core count that vanish (or invert) at the
  fleet's concurrency level.
- `LD_PRELOAD` trial passing but static-link integration subtly failing
  (symbol interposition differences) — re-run full gates on the final
  link, not the trial.
- Fragmentation pathologies on long-running services that a short bench
  window never sees; service-mode targets need soak evidence (Phase 4).
