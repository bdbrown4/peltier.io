# The thesis: trust is the product

Performance work has a credibility problem. "We made it 15% faster" is
almost always some mix of a noisy benchmark, a workload that flatters the
change, and a behavioral difference nobody checked. The number is real to
the person who measured it and worthless to everyone downstream.

Peltier is built on the opposite premise: **a performance claim is a
liability until it is proven, and the proof ships with the number.**

## Three non-negotiables

1. **No unverified performance claims.** A change without passing
   equivalence gates *and* a statistically significant benchmark delta
   does not exist. It is discarded and logged.
2. **Measure before optimizing.** No patch is proposed until profile data
   identifies the hot path. A single benchmark run is never trusted.
3. **Cheap wins first.** Build flags → LTO → PGO → allocator swap → *then*
   code changes. The ledger prevents re-attempting a dead end.

## What "proven" means here

A win must clear **both** of two independent bars, because either one alone
is gameable:

- **Equivalence** stops "optimizations" that are actually behavior changes.
  A JSON parser that's faster because it stopped validating numbers is not
  faster; it's broken. Byte-identical golden replay and differential
  fuzzing catch this.
- **Significance** stops noise from masquerading as a win. The bar is not
  "the median improved" — it is "the bootstrap 95% CI *lower bound* clears
  the threshold," measured by interleaving baseline and candidate on pinned
  hardware that has passed its own A/A calibration.

A real +5.7% median that couldn't clear the significance bar (its CI lower
bound was +0.2%) was **rejected** — and that rejection is in the ledger, because
a rejection is a complete, valid outcome.

## Why an agent

The loop — profile, hypothesize, patch, gate, measure, record — is
mechanical and tedious, and the temptation to fudge a number is exactly
what a tired human succumbs to. An agent proposes; the *trust layer*
disposes. The agent cannot write the ledger, cannot move the baseline,
cannot touch the thresholds. It can only propose a patch and ask the
harness to judge it. The [architecture](./architecture.md) is designed so
that even a compromised or adversarial proposer cannot manufacture a false
win.
