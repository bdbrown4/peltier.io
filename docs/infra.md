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
| Separate-uid read-only trust layer | ✅ shipped | `scripts/agent-isolated.sh` + `just isolation-check` (below) |

## OS-level agent isolation (SPEC §10) — shipped

`scripts/agent-isolated.sh` launches one unattended attempt with the
agent process tree behind an OS boundary; `just isolation-check`
verifies the boundary from the agent's side (19 negative/positive
checks, both modes). harnessd runs OUTSIDE the boundary as the trusted
uid, serving its seven ops on a Unix socket (`harnessd --socket`) — the
agent's only write path into the repo.

Two modes (`HOTPATH_ISOLATION`):

- **`mountns` (default in this container).** The agent tree runs uid 0
  inside a private mount namespace with the repo bind-mounted
  read-only, `CAP_SYS_ADMIN`/`CAP_SYS_PTRACE` dropped from the bounding
  set. Verified refused: repo writes (EROFS), forged ledger INSERT,
  trust-binary replacement, `mount -o remount,rw`, and userns re-entry
  to undo the mount. Chosen because the nested CLI's auth is bound to
  the parent session's uid — no credentials are copied anywhere.
- **`user` (bench-metal shape).** The agent runs as unprivileged
  `hpagent`; the repo is root-owned with no world-writable paths (the
  launcher refuses to start otherwise). Verified refused: all of the
  above plus running the `verdict` binary directly. Requires the agent
  user to bring its own API credentials; a parent session's root-owned
  auth is deliberately not copied across the boundary.

Residual (both modes, this container): the agent tree can still read
the repo (by design — it needs source), reach the network through the
container's egress proxy, and signal same-uid processes in `mountns`
mode. The bench-metal `user` mode plus no-net target containers remains
the production shape.

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
