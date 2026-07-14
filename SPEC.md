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

  > **Amended 2026-07-13.** As built: **wall time is the only metric a verdict is decided on.** Max RSS is captured per timed run on Unix (`wait4` / `ru_maxrss`) and recorded as baseline/candidate medians in the env fingerprint — it is **context, never an accept metric**. PMU counters (cycles, instructions, cache misses, branch misses) are opt-in diagnostics via `bench-runner compare --perf-stat` — printed with the workload caveat, never part of the accept decision, and unavailable in most VMs. RAPL energy capture was never implemented. The **"no-network container"** named in the environment-control bullet above is *not* what ships — see the §10 amendment for the isolation actually built (a network namespace on the agent path only).
- Modes: (a) whole-program via hyperfine-style CLI timing; (b) microbench via criterion harness hooks; (c) service mode (Phase 4): wrk2 / vegeta load replay with **coordinated-omission-aware** latency recording (HdrHistogram).

Acceptance: passes A/A calibration at <5% false-positive rate and injected-regression detection at ≥95%, documented in `results/calibration/`.

### 3.2 `diff-test` (equivalence gates)

Layered, all must pass:
1. **Upstream test suite** of the target, unmodified, green.
2. **Golden replay**: recorded input corpus → byte-identical outputs (stdout, files, exit codes). FP-producing targets declare an explicit tolerance policy in `targets/<name>/equivalence.toml`; absent a policy, byte-identical is required.
3. **Differential fuzzing** of changed functions: old vs. new implementation compiled side by side, driven by proptest/cargo-fuzz (Rust) or libFuzzer (C/C++), outputs compared. Minimum 60s or 10k iterations per changed function.
4. **Sanitizers** on the candidate: ASan + UBSan always; TSan when the diff touches threads, atomics, or locks; MIRI for Rust targets where feasible.

Corpus and test files live outside agent-writable paths and are hash-pinned; `diff-test` refuses to run if hashes mismatch.

> **Amended 2026-07-13 — layer 3 (differential fuzz) as actually built.** Fuzz is a **baseline-vs-candidate** gate: it needs both binaries, so it runs *only* on the accept path (`verdict`, the one flow that rebuilds a pristine baseline). The standalone `diff-test` / `just gates` flow has no baseline and records the gate as **Skipped, with that reason** — it is never faked by comparing the candidate against itself. `diff-test` executes the target's declared `[gates].fuzz` command and grades it strictly from a `FUZZ-RESULT iters=<n> divergences=<m>` line; a run that never prints the line is a **Failed** gate, not a pass by silence. The ledger records the iteration count actually executed. **Hard rule:** a machine `accepted` verdict is impossible unless this gate *Passed* — skipped or failed caps the verdict at `needs-human-review`. `cjson`, `comrak`, and `tokei` declare `[gates].fuzz`; `matmul` does not. The per-changed-function scoping and the 60s/10k minimum remain aspirational: the shipped fuzzers are per-target whole-binary differential drivers (`scripts/diff-fuzz-*.py`) with a configurable iteration budget.

> **Amended 2026-07-13 — layer 4 (sanitizers) as actually built.** ASan+UBSan is machine-enforced on the accept path: `verdict` builds the target's `[build].sanitizer` binary, runs the pinned workload under it, and caps a flagged run — or a target that declares **no** sanitizer build — at `needs-human-review`. The **TSan lane is opt-in per target** (`[build].tsan` + `[build].tsan_binary`) rather than diff-triggered, and **no target declares it today**, so the lane is wired but **inert and no TSan coverage currently exists**; a threaded patch requires adding that opt-in first. **MIRI was never implemented.**

> **Amended 2026-07-13 — test-suite pinning: mechanized, not yet active.** As built, upstream test suites are vendored inside `targets/<t>/workspace/` — an agent-writable path — so "unmodified" is enforced by pinning, not placement: `corpora/<t>/TESTSUITE.sha256` (generated only by a deliberate human run of `scripts/pin-testsuite.sh`) is verified by `diff-test` before the suite runs; a hash mismatch is a hard refusal, same posture as the corpus pin. Targets without a pin file run with an explicit "suite unpinned" warning, and `targets/fetch.sh` verifies the pin after every fetch. **Status: no target ships a pin — there are zero `TESTSUITE.sha256` files in the repo — so this gate is currently inert and constrains nothing.** It becomes real only when pins are generated per target after a fetch. (The *corpus* pin, `MANIFEST.sha256`, ships for every target and **is** enforced today.)

### 3.3 `ledger`

Append-only SQLite. One row per attempt:
`run_id, timestamp, target, target_commit, phase, hotspot (symbol + % of profile), playbook_class, hypothesis, patch (diff), gates {tests, golden, fuzz_iters, sanitizers}, bench {baseline_ci, candidate_ci, delta_ci, env_fingerprint}, verdict {accepted | rejected-gate | rejected-bench | needs-human-review}, tokens_spent, wall_time`.

Nothing is ever deleted. The ledger is the audit trail, the anti-double-attempt memory, and the future training set.

### 3.4 `profiler adapter`

Phase 0–2: `perf record` + flamegraphs (inferno) + `perf stat`; hotspot ranking = exclusive samples by symbol with source mapping. Phase 3: add **coz** causal profiling — "what does speeding this line up buy end-to-end" — which is the literal bridge from cycles to ROI and the correct prioritizer for multi-threaded targets.

### 3.5 `agent/`

Claude Agent SDK (Python). Tools exposed to the model — seven: `read_profile`, `read_ledger`, `read_playbook`, `read_target_source` (read-only), `propose_patch(diff, hypothesis)`, `run_verdict`, `read_verdict`. Deliberately **not** exposed: shell on the host, writes outside `targets/<name>/`, any access to `crates/`, `config/`, `corpora/`. *(Amended 2026-07-13: `run_verdict` launches the pipeline detached and returns immediately; the pollable `read_verdict` — added in Phase 2 when the pipeline outlived the MCP transport timeout — reads the result, making seven operations total.)*

Prompting spine: (1) state hypothesis before patching; (2) prefer the cheapest untried playbook class; (3) max two iterations per rejected hypothesis; (4) a rejection with a clean ledger row is a successful outcome.

### 3.6 `report/`

Consumes ledger rows, emits per-engagement ROI report:
- Throughput jobs: cores saved = fleet_cores × (1 − 1/speedup); $ = cores × $/core-hr × hours/yr (rates in `config/pricing.toml`).
- Services: p50/p95/p99 deltas under replayed traffic, with the business-value mapping left as a customer-supplied parameter and one worked example.
- Optional: joules saved (RAPL) for the sustainability line. *(Amended 2026-07-13: never implemented — RAPL energy is not captured anywhere in the harness, so no report emits a joules figure. See the §3.1 amendment.)*
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

> **Amended 2026-07-13.** The Phase 2 criterion as written — "zero false accepts across ≥20 audited attempts" — was violated once *in-pipeline*: `phase2-comrak-010` was falsely accepted by the pipeline (no machine sanitizer gate existed yet), caught by the 100% human audit, overturned, and never shipped. The criterion was met in the form **zero *shipped* false accepts**; the overturn drove the machine-enforced sanitizer gate and stands documented in the Phase 2 case study.

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

> **Amended 2026-07-13.** The needs-human-review tier is machine-routed, not honor-system: `verdict` runs a **conservative lexical risk classifier** over the patch's changed lines (substring token lists for concurrency, unsafe, and floating-point signals) and forces any would-be accept carrying a signal to `needs-human-review`; using **fp-tolerance equivalence mode** at all also auto-routes to review; the manual `--needs-human-review` flag remains. The classifier is lexical and deliberately over-triggering — it cheaply catches the presence of risk markers, it does not prove their absence.

## 9. ROI methodology (the pitch artifact)

The report answers one question: *what did the stopwatch say, and what does that cost or buy?* Throughput → cores → dollars via public cloud pricing; latency → percentile deltas under recorded production-shaped load, priced by the customer's own latency-value number; batch windows → wall-clock hours returned to the schedule. All three come with CIs and a workload statement. The methodology section ships *inside* every report so the number survives hostile review — that is the differentiation.

## 10. Security & anti-reward-hacking (load-bearing)

The agent's incentive is "make the number go up"; the design must make cheating structurally impossible, not merely discouraged:
- Trust layer (`crates/`, `config/`, `corpora/`, upstream tests) is **read-only to the agent** — enforced by filesystem permissions and a separate-process harness, not by prompt.
- Patches are applied by the harness via `git apply` after a path-allowlist check; the agent never holds a general shell on the host.
- Baselines rebuilt from pristine checkouts; corpora hash-pinned; `diff-test` refuses on mismatch.
- Target code executes only in no-network containers (bench containers additionally seccomp-restricted).
- Periodic human audit of accepted wins (100% in Phase 2, sampled thereafter); any false accept is a stop-the-line event.

> **Amended 2026-07-13 — isolation as actually built.** The bullet above ("target code executes only in no-network containers, seccomp-restricted") is **not** what ships. What ships:
>
> - **The `harnessd`-launched verdict pipeline** — the agent path — execs through `scripts/no-net.sh`, a **network-namespace** wrapper (`unshare --net --map-current-user`). `HOTPATH_VERDICT_WRAPPER` overrides the wrapper.
> - **It fails closed.** Where namespaces are unavailable the wrapper **exits 97 without running the pipeline**, rather than degrading silently. The only bypass is the explicit `HOTPATH_ALLOW_UNISOLATED=1` override — which runs the pipeline **with full network access**, warns loudly on stderr, and is recorded verbatim in the ledger's isolation note (`"no-net.sh (HOTPATH_ALLOW_UNISOLATED=1 — network NOT isolated)"`), so a run can never *claim* an isolation it did not have.
> - **In CI**, the bench workload runs under `docker run --network=none`.
>
> **Two gaps, stated plainly.** (1) **Only the harnessd (agent) path is wrapped.** A human invoking `just verdict` directly runs **unwrapped** on the host; such rows record `isolation: "unwrapped-host"`. (2) **Full-container isolation of the whole pipeline remains open** — the wrapper isolates the *network*, not the filesystem or the syscall surface; the seccomp-restricted bench container of this section is not built. The agent-side OS boundary (mount-namespace / unprivileged-uid isolation, `just isolation-check`) is a separate mechanism and *is* shipped.

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

> **Amended 2026-07-13 — which mitigations above are actually live.** The "Concurrency bugs introduced" and "UB introduced" rows overstate the machine coverage. Live today: **UBSan** (bundled with ASan on the accept path, machine-enforced) and the **human-review tier**, which is machine-routed by the lexical risk classifier of the §8 amendment — so a concurrency-token or `unsafe`-token patch *is* forced to `needs-human-review`, conservatively and lexically. **Not live: the TSan gate** (opt-in via `[build].tsan`; no target declares it) and **MIRI** (never implemented). The concurrency and UB rows therefore rest on the review tier and UBSan, not on TSan/MIRI.

## 12. Tech stack

Rust workspace (`bench-runner`, `diff-test`, `ledger`, `report`) · `just` · hyperfine-style timing + criterion hooks · perf, inferno flamegraphs, coz · proptest, cargo-fuzz, libFuzzer · ASan/UBSan/TSan, MIRI · podman/docker (no-net) · wrk2 + HdrHistogram, vegeta · SQLite · Python 3.11+ with Claude Agent SDK (`agent/`) — confirm current SDK package name and auth at https://docs.claude.com/en/docs/claude-code/overview.

> **Amended 2026-07-13 — the stack as actually used.** This list is the *planned* stack, not an inventory. In use: the Rust workspace (plus `verdict`, `harnessd`, `policy`), `just`, in-house interleaved timing and an in-house open-loop load generator (**not** hyperfine/criterion/wrk2/vegeta), valgrind-callgrind and coz for profiling (`perf` is often unavailable in the container), Python differential fuzzers (`scripts/diff-fuzz-*.py` — **not** proptest/cargo-fuzz/libFuzzer), ASan+UBSan, SQLite, and the Agent SDK. **Not used: MIRI (never implemented), TSan (wired but no target opts in), and podman/docker for the local verdict path** — local isolation is a network namespace (`scripts/no-net.sh`); Docker `--network=none` is used in CI.

## 13. Later / research fork (explicitly out of scope now)

- GPU kernel lane (where 2–10x gaps live): Triton/CUDA targets, correctness via reference-kernel differential testing, reward = measured kernel time.
- Learned optimization policies: ledger → RL; DELTA-over-program-graphs (ProGraML-style IR graphs) as the policy/ranking model. Prior art anchors: MLGO (learned inlining/regalloc in LLVM), AlphaDev (Nature 2023), Meta LLM Compiler (2024).

## 14. References

Mytkowicz et al., *Producing Wrong Data Without Doing Anything Obviously Wrong* (ASPLOS 2009) — read before building bench-runner · Curtsinger & Berger, *Stabilizer* and *coz* · Gil Tene on coordinated omission · MLGO (Trofin et al.) · ProGraML (Cummins et al.) · AlphaDev (Mankowitz et al., Nature 2023) · Meta LLM Compiler (Cummins et al., 2024).
