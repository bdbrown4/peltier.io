# ROI: from stopwatch to dollars

The report is the pitch artifact, and it answers exactly one question:
*what did the stopwatch say, and what does that buy?* It is generated
**mechanically** — `just report <run-id>` reads a ledger row and the
calibrated bench and renders the numbers; nothing is hand-typed.

## The three conversions

- **Throughput → cores → dollars.** A speedup `s` on a CPU-bound fleet of
  `N` cores returns `N × (1 − 1/s)` cores; at a `$/core-hour` rate that is
  an annual saving. Both flow through the speedup's confidence interval, so
  the report quotes a *range*.
- **Latency → percentile deltas → value.** Under replayed load, the p50/p99
  latency speedups with their CIs; a p50 service-time reduction maps to
  throughput capacity for a CPU-bound service.
- **Batch windows → wall-clock hours** returned to a schedule.

## The rule that makes it survive review

Every figure carries its CI and its workload, and the **methodology ships
inside the report** — the interleaved A/B, the pristine baseline, the
bootstrap CI, the gates, the calibration. And the number the report tells
you to quote is the **CI lower bound**, never the median:

> On a 500-core fleet at $0.04/core-hour, the cJSON service win (+6.2% p50
> latency) returns **27.5 cores / $9,621 per year at the CI lower bound**
> (median 29.4 / $10,290). The point estimate is not a promise.

## What the report refuses to do

It will not mint a clean dollar figure for a change that wasn't shipped. A
row whose verdict isn't `accepted`, **or** an `accepted` row that isn't
sanitizer-clean (a pre-gate accept the audit overturned), gets a bold **Not
shipped** banner before any number. That guard was itself added by the
[adversarial review](https://github.com/bdbrown4/peltier.io/blob/main/results/adversarial-review.md),
which caught the generator pitching the overturned comrak patch. The pitch
artifact is held to the same standard as everything else: it cannot claim
what wasn't proven.
