# Phase 3 — playbook + proof

**Exit criteria (SPEC §5): 3–5 public case studies · zero regressions
shipped · playbook ≥6 classes · C/C++ targets · coz.**

Phase 3 turns the trust layer and the agent loop from Phases 1–2 into
*evidence*: verified optimization wins across the playbook and across
languages, each one reproducible from a pinned corpus through the same
gated pipeline, with every attempt — win or loss — in the append-only
ledger.

## What changed for Phase 3

- **The trust layer went language-agnostic.** Build isolation moved from
  a cargo-specific `CARGO_TARGET_DIR` injection to an explicit `{out}`
  placeholder plus optional `sanitizer`/`sanitizer_binary` build
  templates (`crates/diff-test/src/target.rs`). A C/C++ target is now
  just a `target.toml` with `clang` commands. Proven zero-regression:
  tokei ran a full verdict through the refactored pipeline to a correct
  null result before any C target was added.
- **A C target joined the roster** — cJSON (v1.7.19, MIT), with a
  trust-layer harness driver, a hermetic Unity/CTest gate, ASan+UBSan
  sanitizers, a deterministic pinned corpus, and its own A/A calibration
  (0/20 false positives, 20/20 injected-slowdown detections). CI gates
  it on every push.
- **coz** is wired (`-DHOTPATH_COZ` progress point, `just coz`,
  `scripts/coz-*`); its apt runtime aborts in its own init on this
  container's glibc (documented in `results/cjson/coz/README.md`), so
  callgrind cache-sim is the working profiler.

## Playbook classes exercised (≥6 required)

| Class | Mechanism | Exercised on | Result |
|---|---|---|---|
| 1 | build config (LTO, codegen-units) | tokei | ✅ win (+10.4%) |
| 2 | allocator swap (mimalloc) | comrak | needs-human-review (+4.6%) |
| 3 | allocation churn | tokei, comrak, **cjson** | ✅ wins + one overturned |
| 4 | data layout | comrak | attempted (rejected) |
| 5 | algorithmic / redundant work | tokei ×3, **cjson** | ✅ wins |
| 6 | SIMD / autovectorization | comrak, tokei | attempted (rejected) |

Six classes exercised (1–6); class 5 and class 3 carry verified wins on
both a Rust and a C target.

## The case studies

1. [`01-tokei-algorithmic-rust.md`](01-tokei-algorithmic-rust.md) —
   three independent CI-verified class-5 wins on a Rust code counter
   (~1.26× multiplied, stated as an approximation, not a measured
   figure), each unattended and audited.
2. [`02-build-flags-and-allocators.md`](02-build-flags-and-allocators.md)
   — the cheap wins first: LTO (class 1) accepted, mimalloc (class 2)
   correctly held at needs-human-review.
3. [`03-comrak-caught-false-accept.md`](03-comrak-caught-false-accept.md)
   — the pipeline auto-accepted a leaking teardown patch; the audit
   overturned it and hardened the pipeline. The most important study.
4. [`04-cjson-c-target.md`](04-cjson-c-target.md) — the cross-language
   proof: a real sub-threshold win correctly rejected, then a byte-
   identical number-handling win accepted on the C target.

## The through-line

Every number here carries its 95% bootstrap CI and its workload. Every
win survived byte-identical golden replay, a 10k-input differential
fuzz, and sanitizers. Every rejection is a real mechanism that simply
didn't clear the 2% CI-lower-bound bar — and it is in the ledger too,
because a rejection is a complete, valid outcome. Nothing shipped that
the pipeline couldn't prove.
