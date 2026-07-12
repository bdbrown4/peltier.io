# Equivalence gates

A faster binary that behaves differently is not an optimization — it is a
regression with good marketing. Before any speedup is even measured, the
candidate must prove it does the same thing the baseline does. These are
**hard gates**: a failure is `rejected-gate` and the bench never runs.

## The layers

1. **Upstream test suite.** The target's own tests, run against the patched
   code. For cJSON that is the vendored Unity/CTest suite (19 tests:
   `parse_number`, `parse_string`, `print_*`, `misc_tests`, …); for the Rust
   targets, `cargo test`.

2. **Golden replay — byte-identical.** The candidate processes the
   hash-pinned corpus and its output is hashed and compared to a pinned
   `GOLDEN.sha256`. A code counter that miscounts one line, a JSON printer
   that serializes one float differently — caught here. The corpus itself
   is pinned via `MANIFEST.sha256` and the harness refuses to run on a
   mismatch.

3. **Differential fuzzing.** Pristine vs candidate on thousands of mutated
   inputs, comparing output. Every accepted win cleared **10,000 mutated
   inputs with 0 divergences** — the tokei wins after canonicalizing benign
   parallel-walker output ordering (a real finding: differential gates must
   compare *semantics*, not bytes, where the target is legitimately
   nondeterministic in presentation), the cJSON win on quote/number/escape
   edge cases.

4. **Sanitizers.** ASan/LSan for Rust, ASan+UBSan for C. Run on **every
   would-be accept** — this is machine-enforced, not manual (see below).

## The tier rule (SPEC §8)

Some changes can pass every gate and still be unsafe to auto-ship. A change
that touches floating-point ordering, concurrency primitives, `unsafe`, or
carries a sanitizer flag is **never** auto-accepted — its verdict is capped
at `needs-human-review`. The tier is decided by the *nature* of the change,
never by the size of the number. The mimalloc allocator swap
([case study](./case-studies/build-flags.md)) is the canonical example: a
real +4.6% win, correctly held for a human ruling because a global
allocator change's safety is not something one benchmark can clear.

## When a gate caught the pipeline itself

The sanitizer gate exists in its current, machine-enforced form because it
once *wasn't*. The pipeline auto-accepted a comrak patch that skipped AST
teardown for a +10.7% win — and the audit's manual LeakSanitizer run found
it leaks the arena at exit. It was overturned before shipping and the fix
made `verdict` build with ASan/UBSan and run the pinned workload on every
accept, capping any flagged win at `needs-human-review`. The full story is
[the caught false-accept](./case-studies/comrak-false-accept.md).
