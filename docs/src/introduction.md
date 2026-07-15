# peltier.io

> Spot cooling for hot paths. A profile-guided optimization agent that
> only ships wins it can prove.

Peltier profiles real binaries and services, isolates hot paths, proposes
optimizations, and accepts a change **only** when two independent bars are
cleared:

1. **Behavioral equivalence** — the change provably preserves behavior
   (upstream tests, byte-identical golden replay, differential fuzzing,
   sanitizers) or it is discarded.
2. **Statistical significance** — the speedup's bootstrap 95% CI lower
   bound clears a threshold on trusted, calibrated hardware.

Everything else is commentary. A change without both is not a win; it is a
ledger row explaining why not.

## What it has actually done

Every number here carries its 95% confidence interval and its workload,
and every one survived the gates above. These are not projections.

| Result | Verified |
|---|---|
| tokei (Rust) — three compounding class-5 algorithmic wins | +10.7%, +3.7%, +9.9%, each CI-significant |
| tokei — link-time optimization (class 1) | +10.4%, CI [+8.5%, +12.0%] |
| cJSON (C) — number-handling rewrite (class 5) | +8.85%, CI [+7.5%, +10.0%] |
| cJSON HTTP service — p50 latency under replayed load | +6.2%, CI [+5.8%, +7.2%] |
| comrak (Rust) — mimalloc allocator swap (class 2) | +4.6%, CI [+3.3%, +5.8%], human-ruled |

Across **five phases and 34 ledger rows**, with **zero shipped false
accepts** — including two occasions where the pipeline itself over-accepted
and the audit caught it before anything shipped, each becoming a permanent
new gate.

## The differentiator

Consultants sell surveys; compilers sell flags. Peltier sells **verified
deltas with the methodology attached** — a stopwatch on pinned hardware,
a proof of equivalence, and an ROI figure whose confidence interval you
can quote to a hostile reviewer. That is the entire product, and this site
is how it works.

Start with [the thesis](./thesis.md), or jump to a
[case study](./case-studies/overview.md). To run this discipline on your own
code from your own agent, see [the peltier skill](./skill.md).
