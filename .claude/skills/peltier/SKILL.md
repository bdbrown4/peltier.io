---
name: peltier
description: Prove a performance change is real — or refuse to claim it. Use when optimizing code for speed, verifying a claimed speedup ("is this actually faster?", "did my optimization work?"), checking for a performance regression ("did this PR make things slower?"), or comparing the performance of two implementations. Runs peltier's trust layer: equivalence gates, A/A environment calibration, interleaved A/B with bootstrap confidence intervals. Never reports a naked percentage and never claims a win that is not statistically significant. Do NOT use for correctness-only work.
---

# peltier — prove it, or claim nothing

A performance change that has not passed equivalence gates **and** shown a
statistically significant improvement **does not exist**. You may discard it
or log it. You may not claim it.

Most benchmarking is wrong in ways that flatter the author: run-to-run noise
read as a win, a faster binary that computes something subtly different, one
lucky run out of five, a percentage with no interval attached. This skill
exists to make those failures impossible rather than unlikely.

## Preflight — never skip, never work around

Run `scripts/preflight.sh` **relative to this skill's directory** (resolve its
absolute path first — the skill may be installed at `~/.claude/skills/`, as a
plugin, or copied into another project):

    sh <skill-dir>/scripts/preflight.sh

It locates the trust layer, builds the real `bench-runner`, and checks this
host can run it. `STATUS=ok` does **not** mean the host can measure your
change — only the A/A self-test decides that.

**If it refuses, that ends the claim, not the work.** You may still write the
patch and explain why you think it is faster. You may not attach a number to
it. Do not fall back to `time`, `hyperfine`, or a stopwatch loop — a number
you cannot stand behind is worse than no number, because someone will act on
it. The statistics live in `bench-runner` and are used from there; a second
copy of the math is a second thing that can lie.

## Two modes

| Mode | Use when | Read |
|---|---|---|
| **verify** | You have a baseline and a candidate — any repo, any language. | [reference/verify.md](reference/verify.md) |
| **attempt** | You are inside a peltier checkout and want the full loop: profile → hypothesize → patch → gated verdict → ledger row. | [reference/attempt.md](reference/attempt.md) |

Default to **verify**. It is the portable one, and it is what almost every
request actually wants.

### No candidate yet? Profile first — do not guess.

"Measure before optimizing" is not advice, it is a gate. If you have one
program and a hunch, get profile data *before* writing a patch: `perf record`
(native), `py-spy` / `cProfile` (Python), `node --prof` / `--cpu-prof` (Node),
`cargo flamegraph` (Rust), or `just coz` for causal profiling inside a peltier
checkout. Then form a hypothesis about a *named* hot path, and only then patch.
Guessing where the time goes is how people spend a week on 0.3% of the runtime.

## The order of the gates is the point

Each gate can end the job, and ending early is a success, not a failure.

1. **Equivalence — before you time anything.** A faster program that does
   something different is not an optimization, it is a bug with good
   benchmarks. Prove baseline and candidate produce identical output on a fixed
   workload. Comparing two *empty* outputs is a failed gate, not a passed one.
2. **Calibration — can this machine measure this change at all?** The A/A
   self-test runs the same command against itself and must find **nothing**. A
   harness that "discovers" a speedup between a binary and itself is measuring
   noise, and every number it gives you afterwards is fiction.
3. **Measurement — interleaved A/B, bootstrap CI.** Not "run A five times, then
   B five times" — that hands any thermal or cache drift to whichever side ran
   last.
4. **Risk — what did the patch actually touch?** Concurrency primitives,
   `unsafe`/raw memory, floating-point reassociation: `needs-human-review`,
   **regardless of how good the numbers are**. In verify mode *you* are the
   classifier — `bench-runner` only ever emits `accepted` or `rejected-bench`,
   so it cannot make this call for you.
5. **Verdict — the CI lower bound decides.** Accept only if the bootstrap 95%
   CI lower bound of the speedup clears the threshold (default 2%). The median
   is not the claim. The lower bound is the claim.

## Non-negotiables

- **Cheapest wins first:** build flags → LTO → PGO → allocator swap → *then*
  code changes. Do not hand-write SIMD before you have tried `-O3`.
- **Every attempt is recorded, including failures.** A rejected hypothesis is a
  result — it stops the next person from re-running your dead end.
- **A failed gate is a complete, valid outcome.** Write it up and move on. Do
  not iterate on a rejected patch more than twice without a new hypothesis.
- **The thing being measured never grades itself.** The patch may not touch the
  harness, the corpus, or the tests.

## Never

- **Never a naked percentage.** Every number carries its confidence interval
  and the workload it was measured on. "12% faster" is not a result.
- **Never re-run a benchmark until it passes.** That is p-hacking with extra
  steps. The first calibrated run is the answer.
- **Never claim a win the CI lower bound does not support**, however good the
  median looks and however "obviously" faster the change is.
- **Never present a measurement from a host that failed calibration.**

## What good looks like

**A claim, fully dressed** (`phase3-cjson-002`, in the ledger):

> Accepted. Speedup 1.088 (candidate is 8.8% faster), 95% CI [1.074, 1.100] —
> so the *defensible* claim is ≥7.4%, not 8.8%. Workload: 5.6 MB synthetic
> JSON, 20k records, parsed + serialized 2×, single thread. Gates: upstream
> suite green, golden replay byte-identical, ASan + UBSan clean.

Note what that write-up does **not** say. It does not claim a differential-fuzz
run, because that row records `fuzz_iters=0` — the fuzzing of that era ran
out-of-band and the machine never witnessed it. Fuzz is a pipeline gate now, and
new rows carry a real count. Say what the record supports, not what you
remember doing.

**Declining to claim what the data won't support** (`phase4-cjson-service`):
p50 latency improved 6.2% [95% CI +5.8%, +7.2%] at 150 rps — claimed. The p99
CI came back [0.07, 4.97] — uselessly wide, so **p99 was not claimed at all**,
even though its median looked like a win.

**Good numbers on a bad change** (`phase2-comrak-010`): 1.107 speedup, 95% CI
[1.079, 1.121], on 11 MB of markdown. Tight interval, real improvement,
**accepted — and wrong.** LeakSanitizer caught it afterwards and the row was
overturned. It never shipped. The bench was never the problem; skipping a gate
was. This is why the order above is not negotiable.
