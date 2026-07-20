# Explain: why a row won or lost

Verdicts say *what*. `just explain <run-id>` says *why* — an advisory
diagnosis of one ledger row, built so it can never contaminate the thing it
explains:

- **Ledger-only inputs, deterministic.** It reads exactly one row and nothing
  else — no config, no clock, no re-benching. The same row produces
  byte-identical text, so a regenerated explanation is checkable.
- **Strictly off the accept path.** It runs after the verdict is written;
  nothing it says feeds gates, the bench, or the ledger.
- **Record vs. inference is explicit.** Restatements of the machine record are
  printed plain; anything beyond that is prefixed `inference:` — so an
  explanation can never be quoted as if the machine measured it.
- **The narrative must agree with the verdict.** An accepted row is described
  by its CI *lower bound* (the defensible claim). A rejected bench is
  classified honestly: measured regression, null result, or
  real-but-below-bar. A `needs-human-review` row names the recorded cap and
  says plainly that the numbers were not the deciding factor.

## Real output, real rows

The shipped cJSON win ([`phase3-cjson-002`](./case-studies/cjson.md)):

```
verdict (machine record): accepted
bench: baseline median 0.4263s, candidate median 0.3916s — speedup 1.0885 (+8.8%), 95% CI [1.0745, 1.1001]
workload: 5.6 MB synthetic JSON (20k records: nested objects/arrays, escaped/unicode strings, ints, dense floats), parsed+serialized 2x, single thread
inference: the defensible claim is the lower bound — "at least +7.4%" on this workload, not the median +8.8%
```

The [caught false-accept](./case-studies/comrak-false-accept.md)
(`phase2-comrak-010`) — explain flags it instead of laundering it:

```
inference: the defensible claim is the lower bound — "at least +7.9%" on this workload, not the median +10.7%
CAUTION: accepted with sanitizers_clean=false — this row predates the machine-enforced
sanitizer lane; under current doctrine it could not be machine-accepted. See results/rulings/
```

The kernel-lane row (`phase5-matmul-opt`, `needs-human-review`):

```
verdict (machine record): needs-human-review
bench: … speedup 3.2301 (+223.0%), 95% CI [3.1600, 3.2594]
inference: the numbers were not the cap — the review routing below decided this verdict
```

That last line is the point: a 3.2× speedup with a tight interval still does
not argue with the review routing. Explanations restate the record and reason
*from* it — they do not re-litigate it.

## Why this exists

The loop's compounding value is not any single verdict — it is that the next
hypothesis is better than the last. A rejection that says only
`rejected-bench` teaches nothing; one that says "real improvement, below the
ship bar" or "indistinguishable from no change — a null result, not a small
win" or "a measured regression, report it as one" points at three different
next moves. Explain is that teaching layer, kept deliberately outside the
measurement it learns from — see SPEC §3.7 for the invariants, including the
one that keeps it out of the agent-facing surface: better hypotheses must
never become a feedback channel into the accept path.

Rows written from 2026-07-20 on also record the accept bar in force for that
run (`accept_threshold` in the env fingerprint), so an explanation never has
to guess a historical threshold from today's config — for older rows it says
so explicitly instead.
