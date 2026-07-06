# Class 7 — Concurrency

Last in the order, highest risk. **Every diff in this class routes to
`needs-human-review`** (SPEC §8); the agent proposes, a human disposes.

## Preconditions
- Profile signature: multithreaded target where wall time ≫ ideal
  (cpu-time / cores); lock symbols (`futex`, `pthread_mutex`,
  `parking_lot`) hot; run-queue latency; per-thread throughput that
  *drops* as threads are added. coz (Phase 3) is the right prioritizer
  here — cycle profiles mislead on blocked time.

## Procedure (cheapest first)
1. Shrink critical sections: move allocation, I/O, and computation out
   from under locks; drop guards early.
2. Reduce sharing: per-thread accumulation + merge at the end beats a
   shared counter; sharding a hot map by key hash beats a fancier lock.
3. Right-size the primitive: `RwLock` for read-mostly, `OnceLock` for
   init-once, atomics for simple counters *with explicitly justified
   orderings* (start from `SeqCst`; relax only with a written argument).
4. Lock-free structures: last resort, library-only (crossbeam), never
   hand-rolled, and only when the profile proves the lock is the
   bottleneck after 1–3.

## Verification notes
- TSan gate is mandatory for this class, on both the upstream suite and
  the golden replay run (SPEC §3.2).
- Thread-count sweep: gates and bench at 1, 2, N, and 2N threads —
  concurrency bugs and wins are both thread-count-dependent.
- Nondeterministic output ordering (e.g. results gathered from threads)
  must be canonicalized at the boundary or the change rejected; replay
  equivalence is not negotiable to make a parallel win look clean.

## Known failure modes
- Benchmarks too short to hit the contention window: a race that fires
  once per million ops passes 60s of fuzz. TSan + long soak, and still
  route to a human.
- Relaxed atomics that are correct on x86's strong model and wrong on
  ARM — orderings need an argument, not a test pass.
- Sharding that wins the benchmark's uniform key distribution and loses
  production's skewed one — holdout workloads (SPEC §11).
- Deadlocks introduced by lock-order changes that no gate exercises;
  the human reviewer checks lock ordering explicitly.
