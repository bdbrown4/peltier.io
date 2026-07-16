# verify — prove a speedup in any repo

You have two things that should do the same work, one of which you think is
faster. Find out. Works on any pair of shell commands, in any language.

## 0. Preflight

    sh <skill-dir>/scripts/preflight.sh
    # `sh` launcher missing ("sh: not found" — a launch failure, NOT a refusal)?
    # same contract, byte-diffed against the sh version in CI:
    powershell -NoProfile -ExecutionPolicy Bypass -File <skill-dir>/scripts/preflight.ps1
    # installation carries no scripts/ (script-blocking harness)? run from the
    # checkout — either entry point, same dispatch rule:
    sh "$PELTIER_HOME/.claude/skills/peltier/scripts/preflight.sh"
    powershell -NoProfile -ExecutionPolicy Bypass -File "$env:PELTIER_HOME\.claude\skills\peltier\scripts\preflight.ps1"

A `STATUS=refuse` from either entry point is final — do not run the other one
looking for a different answer. If no entry point can launch at all, that too
is a refusal: report it rather than improvising a substitute.

Read the `key=value` output; you need `BENCH_RUNNER` and `PIN_SUPPORTED`. A
refusal ends the **claim**, not the work — report the reason verbatim and do
not substitute another timing tool.

    BR=<BENCH_RUNNER from preflight>

## 1. Config

    cp <skill-dir>/assets/accept.template.toml ./peltier-accept.toml
    CFG=./peltier-accept.toml

Do **not** loosen `threshold`, `runs_per_side`, or `confidence` to make a
result pass. That is not measurement, it is negotiation.

**Pinning is not done through this file.** `pin_prefix` is read only by the
in-repo `verdict` pipeline — `compare`, `aa`, and `calibrate` ignore it
entirely (bench-runner's own fingerprint says `"caller-provided (wrap cmd in
taskset)"`). If `PIN_SUPPORTED=yes` and you know a core is free, pin by
wrapping **both sides identically**:

    --baseline 'taskset -c 2 <cmd>' --candidate 'taskset -c 2 <cmd>'

An unpinned host is not disqualified. A *noisy* one is, and step 3 is what
decides that.

## 2. Two immutable artifacts — not two states of one tree

The harness runs the two sides **alternately, 66 times**. That is impossible if
"baseline" and "candidate" are the same working tree in different states.
Before measuring:

- Build each side into its **own** location — a `git worktree` per side, or two
  separate build/output directories. Never `git stash` between runs.
- Pass the **built artifact paths** as the commands. Never pass a wrapper that
  can rebuild: `cargo run`, `npm start`, `go run`, `mvn exec` will happily
  rebuild from whatever is in the tree *mid-benchmark*, so you can end up
  timing the same binary twice — and paying build cost inside the timed window.
- Confirm the two artifacts actually differ: `sha256sum <baseline> <candidate>`.

Peltier learned this the hard way (`phase1-verdict-smoke-001`: "workspace paths
get silently rebuilt from the patched tree by test runs"), which is why the
in-repo pipeline rebuilds a pristine baseline from the pinned upstream commit
rather than trusting the tree.

## 3. Equivalence — before you time anything

Capture **everything** each side produces, on a fixed, representative workload:

    run_side() {  # $1 = tag, $2 = command
        rm -rf "out.$1"; mkdir -p "out.$1"
        ( eval "$2" ) > "out.$1/stdout" 2> "out.$1/stderr"; echo "$?" > "out.$1/exit"
    }
    run_side base "<baseline-cmd>"
    run_side cand "<candidate-cmd>"
    diff -r out.base out.cand && echo "GOLDEN PASS" || echo "DIVERGENT"

If the command's real output is a **file tree** (a bundler, a compiler, an
image pipeline), hash that too — stdout is not the artifact:

    ( cd dist && find . -type f | sort | xargs sha256sum ) > out.<tag>/tree

**An empty diff over empty output is a failed gate, not a passed one.** If both
sides wrote nothing to stdout, nothing to stderr, and you captured no artifact
tree, you have compared nothing — stop and find the real output before going
further. Then run the project's own test suite against the candidate; it must
pass.

- **Identical output + tests green** → proceed.
- **Divergent** → stop. No speedup may be claimed from a change you cannot show
  is behavior-preserving. The one exception — a change that *legitimately*
  alters output, floating-point reassociation being the common case — is
  `needs-human-review`, never an auto-accept. Quantify the divergence (how many
  values, by how much) and hand it to a human.

## 4. Calibration — can this host measure this change?

The A/A self-test runs the **same command on both sides**. It must find nothing:

    $BR --config "$CFG" aa --cmd '<baseline-cmd>'

- stdout `A/A self-test passed (null verdict)` → the harness is not inventing
  speedups. Proceed.
- **Nonzero exit**, with `Error: A/A self-test FAILED: harness claims a speedup
  from identical binaries` on **stderr** → stop. This host is too noisy to
  measure this change; every number it produces afterwards is fiction. Report
  the noise floor, then quiet the machine (pin a core, disable turbo/SMT, close
  everything else) or move to calibrated hardware.

For a claim anyone will act on, run the full calibration — a false-positive
rate over many A/A sessions *plus* proof the harness can detect a **known
injected regression**:

    $BR --config "$CFG" calibrate --cmd '<baseline-cmd>' \
        --sessions 20 --slowdown 0.05 --out calibration.json

Required: **<5% A/A false-positive rate** and **≥95% detection** of the injected
5% slowdown. A harness that cannot see a real 5% regression cannot be trusted
to confirm a real 5% win. Keep `calibration.json` — it is the evidence that the
measurement itself was sound.

### Budget the runtime before you start

`compare` executes the workload **(30 + 3) × 2 = 66 times**. `calibrate
--sessions 20` executes it roughly **2,650 times**. Size the workload to
**~0.2–2 s** of wall time: shorter and you measure process startup, longer and
calibration takes hours. Run `calibrate` in the background — it will blow
through a foreground command timeout.

## 5. Measure — interleaved A/B, bootstrap CI

    $BR --config "$CFG" compare --baseline '<baseline-cmd>' --candidate '<candidate-cmd>'

Runs are **interleaved** (ABAB…), not blocked, so thermal drift and cache state
hit both sides equally. Output:

    speedup (baseline/candidate): median 1.0884, 95% CI [1.0745, 1.1001]
    verdict: accepted

The ratio is `baseline/candidate`, so **>1 means the candidate is faster**.

`--perf-stat` (Linux) adds cycles/instructions/cache-miss medians — useful for
explaining *why* something got faster, **never** part of the accept decision,
and usually unavailable in VMs.

## 6. Risk — you are the classifier here

`bench-runner` emits only `accepted` or `rejected-bench`. It **cannot** route a
dangerous change to review, because in verify mode nothing is inspecting your
diff. So inspect it yourself, and override the tool:

Read the patch. If it touches any of these, the outcome is **`needs-human-review`
regardless of what `compare` printed**:

- **Concurrency** — threads, locks, atomics, memory ordering, `volatile`, work
  queues, async pools, `rayon`/`pthread`/`std::sync`.
- **Unsafe / raw memory** — `unsafe`, pointer casts, `transmute`, FFI,
  manual allocation, uninitialized memory.
- **Floating-point reassociation** — reordering accumulation, `-ffast-math`,
  FP SIMD, changed summation order.

These changes can be *correct on your workload and wrong in production*, and a
green bench says nothing about that. A byte-identical output does not clear
them: a data race produces identical output right up until it doesn't.

## 7. Verdict — read it honestly

| What you see | What it means | What you may say |
|---|---|---|
| `verdict: accepted` | CI lower bound clears the threshold. | Claim it — with the **CI lower bound**, the interval, and the workload. |
| `verdict: rejected-bench`, CI spans 1.0 (e.g. `[0.983, 1.001]`) | Indistinguishable from no change. | "No shippable win." A real, complete result. |
| `verdict: rejected-bench`, CI **entirely below** 1.0 (e.g. `[0.91, 0.96]`) | A measured **regression**. | Report it as one, with the interval. Do not call it "no win." |
| Any risk signal from step 6 | Out of the machine's hands. | `needs-human-review`. Never auto-accept. |

Claim the **lower bound**, not the median: a median of 1.088 with CI
[1.074, 1.100] supports "≥7.4% faster", not "8.8% faster".

If the result is a rejection, **that is the answer**. Re-running until it passes
is p-hacking. Two failed hypotheses on the same hot path means you need a new
hypothesis, not another run.

## Services (latency, not throughput)

For a server, wall-clock-per-run is the wrong instrument. Service mode is an
open-loop, fixed-rate load generator that measures each request from its
**intended** send time, so queueing delay is counted rather than hidden
(coordinated omission). It reports p50/p99 with bootstrap CIs.

**It is not a generic HTTP load generator.** The server binary must:

- take `<port> <doc> <iters>` as positional argv;
- print a line beginning with `READY` on stdout, **flushed**, once listening;
- answer `GET /` and **close the connection** (`Connection: close`) — the client
  reads until EOF with no timeout, so a keep-alive server hangs the harness
  forever.

Point it at a stock Express or Flask app and it will either fail the READY
handshake or block indefinitely. Peltier wrote a thin adapter binary
(`targets/cjson/service.c`) for exactly this reason; you will need one too.

    $BR --config "$CFG" service --baseline-bin '<bin>' --candidate-bin '<bin>' \
        --doc <workload> --rate 150 --sessions 20

Calibrate it the same way first (`service-calibrate`). Tail percentiles on a
single-worker loopback setup are jittery — **if the p99 CI is wide, do not claim
p99**, even when p50 is solid. Peltier's own shipped service result claims p50
(+6.2%, CI [+5.8%, +7.2%]) and explicitly declines to claim p99 (CI [0.07, 4.97]).

## Record the attempt — including the failures

Outside peltier there is no ledger, so write the evidence where the next person
will find it (a results file, the PR body): hypothesis, what changed,
equivalence result, calibration evidence, the CI, the workload, and the verdict.
**Failures are the most valuable rows** — they stop someone from re-running your
dead end in six months.
