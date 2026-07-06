# hotpath — Engineering Spec v0.1

## 1. Mission & thesis

Performance engineering is scarce artisan labor. Compilers closed the gap at general codegen; nothing closed it at the application and infrastructure layer, where profile-guided rewrites, build configuration, data-layout changes, and kernel-level work still leave 10–90% on the floor. hotpath is an agent that harvests that gap with two properties no consultant offers: **verified equivalence** (a change provably preserves behavior or is discarded) and **self-verifying ROI** (the value claim is a stopwatch on trusted infrastructure, not a survey).

Why this wins:
- **Empty niche.** Between "the compiler" and "hire a perf consultant" there is no product. LLM codegen is crowded; verified optimization is not.
- **The metric defends itself.** Same inputs, same outputs, fewer cycles, with confidence intervals. No correlation-causation problem is possible by construction.
- **The ledger compounds.** Every accepted/rejected attempt with full evidence is future RL training data and, eventually, the substrate for learned optimization policies (see §13, research fork).

## 2. System overview

```
                        ┌────────────────────────────────────────────┐
                        │              TRUST LAYER                   │
                        │  (agent has NO write access to this box)   │
  ┌─────────┐  patch    │  ┌────────────┐  ┌───────────┐  ┌───────┐  │
  │  agent/ │──────────▶│  │ diff-test  │─▶│bench-runner│─▶│ledger │  │
  │ (Agent  │  via git  │  │ gates      │  │ interleaved│  │SQLite │  │
  │  SDK)   │  apply +  │  └────────────┘  │ A/B + CIs  │  └───┬───┘  │
  └────┬────┘  allowlist│                  └───────────┘      │      │
       │                └──────────────────────────────────────┼──────┘
       │ reads profiles, ledger, playbook                      ▼
       ▼                                                  ┌─────────┐
  ┌──────────┐                                            │ report/ │
  │ profiler │  perf / flamegraph / coz                   │  ROI    │
  │ adapter  │                                            └─────────┘
  └──────────┘
```

Loop: profile → rank hotspots → select playbook class → hypothesize → patch → gates → bench → verdict → ledger → repeat.

## 3. Component specs

### 3.1 `bench-runner` (the product is this crate)

Requirements:
- Runs baseline and candidate **interleaved** (ABABAB…), never sequential blocks, to control thermal and background drift.
- Baseline is rebuilt from a **pristine checkout** each session — never from agent workspace.
- Environment control: CPU pinning (`taskset`/cgroup cpuset), performance governor, turbo/SMT state recorded (disabled where root permits), ASLR handled explicitly (see §7), no-network container, warm-up runs discarded.
- Statistics: ≥30 measured runs per side (configurable); report median and bootstrap 95% CI of the ratio; effect accepted only if CI lower bound ≥ threshold (`config/accept.toml`, default 2%).
- **A/A self-test mode**: run the same binary as both sides; must produce a null verdict. Scheduled automatically before any measurement session.
- Regression-injection self-test: a synthetic 5% slowdown must be detected ≥95% of the time.
- Metrics captured per run: wall time, cycles, instructions, cache misses, branch misses (via `perf stat`), max RSS, and RAPL energy where available.
- Modes: (a) whole-program via hyperfine-style CLI timing; (b) microbench via criterion harness hooks; (c) service mode (Phase 4): wrk2 / vegeta load replay with **coordinated-omission-aware** latency recording (HdrHistogram).

Acceptance: passes A/A calibration at <5% false-positive rate and injected-regression detection at ≥95%, documented in `results/calibration/`.

### 3.2 `diff-test` (equivalence gates)

Layered, all must pass:
1. **Upstream test suite** of the target, unmodified, green.
2. **Golden replay**: recorded input corpus → byte-identical outputs (stdout, files, exit codes). FP-producing targets declare an explicit tolerance policy in `targets/<name>/equivalence.toml`; absent a policy, byte-identical is required.
3. **Differential fuzzing** of changed functions: old vs. new implementation compiled side by side, driven by proptest/cargo-fuzz (Rust) or libFuzzer (C/C++), outputs compared. Minimum 60s or 10k iterations per changed function.
4. **Sanitizers** on the candidate: ASan + UBSan always; TSan when the diff touches threads, atomics, or locks; MIRI for Rust targets where feasible.

Corpus and test files live outside agent-writable paths and are hash-pinned; `diff-test` refuses to run if hashes mismatch.

### 3.3 `ledger`

Append-only SQLite. One row per attempt:
`run_id, timestamp, target, target_commit, phase, hotspot (symbol + % of profile), playbook_class, hypothesis, patch (diff), gates {tests, golden, fuzz_iters, sanitizers}, bench {baseline_ci, candidate_ci, delta_ci, env_fingerprint}, verdict {accepted | rejected-gate | rejected-bench | needs-human-review}, tokens_spent, wall_time`.

Nothing is ever deleted. The ledger is the audit trail, the anti-double-attempt memory, and the future training set.

### 3.4 `profiler adapter`

Phase 0–2: `perf record` + flamegraphs (inferno) + `perf stat`; hotspot ranking = exclusive samples by symbol with source mapping. Phase 3: add **coz** causal profiling — "what does speeding this line up buy end-to-end" — which is the literal bridge from cycles to ROI and the correct prioritizer for multi-threaded targets.

### 3.5 `agent/`

Claude Agent SDK (Python). Tools exposed to the model: `read_profile`, `read_ledger`, `read_playbook`, `propose_patch(diff, hypothesis)`, `run_verdict`, `read_target_source` (read-only). Deliberately **not** exposed: shell on the host, writes outside `targets/<name>/`, any access to `crates/`, `config/`, `corpora/`.

Prompting spine: (1) state hypothesis before patching; (2) prefer the cheapest untried playbook class; (3) max two iterations per rejected hypothesis; (4) a rejection with a clean ledger row is a successful outcome.

### 3.6 `report/`

Consumes ledger rows, emits per-engagement ROI report:
- Throughput jobs: cores saved = fleet_cores × (1 − 1/speedup); $ = cores × $/core-hr × hours/yr (rates in `config/pricing.toml`).
- Services: p50/p95/p99 deltas under replayed traffic, with the business-value mapping left as a customer-supplied parameter and one worked example.
- Optional: joules saved (RAPL) for the sustainability line.
- Every figure carries CI + workload description + environment fingerprint. Caveats are printed on the report, not in an appendix.

## 4. Optimization playbook v0 (strict order)

1. **Build configuration**: opt level, `-march`/`target-cpu`, LTO (thin → fat), PGO, BOLT where applicable.
2. **Allocator swap**: mimalloc/jemalloc trial. (Classes 1–2 are the "your 30% was a linker flag" tier — always exhausted first.)
3. **Allocation churn**: hoisting, reuse, arenas, small-vector.
4. **Data layout**: struct packing/reordering, AoS→SoA, false-sharing fixes, cache blocking.
5. **Algorithmic**: complexity class, better std/library primitive, memoization, precomputation.
6. **SIMD**: autovectorization enablement first (aliasing, bounds, FP flags — FP flags trigger human review), intrinsics last.
7. **Concurrency**: contention reduction, sharding, lock-free only with `needs-human-review`.

Each class ships as `playbook/NN-name.md`: preconditions (profile signature that suggests it), procedure, verification notes, known failure modes.

## 5. Phase plan & exit criteria

| Phase | Scope | Exit criteria |
|---|---|---|
| **0** Manual dry run (1–2 wknd) | Two CPU-bound OSS targets; loop run by hand with Claude Code as copilot | ≥1 verified win ≥10% on one target; written case study with methodology |
| **1** Trust layer (3–4 wknd) | bench-runner + diff-test + ledger | A/A false-positive <5%; injected 5% regression caught ≥95%; gates run on 2 targets end-to-end |
| **2** Agent (3–4 wknd) | Agent SDK loop, Rust targets first | Unattended profile→verdict loop on 1 target; ≥1 auto-accepted win; **zero false accepts** across ≥20 audited attempts |
| **3** Playbook + proof | Classes 3–7, coz, C/C++ targets | 3–5 public case studies; zero regressions shipped; playbook ≥6 classes |
| **4** Services & scale | Load replay, latency mode, ROI reports | One real service workload optimized under replayed traffic; report generated mechanically |

Language order: Rust first (criterion/proptest/MIRI make Phases 1–2 cheaper), C/C++ second (the richer legacy hunting ground), services third.

## 6. Target selection criteria (Phase 0–3)

CPU-bound (>70% user time on profile); meaningful test suite; existing benchmark or easily scripted workload; permissive license; active but not hyper-optimized (skip anything with a dedicated perf team); builds cleanly in container. Good hunting: codecs, parsers, serializers, image processing, compression, static-site generators, linters.

## 7. Statistical methodology notes

- Interleaved A/B with pristine-checkout baselines is the core defense against drift.
- **Layout bias** (Mytkowicz et al., ASPLOS'09: link order and env size alone swing results past "real" effects): mitigate by randomizing link order / ASLR *across* runs and aggregating, Stabilizer-style, rather than fixing one lucky layout. Document the chosen mode in the env fingerprint.
- Report ratios with bootstrap CIs; never means without spread; never a single run, ever.
- Service latency: wrk2 constant-throughput mode + HdrHistogram to avoid coordinated omission; never trust closed-loop load generators for p99 claims.

## 8. Equivalence policy tiers

- **Auto-acceptable**: bit-identical outputs across all gates, no FP-ordering changes, no concurrency-primitive changes, sanitizers clean.
- **needs-human-review** (never auto-accepted): FP reassociation/contraction or `-ffast-math`-class flags; any diff touching atomics, locks, memory ordering, or thread counts; unsafe blocks / raw pointer arithmetic introduced; anything the sanitizers flag even as warnings.
- **Auto-rejected**: any gate failure, any divergence under differential fuzz, patch outside allowlist.

## 9. ROI methodology (the pitch artifact)

The report answers one question: *what did the stopwatch say, and what does that cost or buy?* Throughput → cores → dollars via public cloud pricing; latency → percentile deltas under recorded production-shaped load, priced by the customer's own latency-value number; batch windows → wall-clock hours returned to the schedule. All three come with CIs and a workload statement. The methodology section ships *inside* every report so the number survives hostile review — that is the differentiation.

## 10. Security & anti-reward-hacking (load-bearing)

The agent's incentive is "make the number go up"; the design must make cheating structurally impossible, not merely discouraged:
- Trust layer (`crates/`, `config/`, `corpora/`, upstream tests) is **read-only to the agent** — enforced by filesystem permissions and a separate-process harness, not by prompt.
- Patches are applied by the harness via `git apply` after a path-allowlist check; the agent never holds a general shell on the host.
- Baselines rebuilt from pristine checkouts; corpora hash-pinned; `diff-test` refuses on mismatch.
- Target code executes only in no-network containers (bench containers additionally seccomp-restricted).
- Periodic human audit of accepted wins (100% in Phase 2, sampled thereafter); any false accept is a stop-the-line event.

## 11. Risk register

| Risk | Mitigation |
|---|---|
| Measurement bias / noise | §7 discipline; A/A tests before every session; env fingerprints |
| Overfitting to the benchmark | Holdout workloads; Phase 4 uses recorded real traffic only |
| FP equivalence gaps | Explicit per-target tolerance policy; default byte-identical; human review tier |
| Concurrency bugs introduced | TSan gate; human-review tier for all concurrency diffs |
| UB introduced | UBSan always; MIRI for Rust; human-review for unsafe |
| Agent games the harness | §10 structural controls; audits |
| License/IP hygiene | Permissive-license targets only for public case studies |
| Wins too small to matter | Cheap-wins-first ordering keeps cost-per-attempt low; ledger prevents re-grinding dead ends |

## 12. Tech stack

Rust workspace (`bench-runner`, `diff-test`, `ledger`, `report`) · `just` · hyperfine-style timing + criterion hooks · perf, inferno flamegraphs, coz · proptest, cargo-fuzz, libFuzzer · ASan/UBSan/TSan, MIRI · podman/docker (no-net) · wrk2 + HdrHistogram, vegeta · SQLite · Python 3.11+ with Claude Agent SDK (`agent/`) — confirm current SDK package name and auth at https://docs.claude.com/en/docs/claude-code/overview.

## 13. Later / research fork (explicitly out of scope now)

- GPU kernel lane (where 2–10x gaps live): Triton/CUDA targets, correctness via reference-kernel differential testing, reward = measured kernel time.
- Learned optimization policies: ledger → RL; DELTA-over-program-graphs (ProGraML-style IR graphs) as the policy/ranking model. Prior art anchors: MLGO (learned inlining/regalloc in LLVM), AlphaDev (Nature 2023), Meta LLM Compiler (2024).

## 14. References

Mytkowicz et al., *Producing Wrong Data Without Doing Anything Obviously Wrong* (ASPLOS 2009) — read before building bench-runner · Curtsinger & Berger, *Stabilizer* and *coz* · Gil Tene on coordinated omission · MLGO (Trofin et al.) · ProGraML (Cummins et al.) · AlphaDev (Mankowitz et al., Nature 2023) · Meta LLM Compiler (Cummins et al., 2024).
