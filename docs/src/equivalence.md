# Equivalence gates

A faster binary that behaves differently is not an optimization — it is a
regression with good marketing. Before any speedup is even measured, the
candidate must prove it does the same thing the baseline does. These are
**hard gates**: a failure is `rejected-gate` and the bench never runs.

## The layers

0. **Pin checks.** Before any gate runs, the corpus is verified against its
   committed `MANIFEST.sha256` and the harness **refuses to run** on a
   mismatch — nothing is measured on tampered inputs. This ships and is
   enforced for every target.

   The *test suite* has the same machinery but is **not yet active**:
   `diff-test` verifies `corpora/<t>/TESTSUITE.sha256` when the file is
   present (mismatch = hard refusal) and prints an explicit "suite unpinned"
   warning when it is absent. **No target ships a suite pin today**, so this
   check currently constrains nothing. It matters because upstream suites
   are vendored under `targets/<t>/workspace/` — an agent-writable path — so
   "the agent cannot edit the tests" rests on a pin that still has to be
   generated (`scripts/pin-testsuite.sh`, a deliberate human action, run
   after a fetch). Until then, that specific guarantee is a mechanism, not
   a fact.

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
   inputs, comparing output.

   *Where it runs.* Fuzz is the one gate that is **not** a property of the
   candidate alone: it differs the candidate against a **pristine baseline**,
   so it needs both binaries. That means it runs **only on the accept path**
   (`just verdict`, the one flow that rebuilds a pristine baseline). The
   standalone `just gates` flow has no baseline, so the gate reports
   **Skipped, with that reason recorded** — it is never faked by comparing
   the candidate against itself. `just gates` is a fast equivalence check,
   **not** a fuzz run. The gate is graded strictly: `diff-test` runs the
   target's declared `[gates].fuzz` command and parses a
   `FUZZ-RESULT iters=<n> divergences=<m>` line; a run that never prints one
   **fails**, rather than passing by silence.

   *The hard rule.* A machine `accepted` verdict is now **impossible**
   without this gate having actually passed. A skipped or failed fuzz gate
   caps the verdict at `needs-human-review` — because an accept minted
   without differential fuzz is an unverified claim.

   *Honest history.* Through Phase 5 this ran as an **out-of-band
   per-attempt script** (`scripts/diff-fuzz-<target>.py`), not inside the
   pipeline — so **8 of the 10 accepted ledger rows record `fuzz_iters=0`**
   even where a 10,000-input audit run is documented in the case study. The
   two exceptions are the Phase 0 tokei rows, which record 4,332. Those
   out-of-band runs produced real findings — the tokei wins passed only
   after canonicalizing benign parallel-walker output ordering (differential
   gates must compare *semantics*, not bytes, where the target is
   legitimately nondeterministic in presentation), and the cJSON win cleared
   quote/number/escape edge cases with 0 divergences.

4. **Sanitizers.** ASan+UBSan (ASan/LSan on the Rust targets) is built and
   run against the pinned workload on **every would-be accept** — the
   sanitizer gate is machine-enforced, not manual (see below). A flagged
   run, *or* a target that declares no sanitizer build at all, caps the
   accept at `needs-human-review`.

   A **TSan lane** exists on the same accept path, but it only runs when a
   target declares `[build].tsan`. **No target declares it today, so there
   is currently no TSan coverage** — the lane is wired and dormant, and a
   patch touching threads needs that opt-in added before it can be checked.
   (MIRI, named in the spec, was never implemented.) The concurrency safety
   net that *is* live is the review tier below, which routes any
   concurrency-token patch to a human.

## The tier rule (SPEC §8)

Some changes can pass every gate and still be unsafe to auto-ship. A change
that touches floating-point ordering, concurrency primitives, `unsafe`, or
carries a sanitizer flag is **never** auto-accepted — its verdict is capped
at `needs-human-review`. The tier is decided by the *nature* of the change,
never by the size of the number. The routing is machine-enforced: `verdict`
runs a conservative **lexical risk classifier** over the patch's changed
lines (concurrency / unsafe / floating-point token lists — deliberately
over-triggering, not a semantic analysis) and any signal, or the use of
fp-tolerance equivalence mode at all, forces a would-be accept to review. The mimalloc allocator swap
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
