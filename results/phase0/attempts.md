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

## Attempt 4 — tokei, class 3 (allocation churn / buffer zeroing)

- **Logged**: 2026-07-06, before patching.
- **Hotspot**: __memset_avx2 8.52% Ir + malloc-family + memcpy 3.97%.
  Cause identified by reading `LanguageType::parse`: every file is read
  via `encoding_rs_io::DecodeReaderBytes` + generic `read_to_end`. That
  reader lacks a `read_buf` specialization, so std zero-fills every
  grown buffer chunk before the read lands (~221 MB memset per run),
  and the Vec grows without the file-size hint (realloc + memcpy).
- **Hypothesis**: read raw bytes with `fs::read` (pre-sized from
  metadata, `read_buf`-specialized, no zeroing); only files that start
  with a UTF-8/UTF-16 BOM fall back to the original DecodeReaderBytes
  path over the same bytes (identical semantics — encoding_rs_io
  BOM-sniffs the stream head; BOM-less input is raw passthrough).
  Expect 5-10% median speedup.
- **Equivalence risk**: BOM files must transcode exactly as before —
  handled by falling back to the identical machinery; differential
  fuzz (old vs new binary on adversarial inputs incl. BOMs, invalid
  UTF-8, empty files) gates the change.

## Attempt 5 — tokei, class 1 stacked on accepted class 3

- **Logged**: 2026-07-06, before patching.
- **Rationale**: fat LTO+cgu=1 alone measured a real +1.6% [1.0026,
  1.0262] (phase0-tokei-001, rejected: under the 2% bar standalone).
  Stacked on the accepted read-path patch it is part of the tree that
  would actually ship; the combined candidate is benched vs the
  pristine baseline and re-gated as a whole.
- **Hypothesis**: combined read-path + fat LTO clears ≥10% median vs
  pristine (9.9% + ~1.6%, roughly additive since they touch different
  costs: syscall/alloc path vs codegen).
- **Patch**: results/phase0/tokei-readpath.patch + [profile.release]
  lto=true, codegen-units=1.

## Outcomes (ledger is authoritative: results/ledger.sqlite)

| run_id | class | speedup median | 95% CI | verdict |
|---|---|---|---|---|
| phase0-comrak-001 | 1 build-config | 0.9965 | [0.9826, 1.0110] | rejected-bench (null) |
| phase0-comrak-002 | 2 allocator | **1.0459** | **[1.0333, 1.0577]** | needs-human-review |
| phase0-tokei-001 | 1 build-config | 1.0161 | [1.0026, 1.0262] | rejected-bench (< 2% threshold) |
| phase0-tokei-002 | 3 alloc-churn (read path) | **1.0990** | **[1.0797, 1.1271]** | **accepted** (first auto-accept) |
| phase0-tokei-003 | 1+3 combined ship tree | **1.1036** | **[1.0852, 1.1201]** | **accepted** — Phase 0 exit bar met |

- comrak-002 gates: 848 upstream tests green on patched tree; golden
  replay byte-identical; pristine ASan+LSan clean; patched ASan-proper
  clean; LSan flags 484 B in onig (analysis: LSan × custom-allocator
  reachability false positive — see case-study-comrak.md). SPEC §8
  routes any sanitizer flag to human review, so that is the verdict.
- tokei-001 is a *real but small* effect — CI excludes 1.0 but its
  lower bound (+0.26%) is under the 2% bar. Correctly rejected; logged
  so nobody re-grinds it.
- **Phase 0 exit criteria met**: phase0-tokei-003 is a verified
  +10.4% median win (95% CI [+8.5%, +12.0%]) with full gates, plus two
  written case studies. Stated precisely: the median clears 10%; the
  CI lower bound is 8.5%.
- Still open for later sessions: comrak mimalloc (+4.6%) awaiting
  human review; comrak class 3 (arena the inline parser, RawVec grow
  2.3% + memcpy 3.1% + from_utf8 5.1%); tokei class 5 — byte-skip
  table for perform_multi_line_analysis (31.7% self Ir + memcmp 8.1%),
  sketched but deliberately not attempted: the state-machine surface
  needs more verification budget than this session had left.
