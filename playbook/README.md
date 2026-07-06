# Optimization playbook v0

Classes are tried in **strict order** (SPEC §4) — cheapest first. Classes
1–2 are the "your 30% was a linker flag" tier and are always exhausted
before any source change. Each class documents:

- **Preconditions** — the profile signature that suggests it applies
- **Procedure** — what to actually try, cheapest variant first
- **Verification notes** — gate interactions specific to this class
- **Known failure modes** — where this class produces fake wins or real bugs

| # | Class | Touches source? | Human review? |
|---|-------|-----------------|---------------|
| 1 | [Build configuration](01-build-config.md) | no | only PGO/BOLT pipeline changes |
| 2 | [Allocator swap](02-allocator.md) | link line only | no |
| 3 | [Allocation churn](03-allocation-churn.md) | yes | no |
| 4 | [Data layout](04-data-layout.md) | yes | no |
| 5 | [Algorithmic](05-algorithmic.md) | yes | no |
| 6 | [SIMD](06-simd.md) | yes | FP-flag changes: always |
| 7 | [Concurrency](07-concurrency.md) | yes | **always** |

The agent's prompting spine (SPEC §3.5): state the hypothesis before
patching; prefer the cheapest untried class; max two iterations per
rejected hypothesis; a rejection with a clean ledger row is a successful
outcome.
