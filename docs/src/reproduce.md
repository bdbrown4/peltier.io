# Reproduce it yourself

Nothing here is a screenshot. Every number regenerates from a pinned corpus
through the same commands.

## Prerequisites

- Rust (stable) + `just`; `clang`/`cmake` for the C target; `valgrind` for
  callgrind profiling. Nightly Rust for the sanitizer gate.
- Targets are vendored fresh (their `workspace/` is gitignored); each
  `targets/<name>/target.toml` has the `[fetch]` recipe.

## The commands

```
just build / test / lint       # trust-layer workspace
just aa [cmd]                  # A/A self-test — must be a null verdict
just calibrate <cmd> <out>     # A/A false-positive + injection detection
just gates <target>            # corpus pin + upstream tests + golden replay
just verdict <t> <bin> ...     # gates + bench vs pristine baseline + ledger row
just report <run-id>           # ROI report from a ledger row
```

Service mode (Phase 4):

```
just service <baseline-bin> <candidate-bin> <doc>     # interleaved p50/p99 CIs
just service-calibrate <server> <doc> <out>           # latency A/A + injection
just report <run-id> --service-json <json>            # latency + ROI report
```

The agent loop, behind OS isolation:

```
just isolation-check                    # 19 boundary checks, both modes
scripts/agent-isolated.sh <target> <run-id>   # one unattended attempt, confined
```

## Auditing a result

Every accepted win can be re-audited independently:

```
python3 scripts/audit-attempt.py <run-id>        # mechanical re-check
python3 scripts/diff-fuzz-<target>.py <dir> 10000 <candidate-bin>   # 10k fuzz
```

The ledger itself is the record: `results/ledger.sqlite`, append-only,
every attempt (win or loss) with its hypothesis, gates, bench evidence, and
verdict.
