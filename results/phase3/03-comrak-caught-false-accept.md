# Case study 3 — the caught false-accept (comrak, class 3)

**This is the most important case study in the project, because it is
the one where the machine got it wrong and the process caught it.**

## What happened

`phase2-comrak-010`: the unattended agent, reading comrak's cache-sim
profile, saw that AST-arena teardown at process exit —
`drop_in_place::<Vec<Node<RefCell<Ast>>>>` plus its share of the glibc
free path — cost real instructions and cache misses in a
run-to-completion CLI. Its hypothesis: comrak already ends in
`process::exit`, so the OS reclaims memory in bulk anyway; wrap the arena
in `std::mem::ManuallyDrop` to skip the per-node drop walk, and add a
`std::hint::black_box(&arena)` before the scope closes so the leak stays
reachable and a leak checker won't flag it.

The pipeline ran it:

- Upstream tests: **PASS**
- Golden replay: **byte-identical**
- Interleaved A/B: **+10.7% median, 95% CI [+7.9%, +12.1%]**
- Verdict: **`accepted`**

A clean, large, CI-significant win. And it was **wrong**.

## The audit overturned it

Phase 2 runs a **100% human audit** of every accepted win (SPEC §10).
The audit ran the sanitizers the pipeline had not: LeakSanitizer flagged
a **384-byte arena chunk as unreachable at exit**. The agent's
`black_box(&arena)` keeps the arena live only until its scope *closes* —
but `process::exit` runs *after* that scope, so at the moment of exit the
arena is genuinely unreachable. A real leak. The agent's own hypothesis
had even named the risk ("if the sanitizer still flags the leak,
rejected-gates is the honest outcome") — the pipeline simply had no
sanitizer gate to trip, because in Phase 2 ASan/LSan were a per-attempt
*manual* step.

Called per SPEC §10: a **pipeline false accept**, caught by audit
**before anything shipped**. The win was never banked. Shipped false
accepts across the entire project remain **zero**.

## The fix made the machine enforce the tier

The corrective action was not "be more careful." It was structural:
`verdict` now **builds the patched tree with ASan+LSan (ASan+UBSan for
C/C++) and runs the pinned workload on every would-be accept**, capping
any sanitizer-flagged win at `needs-human-review`. The tier rule SPEC §8
always specified is now enforced by the machine, not the auditor. The
very next Rust accept (`phase2-tokei-008`) and the first C accept
(`phase3-cjson-002`) both passed through this automated gate.

The 10k-input differential fuzz on the overturned patch found **0
divergences** — the output equivalence was genuinely real. Only the
teardown-leak tier was wrong. That is the subtle failure mode the layered
gates exist for: a change can be behaviorally equivalent on output and
still unacceptable on a dimension the bench and the golden replay can't
see.

## What it demonstrates

The product is not "an agent that finds speedups." The product is
**trust** — and trust is demonstrated most by the case where the
optimistic path failed and the structural controls held. An agent
graded only by accept-rate would have banked this win. The ledger is
append-only, so the `accepted` row still stands, immutable — with this
document as the permanent corrective record beside it.
