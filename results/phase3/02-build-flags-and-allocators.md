# Case study 2 — cheap wins first: build flags and allocators

**The playbook's ordering principle (SPEC §3.3, CLAUDE.md
non-negotiable #3): build flags → LTO → PGO → allocator swap → *then*
code changes.** The cheapest classes are tried first because they carry
the lowest cost-per-attempt and the lowest risk. Two of them produced
Phase-0 results that frame the whole project's discipline.

## Class 1 — link-time optimization (tokei) — ACCEPTED

`phase0-tokei-003`: set `lto = true` (fat LTO, up from `thin`) and
`codegen-units = 1` in tokei's release profile.

- **Speedup: 1.104 median, 95% CI [1.085, 1.120]** vs the pristine
  `thin`-LTO baseline.
- Byte-identical golden replay; no source change, so equivalence is by
  construction (same code, different codegen). Auto-accept tier.
- A pure build-config win worth **+10%** — the cheapest possible class,
  and on this target the single largest individual increment.

The lesson the ledger enforces: you do not go hand-optimize a hot loop
before you have tried turning on the optimizer's own switches.

## Class 2 — allocator swap (comrak) — NEEDS-HUMAN-REVIEW

`phase0-comrak-002`: swap the global allocator to
[mimalloc](https://github.com/microsoft/mimalloc).

- **Speedup: 1.046 median, 95% CI [1.033, 1.058]** — a real, CI-clean
  +4.6% on comrak's Pro Git render.
- Byte-identical golden replay. And yet the verdict is **not
  `accepted`** — it is **`needs-human-review`**.

Why: an allocator swap changes the process's memory behavior globally.
It is exactly the class of change SPEC §8 routes to a human — not
because this instance is wrong, but because "the benchmark got faster"
is not sufficient evidence that a global allocator change is safe under
every workload, fragmentation pattern, and concurrency scenario the
target will meet in production. The pipeline measured a real win and
then **refused to auto-ship it**, tagging it for a human ruling instead.

**Human ruling (`results/rulings/phase0-comrak-002.md`): ACCEPTED,
scoped to the comrak CLI single-stream markdown-render workload** — a
real win for the measured use, deliberately *not* a blanket "always use
mimalloc" mandate. The tier forced the judgment; the judgment is bounded
and written down. The ledger row stays `needs-human-review` (append-only
records what the pipeline decided); the ruling is the human layer on
top.

## What the pair demonstrates

The cheap classes are not lesser — class 1 produced this project's
biggest single win. But "cheap to try" is not "safe to auto-accept."
LTO is deterministic codegen: accept it. An allocator swap is a global
behavioral change: measure it, prove the speedup, and *still* hand it to
a human. The tier is decided by the *nature* of the change, never by the
size of the number — the same rule that later caught a leaking teardown
patch the bench loved (case study 3).
