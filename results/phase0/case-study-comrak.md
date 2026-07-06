# Phase 0 case study — comrak (CommonMark/GFM renderer)

**Result: +4.6% median wall-time speedup (95% CI [+3.3%, +5.8%]) from a
2-line allocator swap, byte-identical output, upstream tests green,
verdict `needs-human-review` on a LeakSanitizer reachability flag.**

## Target

- comrak v0.53.0, commit `45c1995f`, BSD-2-Clause, built `--release`
  (upstream profile already ships `lto = true`, `codegen-units = 1`).
- Workload: the target's own benchmark (benches/bench.sh) — the Pro Git
  book, all translations, 11.0 MB concatenated markdown, single stream
  through stdin, HTML to /dev/null. Corpus pinned:
  `corpora/comrak/MANIFEST.sha256` (progit @ `61833a52`).

## Environment

Remote 4-core container, Intel Xeon @ 2.10GHz, kernel 6.18.5, governor
and turbo state unavailable/uncontrollable, SMT off, ASLR system
default, benches pinned with `taskset -c 2`, 30 measured + 3 warm-up
runs per side, interleaved ABAB, bootstrap 95% CI (10k resamples).

**A/A calibration, same session, pinned** (results/calibration/):
median 1.0006, 95% CI [0.9932, 1.0080] — null, as required. Unpinned
A/A for contrast: CI [0.9376, 1.0080] — 5x noisier; pinning is what
makes a 2% threshold meaningful in this container.

## Loop

1. **Profile** (callgrind; perf unavailable in container — Ir counts,
   not cycles): allocator symbols total **26.7% of instructions**
   (`_int_malloc` 9.99%, `_int_free` 6.85%, `malloc` 3.92%,
   `malloc_consolidate` 3.48%, `free` 2.49%). Parser frames
   (`parse_inline` 6.7%, `process_line` 6.5%) come *after* the
   allocator. Full ranking: results/comrak/hotspots.txt.
2. **Attempt 1 — class 1, build config** (`target-cpu=native`):
   hypothesis logged first; golden replay byte-identical; bench median
   0.9965, 95% CI [0.9826, 1.0110] → **rejected-bench** (null). Ledger
   `phase0-comrak-001`.
3. **Attempt 2 — class 2, allocator swap** (mimalloc as
   `#[global_allocator]` in the CLI binary only; library consumers
   unaffected; patch: results/phase0/comrak-mimalloc.patch):
   - Golden replay: stdout sha256 identical to pinned golden.
   - Upstream test suite on the patched tree: green (see ledger row).
   - Bench, interleaved A/B vs pristine-built baseline: **median
     speedup 1.0459, 95% CI [1.0333, 1.0577]** — CI lower bound 3.3%
     clears the 2% accept threshold.
   - Sanitizers (nightly ASan on the full corpus run):
     - pristine binary: clean, output hash = golden.
     - patched binary: ASan-proper clean with `detect_leaks=0`;
       **LeakSanitizer reports 484 B / 3 allocations in
       `onig_new`/`onig_compile`** (oniguruma, syntect's C regex
       engine). Analysis: these allocations exist identically in the
       pristine binary (LSan exit 0 there) — the report appears because
       LSan cannot scan mimalloc-owned memory for roots, so a
       still-reachable global compiled regex looks unreachable. Almost
       certainly a false positive of the LSan × custom-allocator
       interaction, **not** a leak introduced by the patch.
   - Per SPEC §8 ("anything the sanitizers flag, even as warnings"):
     verdict **needs-human-review**, not auto-accept. Ledger
     `phase0-comrak-002`.

## ROI (illustrative, config/pricing.toml rates)

For a hypothetical fleet spending 100 cores 24/7 on this workload shape,
at $0.04/core-hr (public-cloud example rate — replace per engagement):

- cores returned: median **4.4**, 95% CI [3.2, 5.5]
  (`fleet × (1 − 1/speedup)`)
- dollars/yr: median **$1,540**, 95% CI [$1,130, $1,910]

Caveats printed with the number, per policy: single workload shape
(large-document batch rendering); Ir-based profile, not cycles;
container hardware without governor control; speedup CI is wall-time on
pinned single-core execution; ISA = generic x86-64 (no target-cpu
narrowing — attempt 1 showed it buys nothing here anyway).

## Reproduction

```
just pin-check comrak                  # corpus manifest verify
cargo run -p bench-runner -- --config config/accept.toml compare \
  --baseline  "taskset -c 2 sh -c '<pristine comrak>  < corpora/comrak/progit.md > /dev/null'" \
  --candidate "taskset -c 2 sh -c '<patched comrak>   < corpora/comrak/progit.md > /dev/null'"
```

Patch: `results/phase0/comrak-mimalloc.patch` applied at `45c1995f`.
