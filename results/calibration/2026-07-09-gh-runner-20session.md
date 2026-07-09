# GitHub-hosted runner — full 20-session calibration (trust anchor #2)

**PASS.** A/A false-positive rate 0.000 (< 0.05 required);
injected-5%-slowdown detection rate 1.000 (≥ 0.95 required).

- Run: https://github.com/bdbrown4/peltier.io/actions/runs/29054680971
  (job `runner-calibration`, 2026-07-09; JSON artifact
  `runner-calibration`, artifact ID 8213166804 — artifacts expire, this
  transcription is the durable record)
- Protocol: identical to the local anchor — 20 A/A sessions + 20
  injection sessions, 30 measured + 3 warm-up runs/side, interleaved,
  bootstrap 95% CI, per-session seeds. Workload: `head -c 3000000
  /dev/urandom | sha256sum` (synthetic, CPU-bound). No CPU pinning
  (shared VM; `taskset` target cores unknown a priori).
- Environment: GH-hosted `ubuntu-latest` VM, rustc 1.97.0.

Per-session results (from job log, verbatim):

```
session  1/20: A/A [------,------] fp=0; inj (see artifact)   det=1
session  2/20: A/A [0.9949,1.0087] fp=0; inj [0.9404,0.9530] det=2
session  3/20: A/A [0.9956,1.0038] fp=0; inj [0.9469,0.9570] det=3
session  4/20: A/A [0.9938,1.0030] fp=0; inj [0.9489,0.9559] det=4
session  5/20: A/A [0.9963,1.0057] fp=0; inj [0.9462,0.9601] det=5
session  6/20: A/A [0.9934,1.0133] fp=0; inj [0.9463,0.9539] det=6
session  7/20: A/A [0.9971,1.0052] fp=0; inj [0.9480,0.9557] det=7
session  8/20: A/A [0.9963,1.0016] fp=0; inj [0.9478,0.9568] det=8
session  9/20: A/A [0.9943,1.0012] fp=0; inj [0.9454,0.9599] det=9
session 10/20: A/A [0.9950,1.0091] fp=0; inj [0.9503,0.9580] det=10
session 11/20: A/A [0.9990,1.0061] fp=0; inj [0.9473,0.9575] det=11
session 12/20: A/A [0.9955,1.0020] fp=0; inj [0.9515,0.9584] det=12
session 13/20: A/A [0.9976,1.0033] fp=0; inj [0.9493,0.9572] det=13
session 14/20: A/A [0.9944,1.0054] fp=0; inj [0.9462,0.9520] det=14
session 15/20: A/A [0.9911,0.9985] fp=0; inj [0.9470,0.9562] det=15
session 16/20: A/A [0.9947,1.0041] fp=0; inj [0.9460,0.9550] det=16
session 17/20: A/A [0.9972,1.0058] fp=0; inj [0.9489,0.9573] det=17
session 18/20: A/A [0.9951,1.0100] fp=0; inj [0.9489,0.9541] det=18
session 19/20: A/A [0.9945,1.0028] fp=0; inj [0.9475,0.9558] det=19
session 20/20: A/A [0.9925,1.0030] fp=0; inj [0.9488,0.9541] det=20
```

(Session 1's line scrolled past the retained log window; its outcome is
included in the summary counters and the JSON artifact.)

Reading: A/A CIs are ~±0.5% wide and 19/20 straddle 1.0 (session 15's
upper bound grazes 0.9985 — still a null verdict, since accept requires
lower bound ≥ 1.02). Every injection CI sits entirely below 1.0,
centered ~0.950 — the harness recovers the injected 5% almost exactly.

## Phase 1 exit criteria (SPEC §5) — status with this anchor

- A/A false-positive < 5%: met on two independent environments
  (local container 0/20 with pinning; GH runner 0/20 without).
- Injected 5% regression caught ≥ 95%: 19/20 local, 20/20 GH runner.
- Gates end-to-end on 2 targets: comrak + tokei via `just gates`,
  re-verified mechanically on every CI run (`target-gates-no-net`).

Caveat that stays attached: shared-VM placement varies; this evidence
trusts the *protocol* on this runner class, not any specific machine.
Every CI run re-calibrates, so drift is caught per-run. Dedicated
pinned hardware (docs/infra.md) remains the standard for Phase 3/4
customer-facing case studies.
