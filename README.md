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

**Phase 0 complete** (see SPEC §5). Five attempts in the ledger across
two pinned targets (comrak 0.53.0, tokei 14.0.0), A/A-calibrated,
every number with CI and workload:

- **tokei +10.4% median [95% CI +8.5%, +12.0%]** — read-path buffer
  zeroing eliminated + fat LTO, byte-identical, 4,332-iteration
  diff-fuzz clean, sanitizers clean, auto-accepted
  (`results/phase0/case-study-tokei.md`). Meets the Phase 0 exit bar.
- comrak +4.6% [+3.3%, +5.8%] — mimalloc swap, `needs-human-review`
  on an LSan reachability flag (`results/phase0/case-study-comrak.md`).
- Two null/sub-threshold results correctly rejected and logged.

Next: Phase 1 trust-layer hardening (containers, perf, automated
calibration, gate orchestration, agent IPC).

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
