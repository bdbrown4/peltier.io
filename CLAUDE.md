# hotpath — profile-guided optimization agent

> Working name. Rename freely; nothing depends on it.

## Mission

An agent that profiles real binaries and services, isolates hot paths, proposes optimizations, and accepts a change **only** when (a) behavioral-equivalence gates pass and (b) the benchmark improvement is statistically significant on trusted infrastructure. Output: verified performance deltas translated into ROI reports nobody can dispute.

Full engineering spec: `SPEC.md`. Read it before starting any phase.

## Non-negotiables (re-read every session)

1. **No unverified performance claims.** A change without passing equivalence gates AND a significant bench delta does not exist. Discard it. Log it.
2. **Measure before optimizing.** No patch is proposed until profile data identifies the hot path. Never trust a single benchmark run.
3. **Cheap wins first, always.** Build flags → LTO → PGO → allocator swap → *then* code changes. Check the ledger before re-attempting any class of change on a target.
4. **Equivalence gates are hard gates.** Changes touching floating-point ordering, concurrency primitives, or anything UB-adjacent are NEVER auto-accepted — verdict is `needs-human-review`.
5. **Every attempt goes in the ledger**, including failures and rejections. Failures are future training data.
6. **The agent never modifies the trust layer.** `crates/`, `config/`, `corpora/`, and upstream test suites are read-only to the agent. Patches may touch only allowlisted paths under `targets/<name>/`. See SPEC.md §10.
7. **Sandbox everything.** Target code runs only inside no-network containers. Never on the host.
8. **Statistical bar:** accept only if the bootstrap 95% CI lower bound of speedup ≥ threshold in `config/accept.toml` (default 2%), from interleaved A/B runs on the same pinned hardware.

## Architecture map

- `crates/bench-runner` — trust layer: containerized, CPU-pinned, interleaved A/B benchmarking with bootstrap confidence intervals and A/A self-tests
- `crates/diff-test` — behavioral equivalence: upstream test suite + golden I/O replay + differential fuzzing of changed functions + sanitizers
- `crates/ledger` — append-only SQLite record of every attempt: hypothesis, patch, gate results, bench deltas, verdict, cost
- `crates/report` — ROI generator: cores saved × $/core-hr, latency percentile deltas, CIs, workload caveats printed on every number
- `agent/` — Claude Agent SDK (Python) loop, prompts, tool definitions
- `targets/` — vendored OSS targets for case studies (permissive licenses only)
- `playbook/` — optimization classes, ordered, with preconditions and known risks
- `results/` — generated reports and flamegraphs

**Phase 4 COMPLETE — all SPEC §5 exit criteria met. Phases 0–4 are all done.** Service-mode latency benchmarking shipped: a coordinated-omission-correct, open-loop, fixed-rate load generator (`crates/bench-runner/src/service.rs`) measuring latency from each request's intended send time (queueing counted, not hidden), interleaved A/B, exact percentiles, bootstrap p50/p99 CIs. A real service target — `targets/cjson/service.c`, a minimal HTTP server wrapping the patched `cJSON.c` (trust-layer, outside the workspace allowlist). Service calibration PASS (0/10 A/A false-positive, 10/10 injected-5%-latency-regression detection; `results/calibration/cjson-service-aa.json`). **The verified batch win phase3-cjson-002 measured under a 150 rps replay (20 rounds, 40k requests, 0 drops): p50 latency +6.2%, 95% CI [+5.8%, +7.2%] — accepted (`phase4-cjson-service` ledger row); p99 NOT claimed (CI [0.07, 4.97], single-worker loopback tail jitter) — harness correctly rejected it.** Mechanical ROI report generator (`crates/report` bin, `just report`, SPEC §9): reads a ledger row + service-latency JSON → throughput→cores→dollars (27.5 cores / $9,621-per-year CI lower bound on a 500-core fleet) *and* latency percentiles, every figure with CI + workload + methodology inline; flags any non-accepted row. Case study: `results/phase4/`. **The roadmap is complete — the profile→hypothesize→patch→gated-verdict→ROI loop runs end to end across Rust and C, batch and service, with zero shipped false accepts across all phases.**

**Phase 5 COMPLETE — the two SPEC §13 research forks, both built and both routed to human review.** (a) *Learned class-selection policy* (`crates/policy`, `cargo run -p policy`): reads the append-only ledger and ranks optimization classes by the Wilson lower bound of their shippable-win rate — a learned prior over the fixed cheapest-first ordering, advisory only (gates still decide). A "win" counts only machine-sanitizer-verified accepts; the overturned comrak-010 and tier-gated mimalloc are excluded/held. On the current 34-row ledger it recommends algorithmic first (Wilson lb 0.066). (b) *Kernel lane* (`targets/matmul/kernel.c`, `crates/diff-test` FP-tolerance policy + `fp-compare` bin, `scripts/kernel-lane-demo.sh`): a matmul optimization (transpose-B cache locality + eight-accumulator ILP) that **reorders FP accumulation**, so byte-identical golden replay is the wrong gate — 244,901/262,144 values differ. The FP-tolerance gate (`abs 1e-4 + rel 1e-3`) accepts it, still REJECTS a +0.5 perturbation, and the bench measures 3.23× [3.16, 3.26] with the same interleaved A/B + bootstrap CI. Ledger row `phase5-matmul-opt` = needs-human-review (using the tolerance tier is a §8 signal, same posture as mimalloc). No GPU in this environment, so the GPU fork is the identical shape shown on CPU — only the timer and hardware change. Case study: `results/phase5/`; docs chapter `docs/src/research-forks.md`. Open follow-up: the standing accept-scoped ruling on `phase0-comrak-002` (mimalloc) is resolved (`results/rulings/`).**

## Commands

Fill these in as the harness is built; keep them current:

```
just profile <target>       # perf record + flamegraph for a target's benchmark workload
just bench <target> <ref>   # interleaved A/B: pristine baseline vs working tree
just gates <target>         # test suite + golden replay + diff-fuzz + sanitizers
just verdict <target>       # runs gates + bench, writes ledger row, prints verdict
just report <run-id>        # ROI report from ledger
just aa <target>            # A/A calibration run (must show null result)
```

Built so far (command-level, pre-target-integration):

```
just build / test / lint    # trust-layer workspace: cargo build/test/clippy+fmt
just aa [cmd]               # A/A self-test of bench-runner on a shell command
just compare <a> <b>        # interleaved A/B of two shell commands, bootstrap CI
just gates <target>         # corpus pin + upstream tests + golden replay
just pin-check <target>     # verify corpus MANIFEST.sha256
just calibrate <cmd> <out>  # automated A/A + regression-injection calibration
just verdict <t> <bin> ...  # gates + bench vs pristine-rebuilt baseline + ledger row
cargo run -p harnessd       # agent IPC daemon: six-tool JSON surface (SPEC §3.5)
```

## Definition of done — one optimization attempt

- [ ] Hypothesis logged in ledger *before* patching
- [ ] Patch touches only allowlisted paths under `targets/<name>/`
- [ ] Upstream test suite green
- [ ] Golden replay byte-identical (or within the target's explicit FP tolerance policy)
- [ ] Differential fuzz on changed functions: no divergence (60s or 10k iterations minimum)
- [ ] ASan + UBSan clean; TSan if the patch touches anything threaded
- [ ] Interleaved A/B bench passes the CI-lower-bound threshold
- [ ] Ledger row written with verdict and full evidence
- [ ] If FP-ordering / concurrency / UB-adjacent: verdict `needs-human-review`, never auto-accept

An attempt that fails any gate is a **valid, complete outcome** — write the ledger row and move on. Do not iterate on a rejected patch more than twice without a new hypothesis.

## Conventions

- Rust workspace (edition 2021+) for all harness crates; `just` for task running
- Python 3.11+ for `agent/`; Claude Agent SDK — verify current package name and API at https://docs.claude.com/en/docs/claude-code/overview before scaffolding
- Every number shown to a human carries its confidence interval and workload description. No naked percentages, ever.
- Commit style: `phase0:`, `bench:`, `gates:`, `agent:`, `playbook:` prefixes
- When in doubt between shipping a feature and hardening measurement, harden measurement. The product is trust.
