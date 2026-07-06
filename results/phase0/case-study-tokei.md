# Phase 0 case study — tokei (source-code statistics)

**Result: +10.4% median wall-time speedup (95% CI [+8.5%, +12.0%]) vs
pristine, byte-identical output, 4,332-iteration differential fuzz
clean, sanitizers clean, auto-accepted. Phase 0 exit criterion
(≥1 verified win ≥10%) met by the median; the CI lower bound is 8.5%.**

## Target & workload

- tokei v14.0.0, commit `fa44e519`, MIT OR Apache-2.0.
- Workload: 8 replicas of two pinned trees (progit @ `61833a52`,
  comrak src @ `45c1995f`), 221 MB / 4,360 files, `--sort code`,
  `RAYON_NUM_THREADS=1` (measurement first; multithreaded scaling is a
  class-7 concern). Corpus pinned: `corpora/tokei/MANIFEST.sha256`.
- Environment: as case-study-comrak.md (pinned core, interleaved ABAB,
  30+3 runs/side, bootstrap 95% CI; A/A null [0.9891, 1.0118]).

## The find (profile → hypothesis)

callgrind put **8.5% of instructions in `__memset_avx2`** with heavy
malloc/memcpy alongside — odd for a tool that just reads and scans
files. Reading `LanguageType::parse` explained it: every file is read
through `encoding_rs_io::DecodeReaderBytes` via generic `read_to_end`.
That reader has no `read_buf` specialization, so std **zero-fills every
grown buffer chunk before the read lands** (~221 MB of memset per run),
and the Vec grows without a file-size hint (realloc + memcpy churn).
The decoding wrapper only *does* anything when the file starts with a
BOM — for BOM-less input it is byte-for-byte passthrough.

## The patch (results/phase0/tokei-readpath.patch, ~20 lines)

`fs::read` (pre-sized from metadata, `read_buf`-specialized, no
zeroing) for every file; files starting with a UTF-8/UTF-16 BOM fall
back to the *original* `DecodeReaderBytes` machinery over the same
bytes. Equivalence argument: encoding_rs_io BOM-sniffs the stream head;
identical bytes in → identical bytes out on both paths. The subtle case
that forbids skipping the fallback: a UTF-8-BOM file is actually
transcoded (invalid sequences → U+FFFD), unlike BOM-less input.

## Gates

- Golden replay: stdout sha256 identical to pinned golden.
- **Differential fuzz, old binary vs new binary**: crafted grid (7 BOM
  prefixes × 12 bodies × 2 extensions — including invalid UTF-8,
  UTF-16LE/BE, truncated BOMs, binary junk, 70 KB single lines,
  unterminated strings/comments) plus randomized cases; **4,332
  iterations / >90 s, zero divergence** in either default or JSON
  output.
- Upstream test suite: 250 tests green on the patched tree.
- ASan + LSan (nightly) over the full 221 MB corpus: clean; the
  ASan-instrumented binary's output hashes to golden.

## Bench (ledger rows phase0-tokei-002 / -003)

| candidate | median speedup | 95% CI | verdict |
|---|---|---|---|
| read-path patch alone | 1.0990 | [1.0797, 1.1271] | accepted |
| + fat LTO, cgu=1 (ship tree) | **1.1036** | **[1.0852, 1.1201]** | accepted |

Fat LTO alone had measured a real but sub-threshold +1.6% [1.0026,
1.0262] (phase0-tokei-001, rejected standalone). The ship tree stacks
it on the accepted patch and is benched as a whole against the pristine
baseline — the bench measures exactly what ships.

## ROI (illustrative, config/pricing.toml rates)

Hypothetical 100-core 24/7 fleet on this workload shape at $0.04/core-hr:
cores returned median **9.4**, 95% CI [7.9, 10.7]; dollars/yr median
**$3,290**, 95% CI [$2,750, $3,760]. Caveats: single workload shape
(cold-ish batch scan of a large mixed tree, warm page cache, single
thread); Ir-profile prioritization; container hardware, no governor
control; results include the codegen (LTO) layer which is
toolchain-version-sensitive (rustc 1.94.1).

## Upstreamability

The read-path change is a candidate for an upstream tokei PR
(BOM-gated fast path, no behavior change, 250 tests + fuzz evidence).
Not filed from this session; listed as follow-up.
