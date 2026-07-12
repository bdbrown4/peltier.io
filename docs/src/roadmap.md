# Roadmap & scope

## Done — the core loop, end to end

| Phase | Scope | Result |
|---|---|---|
| **0** | Manual dry run, two Rust targets | tokei +10.4% verified; methodology written |
| **1** | Trust layer (bench-runner, diff-test, ledger) | A/A <5% FP, injection ≥95%, gates on 2 targets; calibrated free on GitHub runners |
| **2** | Unattended agent loop | 20 audited attempts, 3 verified wins, **0 shipped false accepts**, OS-isolated |
| **3** | Playbook + proof, C/C++ | cross-language cJSON win (+8.85%), 4 case studies, 6 playbook classes |
| **4** | Services & scale | cJSON service +6.2% p50 under CO-correct replay, mechanical ROI report |
| **5** | Research forks (SPEC §13) | FP-tolerance kernel lane (matmul 3.23×, needs-human-review) + learned class-selection prior from the ledger |

The `profile → hypothesize → patch → gated-verdict → ROI` loop runs
unattended, OS-isolated, across **Rust and C**, **batch and service**, from
a pinned corpus to a dollar figure — with zero shipped false accepts across
all phases, and the two pipeline over-accepts both caught by audit and
turned into permanent gates.

## Phase 5 — the research forks (SPEC §13)

Deliberately out of the core roadmap, these are the two hardest cases the
spec named — both now built and exercised, both routed to human review.
Full write-up: [Research forks](./research-forks.md).

- **Kernel lane — done as a CPU demonstration.** A matmul optimization that
  reorders floating-point accumulation (transpose-B + eight-accumulator ILP)
  runs 3.23× faster and is *impossible* to gate byte-identically. It slots
  into the harness by swapping the equivalence policy for a declared
  FP-tolerance (`abs + rel·|ref|`) — which still catches a genuine wrong
  result — and routes to `needs-human-review` because using the tolerance
  tier at all is a §8 signal. This is the GPU lane in miniature:
  reference-kernel differential testing, interleaved timing, ledger row.
  Only the timer and the hardware change; **the actual GPU run needs GPU
  hardware**, absent here.
- **Learned optimization policies — done.** `crates/policy` reads the ledger
  and ranks optimization classes by the Wilson lower bound of their
  shippable-win rate, turning the fixed cheapest-first ordering into a
  learned prior (advisory only — the gates still decide). Prior art: MLGO,
  AlphaDev, Meta's LLM Compiler.

## The real target

The end goal is not vendored OSS case studies — it is a **real production
system**. The same gates that matter for a JSON parser matter far more for
a live CMS: an equivalence failure there is a customer-facing bug, and the
no-network sandbox and append-only audit trail are exactly what make it
safe to point an optimization agent at code that a business depends on.
