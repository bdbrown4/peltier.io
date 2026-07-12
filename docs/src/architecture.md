# Architecture

Peltier is split into a **trust layer** (Rust, the agent cannot write it)
and an **untrusted proposer** (the agent). The split is structural, not
advisory: the guarantees hold even if the proposer is adversarial.

```
crates/            Trust layer — agent has NO write access (SPEC §10)
  bench-runner/    Interleaved A/B timing, bootstrap CIs, A/A calibration,
                   service-mode latency (coordinated-omission correct)
  diff-test/       Equivalence gates, corpus hash-pinning, per-target spec
  ledger/          Append-only SQLite attempt ledger (enforced by triggers)
  report/          ROI: speedup CI → cores → dollars, methodology inline
  verdict/         The pipeline in one command: gates → bench → ledger row
  harnessd/        The one door the agent talks through (7 JSON ops)
agent/             Untrusted proposer (Claude Agent SDK, Python)
playbook/          Optimization classes 1–7, tried strictly cheapest-first
config/            accept.toml (thresholds), pricing.toml (ROI inputs)
targets/           Vendored OSS targets — the only agent-writable path
corpora/           Hash-pinned golden-replay inputs (read-only to agent)
results/           Calibration evidence, case studies, generated reports
```

## The seven-tool boundary

The agent never runs a shell, never writes a file, never touches the
ledger directly. It speaks to `harnessd` — the only door — through exactly
seven JSON operations:

| Tool | What it can do |
|---|---|
| `read_profile` | Read a target's ranked hotspots (read-only) |
| `read_ledger` | Read prior attempts — the anti-double-attempt memory |
| `read_playbook` | Read an optimization class's preconditions/procedure |
| `read_target_source` | Read a workspace file at the pinned commit (windowed) |
| `propose_patch` | Submit a unified diff — path-allowlisted, then `git apply` |
| `run_verdict` | Launch the gate+bench pipeline (detached) |
| `read_verdict` | Poll the append-only ledger for the result |

Note what is *absent*: no write outside `targets/<name>/workspace`, no
shell, no way to run the bench or write the ledger except through the
`verdict` binary, which only records a pass after the full gated pipeline.

## The data flow of one attempt

```
profile ─▶ agent hypothesizes ─▶ propose_patch ─▶ [path allowlist + git apply]
                                                          │
   ledger row ◀─ verdict ◀─ bench (interleaved A/B) ◀─ gates (tests, golden,
   (append-only)             vs pristine-rebuilt         fuzz, sanitizers)
                             baseline
```

The baseline is **rebuilt from a pristine checkout every session** — never
from the agent's workspace — so a patch cannot poison its own comparison.
The verdict is decided by the CI-lower-bound rule and written to an
append-only ledger the agent cannot mutate.

The next chapters walk each stage: [measurement](./measurement.md),
[equivalence gates](./equivalence.md), [the agent loop](./agent-loop.md),
and the [isolation](./isolation.md) that makes the boundary load-bearing.
