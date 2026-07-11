# Case study 1 — tokei: compounding algorithmic wins (Rust, class 5)

**Target:** [tokei](https://github.com/XAMPPRocky/tokei) v14.0.0 — a
source-code line counter. **Workload:** 221 MB / 4360 files (8 replicas
of the Pro Git book + comrak's own tree), single rayon thread, CPU
pinned. **Result:** three independent, CI-verified class-5 wins on the
same hot function — each proposed *by the unattended agent* and
100%-audited. Multiplied together they are **~1.26× over the Phase-0
baseline** (1.107 × 1.037 × 1.099), but that product is an
approximation, not a measured figure: each speedup carries its own
bootstrap CI against its own baseline, and confidence intervals do not
compose through multiplication. The trustworthy claims are the three
individual increments below.

## The hot path

tokei spends the overwhelming majority of its instructions in one
function — `LanguageType::parse_lines` and the per-byte loop inside
`SyntaxCounter::perform_multi_line_analysis`. The cache-sim profile put
it at **55.9% of instructions, 62% of all branches, 44% of branch
mispredicts** on the pinned corpus: for every byte of every line, the
counter tested a stack of quote/comment token matchers.

## The three wins

| run | mechanism | speedup (median) | 95% CI |
|---|---|---|---|
| `phase0-tokei-002` (class 3) | read path: `fs::read` pre-sized, BOM-only decode | 1.099 | [1.080, 1.127] |
| `phase2-tokei-001` (class 5) | 256-entry first-byte gate table for the token matchers | 1.107 | [1.084, 1.131] |
| `phase2-tokei-003` (class 5) | forward-only whitespace break (drop the `rposition` rescan) | 1.037 | [1.027, 1.052] |
| `phase2-tokei-008` (class 5) | streaming leftmost `important_syntax` scan (one vectorized pass, not per-line restart) | 1.099 | [1.073, 1.112] |

Each is a **redundant-work elimination**: the first-byte gate replaces
five per-byte matcher loops with one table load; the whitespace-break
change drops a full backward scan of the file tail per byte; the
streaming scan replaces a per-line aho-corasick restart (which kept
memmem's SIMD prefilter in its short-haystack slow path) with a single
leftmost search advanced by integer comparisons.

## Why the numbers are trustworthy

- **Byte-identical golden replay** on the pinned corpus for every win —
  a code counter that miscounts one line fails the gate.
- **10,000-input differential fuzz** (`scripts/diff-fuzz-tokei.py`),
  pristine vs candidate, **0 divergences** — after canonicalizing
  tokei's benign parallel-walker output ordering (a real finding:
  raw-byte differential comparison over-counts when the target is
  legitimately nondeterministic in presentation).
- **ASan/LSan clean** on the patched tree over the full corpus.
- **Interleaved A/B** vs a pristine-rebuilt baseline, bootstrap 95% CI
  lower bound ≥ the 2% bar. `phase2-tokei-008` was the first accept
  through the *machine-enforced* sanitizer gate (see case study 3).

## What it demonstrates

The richest wins on a mature Rust binary were not exotic — they were
"this loop does work it doesn't need to, on every byte." The agent found
each by reading the cache-sim profile, checking the ledger to avoid
re-attempting a class on the same hotspot, and proposing the cheapest
untried mechanism. Three separate hypotheses, three separate verified
increments, all on the same 40%-of-runtime function — and one honest
rejection in between (`phase2-tokei-009`, a real +5.7% median whose
noisy CI lower bound of +0.2% could not clear the bar, so it does not
count).
