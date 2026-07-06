# peltier.io

Spot cooling for hot paths. A profile-guided optimization agent that only ships wins it can prove.

hotpath finds the 10–90% left on the floor between "the compiler" and
"hire a perf consultant," with two properties no consultant offers:
**verified equivalence** (a change provably preserves behavior or is
discarded) and **self-verifying ROI** (the value claim is a stopwatch on
trusted infrastructure, not a survey).

Full design: **[SPEC.md](SPEC.md)**.

## Layout

```
crates/            Trust layer (agent has NO write access — SPEC §10)
  bench-runner/    Interleaved A/B timing, bootstrap CIs, A/A calibration
  diff-test/       Equivalence gates, corpus hash-pinning, per-target policy
  ledger/          Append-only SQLite attempt ledger (enforced by triggers)
  report/          ROI math: speedup CI → cores → dollars, caveats included
agent/             Untrusted proposer (Claude Agent SDK, Python)
playbook/          Optimization classes 1–7, tried strictly cheapest-first
config/            accept.toml (thresholds), pricing.toml (ROI inputs)
targets/           Per-target checkouts — the only agent-writable path
corpora/           Hash-pinned golden-replay inputs (read-only to agent)
results/           Calibration evidence, per-engagement outputs
```

## Quick start

```sh
cargo test --workspace          # trust-layer unit tests
just aa                         # A/A self-test: must yield a null verdict
just compare "cmd-a" "cmd-b"    # interleaved A/B with bootstrap CI
```

## Status

Phase 0 dry run in progress (see SPEC §5 for the phase plan and exit
criteria). Two targets vendored and pinned (comrak 0.53.0, tokei
14.0.0); A/A calibration passed on both workloads; three attempts in
the ledger — best so far **+4.6% [95% CI +3.3%, +5.8%]** on comrak via
allocator swap, `needs-human-review` (sanitizer flag, analysis in
`results/phase0/case-study-comrak.md`). Exit criteria (≥10% verified
win) not yet met.

Done:
- Workspace with the four trust-layer crates; core statistics
  (interleaved scheduling, bootstrap ratio CIs, CI-lower-bound accept
  rule), append-only ledger schema with mutation-refusing triggers,
  corpus hash-pin verifier, equivalence-policy parser, ROI formulas —
  all unit-tested.
- bench-runner CLI: whole-program `compare` and `aa` modes with env
  fingerprinting.
- Playbook classes 1–7; agent tool contract and prompting spine.

Next (Phase 1 exit criteria):
- `perf stat` counters + max RSS + RAPL capture per run.
- Automated A/A calibration sessions and regression-injection self-test
  recorded to `results/calibration/` (<5% FP, ≥95% detection).
- diff-test orchestration end-to-end on two real targets; harness IPC
  for the agent tool surface.

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).
