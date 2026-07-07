# Bench infrastructure — what Phase 1 still needs from hardware

The trust layer is code-complete for Phase 1 (calibration, gates,
verdict, agent IPC), but SPEC-grade *bench numbers* need hardware this
repo's CI cannot provide. Status by requirement:

| Requirement (SPEC §3.1/§10) | GH-hosted runner | This repo's answer |
|---|---|---|
| No-network target execution | ✅ docker `--network=none` | `target-gates-no-net` CI job |
| Gates run mechanically | ✅ | `just gates` in CI |
| Environment noise measured | ✅ (measured, likely fails bars) | `runner-calibration` CI job, artifact per run |
| Profiling | ⚠️ software `cpu-clock` only (VM PMU usually absent) | `perf-probe` CI job reports what works |
| Pinned cores, governor, SMT/turbo control | ❌ shared VM | **bench-metal box (below)** |
| Separate-uid read-only trust layer | ❌ | bench-metal box setup |

## The bench-metal box (human setup, ~15 min + account)

Any dedicated bare-metal server works; the budget default is a Hetzner
AX-line dedicated server (other options: OVH/Equinix Metal/AWS
`*.metal`). Setup:

1. Ubuntu LTS; `apt install linux-tools-$(uname -r) docker.io git build-essential`.
2. Measurement discipline: `cpupower frequency-set -g performance`;
   disable SMT (`echo off > /sys/devices/system/cpu/smt/control`) and
   turbo; reserve two cores via `isolcpus=` kernel arg; set
   `pin_prefix` in `config/accept.toml` to those cores.
3. Register as a GitHub self-hosted runner with label `bench-metal`
   (repo Settings → Actions → Runners). Run the runner as an
   unprivileged user; make `crates/`, `config/`, `corpora/` owned by a
   different user (this is the separate-uid enforcement from SPEC §10).
4. First act: `just calibrate <real workload> results/calibration/bench-metal-01.json`
   — the box is only trusted after its own A/A + injection evidence is
   committed. Then flip verdict bench jobs to `runs-on: bench-metal`.

Until that box exists, verdict-grade benches keep running wherever a
human has verified calibration evidence first (as done for this
container in `results/calibration/`).
