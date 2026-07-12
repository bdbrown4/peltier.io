# ROI report — `phase4-cjson-service`

**Target:** cjson · **Playbook class:** 5 · **Verdict:** `accepted`

**Workload:** corpora/cjson/input/service.json, 1 parse+print/req, 150 rps open-loop, CO-correct

## Throughput → cores → dollars

Measured speedup (baseline/candidate): **1.0624**, 95% bootstrap CI **[1.0581, 1.0725]**.

On a **500-core** fleet running this workload:

| metric | median | 95% CI |
|---|---|---|
| cores returned | 29.4 | [27.5, 33.8] |
| annualized saving | $10290 | [$9621, $11838] |

Pricing: **$0.04/core-hour**, **8760 h/year**. Source: _example: AWS m7i on-demand us-east-1, per-vCPU approximation — replace per engagement_

The **CI lower bound is the number to quote** — 27.5 cores / $9621 per year is
the saving that survives the 95% interval; the point estimate is not a promise.

## Latency under replayed load

Workload: corpora/cjson/input/service.json, 1 parse+print/req, 150 rps open-loop, CO-correct. Coordinated-omission-correct open-loop replay.

| percentile | baseline | candidate | speedup | 95% CI |
|---|---|---|---|---|
| p50 | 3.332 ms | 3.136 ms | 1.0624 | [1.0581, 1.0725] |
| p99 | 24.726 ms | 23.947 ms | 1.0325 | [0.0715, 4.9714] |

## Methodology (ships with the number)

- **Interleaved A/B**, baseline rebuilt from a pristine checkout, never the agent's
workspace. Candidate and baseline alternate to control thermal/background drift.
- **Bootstrap 95% CI** of the ratio-of-medians; an effect is accepted only if the CI
lower bound clears the threshold in `config/accept.toml` (2%).
- **Equivalence gates** passed before any number was trusted: upstream test suite,
byte-identical golden replay, differential fuzzing, and ASan/UBSan sanitizers.
- **Calibrated hardware**: the workload passed A/A (false-positive <5%) and injected-
regression (≥95% detection) self-tests, recorded in `results/calibration/`.
- **Coordinated-omission correct** latency (service mode): each request's latency is
measured from its intended send time, so queueing is counted, not hidden.

## Caveats

Every figure is specific to the workload and hardware named above. Throughput→cores
assumes the fleet is CPU-bound on this path and scales linearly; latency figures are
for the stated arrival rate. The saving to commit to is the **CI lower bound**, not
the median.
