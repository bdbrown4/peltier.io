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

## Current phase

**Phase 2 in progress — agent loop.** The unattended profile→hypothesize→patch→gated-verdict loop runs on comrak via the Claude Agent SDK (`agent/hotpath_agent/loop.py`, model claude-fable-5) over a stdio MCP server wrapping the seven harness tools. 3 audited attempts (phase2-comrak-001/002/003), all honest rejections, **0 false accepts**; the agent twice refused to fabricate an unreadable verdict (results/phase2/case-study-agent-loop.md). Shipped: async run_verdict + pollable read_verdict, windowed read_target_source. Exit-criteria gaps: an auto-accepted win (needs richer profiling to unlock more classes), and OS-level process/user isolation for the agent's tool boundary (SPEC §10; the SDK allow/deny layer is defense-in-depth only — see docs/infra.md). (Update this line as phases complete; exit criteria in SPEC.md §5.)

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
