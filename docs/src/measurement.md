# Measurement discipline

The product is a trustworthy number, so the measurement is where most of
the engineering went. The reference is Mytkowicz et al., *Producing Wrong
Data Without Doing Anything Obviously Wrong* — the default state of a
benchmark is "subtly lying."

## Interleaved A/B, never sequential

Baseline and candidate run **interleaved** (ABABAB…), never in sequential
blocks. Thermal drift, a background cron job, a noisy neighbor — anything
that changes over the run hits both sides equally instead of biasing one.
Warm-up runs are discarded.

## The baseline is always pristine

The baseline binary is **rebuilt from a clean checkout** each session, with
the candidate's patch stashed. A patch can never silently become part of
its own baseline (a real bug that bit an early smoke test, now closed by
`verdict --rebuild-baseline`).

## Bootstrap CIs, and the lower-bound rule

Each side yields ≥30 measured samples. The statistic is the **bootstrap
95% CI of the ratio-of-medians** (deterministic xorshift seed, so a result
reproduces). A change is accepted only if the **CI lower bound** clears the
threshold in `config/accept.toml` (default 2%) — not the median. The number
you commit to is the one that survives the interval.

## The hardware has to earn trust first

Before any measurement session, the harness runs two self-tests, recorded
in `results/calibration/`:

- **A/A**: run the *same* binary as both sides. It must produce a null
  verdict. A "speedup" from identical binaries is a calibration failure.
- **Regression injection**: a synthetic 5% slowdown (a real busy-wait in
  the timed window) must be detected ≥95% of the time.

CPU pinning (`taskset -c 2`) alone tightened the A/A confidence interval
~5× on the dev container. Every calibration anchor — local and on GitHub's
shared runners — passed both bars before its numbers were trusted.

## Service mode: coordinated omission

Phase 4 measures latency under load, where the classic trap is
**coordinated omission**: a closed-loop client that waits for a slow
response simply sends fewer requests during the stall, so the stall never
appears in the latency histogram. Peltier's load generator is **open-loop**
— requests are scheduled at a fixed rate and each latency is measured from
its *intended* send time. When the server falls behind, the requests queued
behind it each carry the full delay a real client would have seen. Nothing
is omitted.

The service measurement is calibrated the same way: 0/10 A/A false
positives, 10/10 injected-5%-latency-regression detections
(`results/calibration/cjson-service-aa.json`).

> **Percentiles are claimed only when the measurement can pin them down.**
> The cJSON service's p50 latency win was CI-tight and accepted (+6.2%,
> [+5.8%, +7.2%]); its p99 CI was [0.07, 4.97] — uselessly wide on a
> single-worker loopback tail — so it was **rejected**, even though the
> median pointed the right way. A number the measurement can't pin down
> does not ship.
