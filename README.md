# peltier.io

> Spot cooling for hot paths. A profile-guided optimization agent that
> only ships wins it can prove.

peltier.io profiles real binaries and services, isolates hot paths, proposes
optimizations, and accepts a change **only** when two independent bars are
cleared: **behavioral equivalence** (upstream tests + byte-identical golden
replay + differential fuzzing + sanitizers) and **statistical significance**
(the speedup's bootstrap 95% CI lower bound clears a threshold on
calibrated hardware). A change without both is discarded and logged.

Consultants sell surveys; compilers sell flags. Peltier sells **verified
deltas with the methodology attached** — a stopwatch on pinned hardware, a
proof of equivalence, and an ROI figure whose confidence interval survives
hostile review.

📖 **Full documentation: the [peltier.io book](https://bdbrown4.github.io/peltier.io/)**
· Design spec: [SPEC.md](SPEC.md) · Charter: [CLAUDE.md](CLAUDE.md)

## Verified results

Every number carries its 95% CI and its workload, and every one survived the
gates above. Across **five phases and 34 ledger rows, zero shipped false
accepts** — including two pipeline over-accepts the audit caught before
anything shipped, each becoming a permanent new gate.

| Target | Win | Verified |
|---|---|---|
| tokei (Rust) — LTO (class 1) | +10.4% | CI [+8.5%, +12.0%] |
| tokei — three class-5 algorithmic wins | +10.7% / +3.7% / +9.9% | each CI-significant |
| cJSON (C) — number-handling rewrite (class 5) | +8.85% | CI [+7.5%, +10.0%] |
| cJSON HTTP service — p50 latency under replay | +6.2% | CI [+5.8%, +7.2%] |
| comrak (Rust) — mimalloc (class 2) | +4.6% | CI [+3.3%, +5.8%], [human-ruled](results/rulings/phase0-comrak-002.md) |

## Layout

```
crates/            Trust layer — agent has NO write access (SPEC §10)
  bench-runner/    Interleaved A/B, bootstrap CIs, A/A calibration, service mode
  diff-test/       Equivalence gates, corpus hash-pinning, per-target spec
  ledger/          Append-only SQLite attempt ledger (mutation-refusing triggers)
  report/          ROI: speedup CI → cores → dollars, methodology inline
  verdict/         The pipeline in one command: gates → bench → ledger row
  harnessd/        The one door the agent talks through (7 JSON ops)
agent/             Untrusted proposer (Claude Agent SDK, Python)
playbook/          Optimization classes 1–7, tried strictly cheapest-first
config/            accept.toml (thresholds), pricing.toml (ROI inputs)
targets/           Vendored OSS targets — the only agent-writable path
corpora/           Hash-pinned golden-replay inputs (read-only to agent)
results/           Calibration evidence, case studies, generated ROI reports
docs/              The mdBook site (deployed to GitHub Pages)
```

## Quick start

```sh
just build / test / lint       # trust-layer workspace
just aa                        # A/A self-test — must yield a null verdict
just gates <target>            # corpus pin + upstream tests + golden replay
                               #   (fuzz needs a baseline — it runs in `just verdict`)
just verdict <t> <bin> ...     # gates + fuzz + sanitizers + bench vs pristine baseline
just report <run-id>           # ROI report from a ledger row
just isolation-check           # 19 OS-boundary checks (both modes)
```

The pipeline is **Linux/POSIX-only** at runtime (Unix sockets, `setsid`,
sh-based gates). Windows builds the workspace and runs the portable unit
tests, but cannot run the pipeline.

Full command reference and reproduction steps:
[Reproduce it yourself](https://bdbrown4.github.io/peltier.io/reproduce.html).

## How it stays honest

- The agent speaks to the trust layer through **seven read-only-plus-one-
  write JSON operations**; it has no shell, cannot write outside a target
  workspace, and cannot touch the ledger or thresholds.
- The **baseline is rebuilt from a pristine checkout** every session — a
  patch never becomes its own comparison.
- The **ledger is append-only** (SQLite triggers refuse UPDATE/DELETE);
  accepted rows are written only by the `verdict` binary after the gates.
- The agent process runs under **OS-level isolation** (read-only mount
  namespace or an unprivileged uid) — the boundary is filesystem
  permissions, not a prompt.

See [the case studies](https://bdbrown4.github.io/peltier.io/case-studies/overview.html)
— especially [the caught false-accept](https://bdbrown4.github.io/peltier.io/case-studies/comrak-false-accept.html),
where the pipeline over-accepted a leaking patch and the audit overturned
it before it shipped.

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).
