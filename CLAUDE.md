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
7. **Sandbox everything.** Target code must never run unisolated on the host. *As built:* the harnessd-launched verdict pipeline execs through `scripts/no-net.sh` (Linux network namespace; **fails closed** — exit 97 — when namespaces are unavailable, unless `HOTPATH_ALLOW_UNISOLATED=1`, which runs it **with** network and stamps that fact into the ledger's isolation note). CI runs the bench workload under `docker run --network=none`. Two honest limits: a human invoking `just verdict` directly is **not** wrapped, and full-container (seccomp-restricted) isolation of the whole pipeline is still an **open gap**. See SPEC §10.
8. **Statistical bar:** accept only if the bootstrap 95% CI lower bound of speedup ≥ threshold in `config/accept.toml` (default 2%), from interleaved A/B runs on the same pinned hardware.

## Architecture map

- `crates/bench-runner` — trust layer: CPU-pinned, interleaved A/B benchmarking with bootstrap confidence intervals and A/A self-tests (it does **not** containerize anything itself — isolation is the wrapper's job, non-negotiable 7)
- `crates/diff-test` — behavioral equivalence: corpus/test-suite pin checks + upstream test suite + golden I/O replay + differential fuzz (a baseline-vs-candidate gate; sanitizers run on the accept path in `crates/verdict`, and `diff-test` reports them as skipped-with-reason)
- `crates/ledger` — append-only SQLite record of every attempt: hypothesis, patch, gate results, bench deltas, verdict, cost
- `crates/report` — ROI generator: cores saved × $/core-hr, latency percentile deltas, CIs, workload caveats printed on every number
- `agent/` — Claude Agent SDK (Python) loop, prompts, tool definitions
- `targets/` — vendored OSS targets for case studies (permissive licenses only)
- `playbook/` — optimization classes, ordered, with preconditions and known risks
- `results/` — generated reports and flamegraphs
- `.claude/skills/peltier/` — the trust layer packaged as a consumable **Agent Skill** (agentskills.io SKILL.md standard, spec-compliant frontmatter): `verify` mode (equivalence → A/A calibration → interleaved A/B + bootstrap CI) works in any repo against any two shell commands; `attempt` mode drives the full in-repo loop. Drives the real `bench-runner` binary and refuses on unsupported hosts rather than falling back to a second, forkable copy of the statistics. Cross-harness: mirrored byte-identically at `.agents/skills/peltier/` (the Codex/Zed/Cursor/Gemini/Copilot convergence path — CI enforces the mirror), `AGENTS.md` at the repo root points non-Claude harnesses here, and `scripts/install-skill.sh` stamps consumer repos (plus a script-less `--zeroclaw-variant`, since ZeroClaw blocks script files in skills by default — its preflight runs from the checkout). Preflight refusal, spec conformance, mirror equality, and the installer are all asserted in CI (`skill-preflight`).

**Phase 4 COMPLETE — all SPEC §5 exit criteria met. Phases 0–4 are all done.** Service-mode latency benchmarking shipped: a coordinated-omission-correct, open-loop, fixed-rate load generator (`crates/bench-runner/src/service.rs`) measuring latency from each request's intended send time (queueing counted, not hidden), interleaved A/B, exact percentiles, bootstrap p50/p99 CIs. A real service target — `targets/cjson/service.c`, a minimal HTTP server wrapping the patched `cJSON.c` (trust-layer, outside the workspace allowlist). Service calibration PASS (0/10 A/A false-positive, 10/10 injected-5%-latency-regression detection; `results/calibration/cjson-service-aa.json`). **The verified batch win phase3-cjson-002 measured under a 150 rps-target replay (20 interleaved rounds, 0 drops — the `phase4-cjson-service` ledger row records sessions=20, rate_rps=150, drop_rate=0.0; total request count is not machine-recorded): p50 latency +6.2%, 95% CI [+5.8%, +7.2%] — accepted; p99 NOT claimed (CI [0.07, 4.97], single-worker loopback tail jitter) — harness correctly rejected it.** Mechanical ROI report generator (`crates/report` bin, `just report`, SPEC §9): reads a ledger row + service-latency JSON → throughput→cores→dollars (27.5 cores / $9,621-per-year CI lower bound on a 500-core fleet) *and* latency percentiles, every figure with CI + workload + methodology inline; flags any non-accepted row. Case study: `results/phase4/`. **The roadmap is complete — the profile→hypothesize→patch→gated-verdict→ROI loop runs end to end across Rust and C, batch and service, with zero shipped false accepts across all phases.**

**Hardening pass (2026-07-13, `audit/resolve-findings`) — what is *enforced* vs. what is only *mechanized*.** Five gates were addressed in code. **Two are live on the verdict path (a, b); three are wired but currently inert, because no target has opted in (c, d, e).** The distinction is stated precisely because collapsing it is exactly the overclaim this project exists to prevent.

- **(a) Differential fuzz — live, and now blocks accepts.** `diff-test` runs the target's declared `[gates].fuzz` command and grades it strictly from a `FUZZ-RESULT iters=<n> divergences=<m>` line (no line = Failed, not a pass by silence); the ledger records the iteration count **actually executed**. It is a **baseline-vs-candidate** gate, so it runs *only* on the accept path (`crates/verdict`, the one flow that rebuilds a pristine baseline). The standalone `just gates` flow has no baseline, so the gate reports **Skipped with that reason** — `just gates` is a fast equivalence check, **not** a fuzz run. **Hard rule:** a machine `accepted` verdict is **impossible** unless the DifferentialFuzz gate actually *Passed*; skipped or failed caps the verdict at `needs-human-review`. `cjson`/`comrak`/`tokei` declare `[gates].fuzz`; `matmul` does not. *Ledger history:* fuzz ran out-of-band via `scripts/diff-fuzz-*.py` through Phase 5, so **8 of the 10 accepted rows record `fuzz_iters=0`** — the two exceptions, `phase0-tokei-002` and `phase0-tokei-003`, record `fuzz_iters=4332`.
- **(b) Lexical risk classifier — live.** `crates/verdict/src/risk.rs` scans the patch's **changed lines** for concurrency / `unsafe` / floating-point tokens; any signal on a would-be accept forces `needs-human-review`, and using fp-tolerance equivalence mode is itself always a signal. It is **lexical and deliberately over-triggering, not semantic** — it detects the presence of risk markers; it does not understand the code and cannot prove their absence. `harnessd` always passes `--patch-file`, so every agent-proposed patch is classified.
- **(c) Test-suite pinning — mechanized, INERT.** `diff-test` verifies `corpora/<t>/TESTSUITE.sha256` **when present** (mismatch = hard refusal, same posture as the corpus pin) and prints an explicit "suite unpinned" warning when absent. **No target ships a pin today — there are zero `TESTSUITE.sha256` files in the repo** — so the gate constrains nothing yet. Pins must be generated per target, post-fetch, by a deliberate `scripts/pin-testsuite.sh` run.
- **(d) TSan lane — mechanized, INERT.** `crates/verdict` builds and runs a TSan lane on the accept path **only when a target declares `[build].tsan`**. **No target declares it today, so no TSan coverage exists.** (ASan/UBSan *is* live on the accept path: a flagged run — or a target with no sanitizer build at all — caps the accept at `needs-human-review`.)
- **(e) FP-tolerance equivalence — mechanized in the pipeline, target NOT wired.** The gate pipeline honors an fp-tolerance policy (`targets/<t>/equivalence.toml`; requires `[corpus].golden_reference`), and any fp-tolerance run auto-routes to `needs-human-review`. But the only FP target, `matmul`, has an `equivalence.toml` and **no `target.toml`** — so it is not a pipeline target at all, and the kernel lane stays script-driven (`scripts/kernel-lane-demo.sh`).

**Phase 5 COMPLETE — the two SPEC §13 research forks, both built and both routed to human review.** (a) *Learned class-selection policy* (`crates/policy`, `cargo run -p policy`): reads the append-only ledger and ranks optimization classes by the Wilson lower bound of their shippable-win rate — a learned prior over the fixed cheapest-first ordering, advisory only (gates still decide). A "win" counts only machine-sanitizer-verified accepts; the overturned comrak-010 and tier-gated mimalloc are excluded/held. On the current 34-row ledger it recommends algorithmic first (Wilson lb 0.066). (b) *Kernel lane* (`targets/matmul/kernel.c`, `crates/diff-test` FP-tolerance policy + `fp-compare` bin, `scripts/kernel-lane-demo.sh`): a matmul optimization (transpose-B cache locality + eight-accumulator ILP) that **reorders FP accumulation**, so byte-identical golden replay is the wrong gate — 244,901/262,144 values differ. The FP-tolerance policy (`abs 1e-4 + rel 1e-3`) accepts it, still REJECTS a +0.5 perturbation, and the bench measures 3.23× [3.16, 3.26] with the same interleaved A/B + bootstrap CI. Ledger row `phase5-matmul-opt` = needs-human-review (using the tolerance tier is a §8 signal, same posture as mimalloc). **Honest scope:** the FP-tolerance *mechanism* lives in the gate pipeline, but `matmul` is **not wired into it** — it has an `equivalence.toml` and no `target.toml`, so `diff-test`/`verdict` cannot load it. The lane is demonstrated script-driven (`fp-compare` + `bench-runner compare`), and that ledger row was recorded outside the automated verdict path. Wiring `matmul` as a real pipeline target is open work. No GPU in this environment, so the GPU fork is the identical shape shown on CPU — only the timer and hardware change. Case study: `results/phase5/`; docs chapter `docs/src/research-forks.md`. Open follow-up: the standing accept-scoped ruling on `phase0-comrak-002` (mimalloc) is resolved (`results/rulings/`).**

## Commands

The real recipe list (`justfile`); keep it current:

```
just build / test / lint    # trust-layer workspace: cargo build/test/clippy+fmt
just aa [cmd]               # A/A self-test of bench-runner on a shell command
just compare <a> <b>        # interleaved A/B of two shell commands, bootstrap CI
just gates <target>         # corpus pin (+ suite pin if present) + upstream tests + golden replay.
                            #   diff-fuzz SKIPS here (needs a baseline) — it runs in `just verdict`
just pin-check <target>     # verify corpus MANIFEST.sha256 (verify-only, never re-pins)
just pin-corpus <target>    # deliberate corpus re-pin (writes MANIFEST.sha256)
just calibrate <cmd> <out>  # automated A/A + regression-injection calibration
just verdict <t> <bin> ...  # gates + bench vs pristine-rebuilt baseline + ledger row
just report <run-id>        # ROI report from a ledger row (CIs + methodology inline)
just agent-attempt <t> <id> # one unattended agent attempt behind the OS boundary
just isolation-check        # verify the SPEC §10 OS boundary (19 checks, both modes)
just service <b> <c> <doc>  # service-mode latency A/B, CO-correct, p50/p99 CIs
just service-calibrate ...  # latency A/A + injected-regression calibration
just coz <target>           # causal profile of a C/C++ target
cargo run -p harnessd       # agent IPC daemon: seven-op JSON surface (SPEC §3.5)
cargo run -p policy         # learned class-ranking prior from the ledger (advisory)
```

**Platform note:** the harness is Linux/POSIX-only at runtime — bench-runner
service mode, harnessd (Unix sockets, `setsid`), and every sh-based gate and
isolation script assume a Unix host. Windows compiles the workspace and runs
the portable unit tests, but cannot run the pipeline.

## Definition of done — one optimization attempt

- [ ] Hypothesis logged in ledger *before* patching
- [ ] Patch touches only allowlisted paths under `targets/<name>/`
- [ ] Upstream test suite green
- [ ] Golden replay byte-identical (or within the target's explicit FP tolerance policy)
- [ ] Differential fuzz on changed functions: no divergence (60s or 10k iterations minimum). Requires `[gates].fuzz` in the target's `target.toml` **and** the accept path (`just verdict`) — an accept is impossible without a Passed fuzz gate
- [ ] ASan + UBSan clean (machine-enforced on accept). TSan if the patch touches anything threaded — note the lane only runs when the target declares `[build].tsan`, and **no target declares one today**, so a threaded patch needs that opt-in added first
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
