# Roadmap & scope

## Done — the core loop, end to end

| Phase | Scope | Result |
|---|---|---|
| **0** | Manual dry run, two Rust targets | tokei +10.4% verified; methodology written |
| **1** | Trust layer (bench-runner, diff-test, ledger) | A/A <5% FP, injection ≥95%, gates on 2 targets; calibrated free on GitHub runners |
| **2** | Unattended agent loop | 20 audited attempts, 3 verified wins, **0 shipped false accepts**, OS-isolated |
| **3** | Playbook + proof, C/C++ | cross-language cJSON win (+8.85%), 4 case studies, 6 playbook classes |
| **4** | Services & scale | cJSON service +6.2% p50 under CO-correct replay, mechanical ROI report |

The `profile → hypothesize → patch → gated-verdict → ROI` loop runs
unattended, OS-isolated, across **Rust and C**, **batch and service**, from
a pinned corpus to a dollar figure — with zero shipped false accepts across
all phases, and the two pipeline over-accepts both caught by audit and
turned into permanent gates.

## Frontier — the research forks (SPEC §13)

Deliberately out of the core roadmap, these are where the largest gaps
live:

- **GPU kernel lane.** Triton/CUDA targets where 2–10× gaps are common;
  correctness via reference-kernel differential testing; reward = measured
  kernel time. The trust machinery (differential equivalence, interleaved
  timing, append-only ledger) transfers directly; the profiler and bench
  become kernel-time-aware, and it needs GPU hardware to run.
- **Learned optimization policies.** The ledger is a growing dataset of
  `(profile signature, class, hypothesis) → verdict`. That is training data
  for a policy that *ranks* which class to try first on a new hotspot,
  turning the fixed cheapest-first ordering into a learned prior. Prior art:
  MLGO, AlphaDev, Meta's LLM Compiler; ProGraML-style IR graphs as the
  program representation.

## The real target

The end goal is not vendored OSS case studies — it is a **real production
system**. The same gates that matter for a JSON parser matter far more for
a live CMS: an equivalence failure there is a customer-facing bug, and the
no-network sandbox and append-only audit trail are exactly what make it
safe to point an optimization agent at code that a business depends on.
