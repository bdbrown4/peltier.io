# Phase 4 — services & scale

**Exit criteria (SPEC §5): one real service workload optimized under
replayed traffic; report generated mechanically.** Both met.

Phase 4 takes the trust story off the batch stopwatch and onto a running
service: the same verified optimization, measured as a latency delta
under replayed load, then turned into a dollar figure by a generator
that reads only the ledger and the calibrated bench — no hand-typed
numbers.

## What was built

- **Service-mode latency bench** (`crates/bench-runner/src/service.rs`,
  SPEC §3.1 mode c): a **coordinated-omission-correct**, open-loop,
  fixed-rate load generator. Each request's latency is measured from its
  *intended* send time, so when the single worker falls behind, the
  requests queued behind it carry the full delay a real client would see
  — the exact error closed-loop tools hide. Interleaved A/B of two
  server binaries, exact percentiles (bounded sample; no HdrHistogram
  bucketing error), bootstrap p50/p99 CIs through the same ratio-CI
  machine the wall-clock bench uses.
- **A real service target**: `targets/cjson/service.c`, a minimal HTTP
  server wrapping the (patched) `cJSON.c` — trust-layer code outside the
  workspace allowlist, so a patch changes the parser under load but not
  the measurement rig.
- **Service-mode calibration** (`bench-runner service-calibrate`): the
  latency measurement is a trusted anchor only after it passes A/A
  (0/10 false positives) and injected-5%-latency-regression detection
  (10/10), recorded in `results/calibration/cjson-service-aa.json`.
- **Mechanical ROI report** (`crates/report` binary, SPEC §9): reads a
  ledger row + the service-latency JSON and renders
  throughput→cores→dollars *and* latency percentiles, every figure with
  its 95% CI, the pricing source, and the methodology printed inline.

## The result (`phase4-cjson-service`, ledger)

The accepted batch win `phase3-cjson-002` (+8.85% throughput) was
measured as a **service under a 150 rps coordinated-omission-correct
replay**, baseline (pristine cJSON v1.7.19) vs candidate (the banked
win), 20 interleaved rounds, server pinned, 40,000 requests, **0 drops**:

| metric | baseline | candidate | speedup | 95% CI |
|---|---|---|---|---|
| **p50 latency** | 3.332 ms | 3.136 ms | **1.062** | **[1.058, 1.072]** |
| p99 latency | 24.7 ms | 23.9 ms | 1.033 | [0.07, 4.97] |

**The p50 win is the claim: +6.2% service-latency reduction under load,
CI [+5.8%, +7.2%]** — the batch parse speedup translating to lower
per-request latency. It is a touch smaller than the +8.85% batch number
because under load the latency also carries fixed connection and HTTP
overhead the parse optimization doesn't touch — an honest dilution, not
a measurement error.

**The p99 was *not* claimed.** Its CI is [0.07, 4.97] — uselessly wide,
because tail latency on a single-worker loopback service is dominated by
rare OS-scheduling spikes that swamp the ~0.2 ms parse difference. The
harness resolved this correctly: `p99 verdict: rejected-bench`. A number
the measurement can't pin down does not get reported as a win, even
though the median points the right way. That refusal is the whole
project in one line.

## The ROI, generated mechanically

`just report phase4-cjson-service --service-json results/phase4/cjson-service.json`
→ [`cjson-service-roi.md`](cjson-service-roi.md). On a 500-core fleet at
$0.04/core-hour, the +6.2% p50 win returns **27.5 cores / $9,621 per
year at the CI lower bound** (median 29.4 cores / $10,290) — and the
report quotes the lower bound, because the point estimate is not a
promise. Every figure carries its CI, its workload, and the methodology
that produced it, so the number survives hostile review.

## Exit criteria (SPEC §5) — met

- **One real service workload optimized under replayed traffic:** the
  cJSON HTTP service under CO-correct open-loop replay; the verified win
  shows a CI-significant +6.2% p50 latency improvement.
- **Report generated mechanically:** the ROI report is produced from the
  ledger row and the calibrated service JSON with no hand-editing;
  throughput→cores→dollars and latency percentiles, CIs and methodology
  inline.
