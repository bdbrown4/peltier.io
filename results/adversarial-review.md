# Adversarial review — whole package

A hostile pass over the entire project: every headline claim re-checked
against the ledger, the trust boundary attacked directly, the newest
code scrutinized hardest, and the statistics re-validated. Two real
defects found and fixed; the load-bearing guarantees held.

## Findings

### 1. FIXED — the ROI generator minted a clean pitch for an overturned patch
The report generator trusted the ledger's `verdict` field. `phase2-comrak-010`
is immutably `accepted` (it predates the machine-enforced sanitizer gate;
the audit overturned it for a LeakSanitizer leak and it was never banked).
`report --run-id phase2-comrak-010` therefore produced a clean
**$16,886/year** ROI report for a memory-leaking patch — exactly the kind
of self-contradiction a hostile reviewer would seize on.

**Fix:** the report now treats an `accepted` row that is not
`sanitizers_clean` as **not shippable** and prints a "Not shipped" warning
before any dollar figure. Because the pipeline now enforces ASan/UBSan on
every accept, all legitimate wins are `sanitizers_clean: true`; only the
historical overturned row is `accepted + false`, so the rule cleanly
separates them. Regression-checked: `phase3-cjson-002` still renders clean.

### 2. FIXED — load generator could panic on degenerate inputs
`bench-runner service --rate 0` (or `--count 0`) reached
`Duration::from_secs_f64(1.0/0.0)` → panic. **Fix:** `run_session` now
rejects a non-finite/non-positive rate and zero count/workers with a clear
error.

## Attacks that were correctly refused (no change needed)

- **Ledger append-only:** `UPDATE`/`DELETE` on a copy both refused by the
  SQLite triggers.
- **Patch boundary:** `propose_patch` refused `../` traversal and `.git`
  access; `read_target_source` refused traversal. A patch can only touch
  `targets/<t>/workspace`.
- **Accepted-row invariant:** all 10 `accepted` rows have green upstream
  tests + golden replay and a bootstrap CI lower bound ≥ 1.02.
- **Statistics:** every batch and service A/A calibration is a null result
  (0 false positives); every injected-5% regression is detected (≥95%).
  The service load generator is coordinated-omission-correct and its
  injection test resolves a known 5% latency regression 10/10.

## Verification run

`cargo test --workspace` (all green), `clippy -D warnings` (clean),
`cargo fmt --check` (clean). Every case-study and CLAUDE.md figure
re-checked against `results/ledger.sqlite` and the calibrated bench —
consistent, with the compound tokei figure stated as an approximation
(CIs don't multiply) and p99 service latency explicitly not claimed.

## Standing (not defects)

- `phase2-comrak-010` remains `accepted` in the ledger by design — the
  ledger is append-only and records what happened; the overturn lives in
  the case study and is now enforced by the report generator too.
- `phase0-comrak-002` (mimalloc) is `needs-human-review`, awaiting a ruling
  (see `results/rulings/`).
