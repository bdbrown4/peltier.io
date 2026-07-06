# Class 4 — Data layout

Make the memory hierarchy work for the access pattern.

## Preconditions
- Profile signature: high cache-miss rate in `perf stat` (LLC misses per
  instruction), hot loops whose flamegraph frames are memory-bound
  (`perf` cycles concentrated on loads), low IPC (<1) on wide cores.
- For false sharing: multithreaded target where per-thread throughput
  degrades as threads are added; `perf c2c` confirms.

## Procedure (cheapest first)
1. Struct field reordering / packing: kill padding, group hot fields on
   one cache line, split hot/cold (`#[repr(C)]` review, cold-field
   box-out).
2. AoS→SoA for loops that touch one field across many elements —
   restructure the container, not the algorithm.
3. False-sharing fixes: pad or align per-thread state to 64 bytes
   (`#[repr(align(64))]`, `crossbeam_utils::CachePadded`).
4. Cache blocking / loop tiling for nested loops over large matrices or
   images; tile to L1/L2, verified empirically, not by datasheet.

## Verification notes
- Layout changes to any type that crosses FFI, serialization, or hashing
  boundaries can change observable behavior (struct hash, serialized
  bytes, memcmp-based equality) — golden replay catches this; check the
  target for `#[repr]`-sensitive traits before patching.
- Differential fuzz the functions whose types changed, not just the
  functions whose code changed.

## Known failure modes
- Reordering fields of a type that derives `Hash`/`Ord` used in ordered
  iteration → output order changes → replay divergence (correct reject).
- AoS→SoA wins the target loop but loses every other access site; the
  end-to-end number is the verdict, not the microbench.
- Alignment padding blowing up per-element size until the working set
  falls out of cache — the cure worse than the disease.
