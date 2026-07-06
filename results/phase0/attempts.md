# Phase 0 dry-run attempt log

Hypotheses are recorded here *before* any patch is applied; final
evidence and verdicts go to the SQLite ledger and are mirrored here.

Environment for the whole session: remote 4-core container
(Intel Xeon @ 2.10GHz, kernel 6.18.5), governor/turbo not controllable,
benches pinned with `taskset -c 2`. A/A calibration on both target
workloads passed with pinning (see results/calibration/).

## Deviations from SPEC (Phase 0 manual dry run)

- No nested no-network container available (no docker daemon): target
  code runs in this session's own isolated ephemeral container, which
  serves as the sandbox. Phase 1 must restore the dedicated container.
- `perf` unavailable (container): profiling via valgrind/callgrind
  (deterministic instruction counts, Ir), not cycle sampling. Ir% is a
  proxy for cycles%; it under-weights cache/branch effects.
- Corpus files >10 MB are pinned by committed MANIFEST.sha256 +
  regeneration scripts rather than committed raw (repo bloat).

## Attempt 1 — comrak, class 1 (build configuration)

- **Logged**: 2026-07-06, before patching.
- **Hotspot**: whole-program; upstream already ships lto=true +
  codegen-units=1, so classic class-1 flags are exhausted upstream.
- **Hypothesis**: `RUSTFLAGS=-C target-cpu=native` (baseline ISA: the
  bench machine's Xeon) enables wider SIMD paths in string scanning
  (jetscii, memchr paths visible in profile) and better scheduling;
  expect a small win, 2-6% median speedup. No FP semantics change
  (integer/string workload; not a -ffast-math-class flag), so
  auto-acceptable if gates pass. Deploy caveat: win only valid for
  fleets whose ISA baseline matches; recorded in workload statement.
- **Patch**: none (env-level build config); candidate built to a
  separate CARGO_TARGET_DIR from the same pinned commit.

## Attempt 2 — comrak, class 2 (allocator swap)

- **Logged**: 2026-07-06, before patching, after Attempt 1's hypothesis.
- **Hotspot**: allocator symbols sum to ~26.7% of Ir (_int_malloc 9.99%,
  _int_free 6.85%, malloc 3.92%, malloc_consolidate 3.48%, free 2.49%)
  — the class-2 precondition (≥5%) is exceeded 5x.
- **Hypothesis**: glibc malloc is the allocator; comrak's AST/inline
  parsing churns small allocations. Swapping the CLI's global allocator
  to mimalloc cuts allocator instructions substantially; expect 8-20%
  median speedup on the progit workload.
- **Patch plan**: 2-line change in comrak CLI main + Cargo.toml dep,
  inside targets/comrak/workspace only.

## Attempt 3 — tokei, class 1 (build configuration)

- **Logged**: 2026-07-06, before patching.
- **Hotspot**: whole-program; upstream ships lto="thin", panic="abort",
  default codegen-units (16).
- **Hypothesis**: lto=true (fat) + codegen-units=1 lets the analysis
  loop (perform_multi_line_analysis, 31.7% Ir) inline across cgu and
  crate boundaries (memchr/aho-corasick calls visible in profile);
  expect 2-8% median speedup. No semantics change; auto-acceptable if
  golden replay passes.
- **Patch**: [profile.release] lto = true, codegen-units = 1 in
  targets/tokei/workspace/Cargo.toml only.

## Outcomes (ledger is authoritative: results/ledger.sqlite)

| run_id | class | speedup median | 95% CI | verdict |
|---|---|---|---|---|
| phase0-comrak-001 | 1 build-config | 0.9965 | [0.9826, 1.0110] | rejected-bench (null) |
| phase0-comrak-002 | 2 allocator | **1.0459** | **[1.0333, 1.0577]** | needs-human-review |
| phase0-tokei-001 | 1 build-config | 1.0161 | [1.0026, 1.0262] | rejected-bench (< 2% threshold) |

- comrak-002 gates: 848 upstream tests green on patched tree; golden
  replay byte-identical; pristine ASan+LSan clean; patched ASan-proper
  clean; LSan flags 484 B in onig (analysis: LSan × custom-allocator
  reachability false positive — see case-study-comrak.md). SPEC §8
  routes any sanitizer flag to human review, so that is the verdict.
- tokei-001 is a *real but small* effect — CI excludes 1.0 but its
  lower bound (+0.26%) is under the 2% bar. Correctly rejected; logged
  so nobody re-grinds it.
- Phase 0 exit criteria (≥1 verified win ≥10%) **not yet met**: best
  verified candidate is +4.6% pending human review. Next cheapest
  hypotheses per profile: comrak class 3 (allocation churn — arena the
  inline parser's Vec growth, RawVec::finish_grow 2.3% + memcpy 3.1%);
  tokei class 3 (memset 8.5% suggests per-file buffer zeroing) and
  class 5 (perform_multi_line_analysis 31.7% single frame).
