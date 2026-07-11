# Case study 4 — cJSON: the cross-language proof (C, classes 3 + 5)

**Target:** [cJSON](https://github.com/DaveGamble/cJSON) v1.7.19 — an
ultralightweight C JSON parser/printer, MIT. **Workload:** a 5.6 MB
deterministic JSON corpus (20k records: nested objects/arrays,
escaped/unicode strings, integers, dense floats), parsed and serialized,
single-threaded, CPU-pinned. **Result:** the first verified win on a
C/C++ target — proof the trust layer, the gates, and the ROI story
carry across languages, not just across Rust crates.

## Getting C through the same pipeline

Nothing in the trust layer assumes cargo anymore. cJSON is "just a
`target.toml`" pointing at `clang` build commands, with:

- a **hermetic Unity/CTest suite** (19 tests: `parse_number`,
  `parse_string`, `print_*`, `misc_tests`, …) as the upstream-test gate;
- a **trust-layer harness driver** (`targets/cjson/harness.c`) compiled
  *with* the patched `cJSON.c` — the agent can patch the parser but not
  the measurement rig (it lives outside the workspace allowlist);
- **ASan + UBSan** as the sanitizer gate (the C analogue of the Rust
  nightly ASan path);
- its own **A/A calibration**: 0/20 false positives, 20/20
  injected-slowdown detections — the C workload is a trusted anchor
  before any verdict runs.

## The profile

callgrind cache-sim put **~22% of instructions in glibc float I/O** —
`__printf_fp` (number serialization) and `strtod`/`str_to_mpn` (number
parsing) — plus `parse_value`, `parse_string`, and heavy malloc/free
churn. A number-dense JSON document spends most of its time turning
numbers into strings and back.

## Attempt 1 — the honest rejection (`phase3-cjson-001`, class 3)

`parse_number` heap-allocated a temporary NUL-terminated buffer
(`malloc` + `memcpy` + `free`) for **every** number, just to hand a
clean string to `strtod`. With ~100k+ numbers in the corpus that is a
large share of the allocation churn. The fix: a 64-byte **stack buffer**
for the common short-number case, heap only for pathologically long
number strings — byte-identical (strtod sees the same bytes).

- **Speedup: 1.012 median, 95% CI [1.004, 1.022]** → **rejected-bench.**

A real mechanism, a real +1.2%, but the CI lower bound (+0.4%) does not
clear the 2% bar — because the corpus's wall-clock is dominated by float
*formatting*, not by malloc. The statistical bar rejected a genuine win,
exactly as designed. It is in the ledger.

## Attempt 2 — the accepted win (`phase3-cjson-002`, classes 5 + 3)

Adding the class-5 lever: `print_number` verified every fractional
number's `"%1.15g"` round-trip by calling **`sscanf("%lg")`** — a full
`strtod` *plus* format-string interpretation, per number. Replacing it
with a direct `strtod` is the identical parse with none of the
format-string overhead. Combined with the stack buffer:

- **Speedup: 1.089 median, 95% CI [1.075, 1.100]** → **accepted.**
- Upstream Unity suite: **PASS**. Golden replay: **byte-identical**.
- **ASan + UBSan: clean** — through the machine-enforced sanitizer gate.
- **10,000-input differential fuzz** (`scripts/diff-fuzz-cjson.py`),
  pristine vs candidate, **0 divergences**.

A **+8.9%** verified win on a C target, banked.

## A finding worth keeping

The first cJSON accept nearly wasn't recorded: the ASan+UBSan build
failed to link because the clang sanitizer runtime (`libclang_rt.asan`)
was not installed, and `sanitizer_check` treated that as a hard error —
crashing the verdict and losing the measured bench. The fix hardened the
gate: an **infrastructure** build failure (missing runtime, broken
toolchain) now caps the accept at `needs-human-review` and logs the
failure for the operator, rather than discarding a real result. "Cannot
verify" is a review tier, not a crash.

## What it demonstrates

The progression — a real win *rejected* for being too small, then a
larger win *accepted* only after golden replay, fuzzing, and sanitizers
all held — is the whole method in miniature, now shown on C. Nothing
about the trust story is Rust-specific. The cheapest safe lever (skip a
per-number allocation) wasn't enough; the algorithmic lever (stop doing a
redundant heavyweight parse) was — and the pipeline proved it the same
way it proves everything else.
