# The playbook

Optimizations are tried in a fixed order — **cheapest and safest first** —
because cost-per-attempt and risk both rise as you go down the list, and
the ledger stops you from re-grinding a class that already has a verdict.

| # | Class | Mechanism | Verified on |
|---|---|---|---|
| 1 | Build config | LTO, codegen-units, target-cpu | tokei ✅ +10.4% |
| 2 | Allocator | Global allocator swap (mimalloc) | comrak ✅ +4.6% (human-ruled) |
| 3 | Allocation churn | Remove per-item malloc/free | tokei ✅, cJSON ✅ (+ one overturned) |
| 4 | Data layout | Make the cache hierarchy fit the access pattern | comrak (attempted) |
| 5 | Algorithmic | Eliminate redundant work | tokei ×3 ✅, cJSON ✅ |
| 6 | SIMD | Autovectorization enablement | comrak, tokei (attempted) |
| 7 | Concurrency | Parallelism — always `needs-human-review` | — |

Six classes were exercised across the case studies, with verified wins on
classes 1, 2, 3, and 5 across **both a Rust and a C target**.

## Why the ordering is load-bearing

The cheapest class produced the project's single largest individual win:
LTO on tokei, **+10.4%**, a pure build-config change with no source edit.
The lesson the ledger enforces is the one every performance engineer learns
the hard way — *you do not hand-optimize a hot loop before you have turned
on the optimizer's own switches.* An agent that jumped straight to SIMD
would have missed a bigger, safer, free win sitting one config flag away.

Each class entry (`playbook/0N-*.md`) carries its **preconditions** (the
profile signature that justifies trying it), its **procedure**, its
**verification** requirements, and its **known failure modes**. The agent
reads the profile, matches the signature, and picks the cheapest class
whose preconditions are met and that isn't already spent on this hotspot.
