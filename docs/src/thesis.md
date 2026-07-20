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

## The detective and the court

> The optimizer is a **Bayesian detective**. The trust layer is a
> **frequentist court**.

That sentence is the whole design. The two halves run on opposite
statistical philosophies, deliberately:

```
     BAYESIAN DETECTIVE                        FREQUENTIST COURT
     the optimizer: agent loop,                the trust layer: gates,
     playbook prior, policy ranking            calibrated bench, verdict

   ┌───────────────────────────┐            ┌────────────────────────────┐
   │ generate hypotheses       │            │ prove equivalence first    │
   │ prioritize the docket     │  proposes  │ calibrate the instrument   │
   │ estimate likely payoff    │ ─────────▶ │ measure, interleaved A/B   │
   └───────────────────────────┘            │ reject noise (CI lower     │
               ▲                            │ bound vs. threshold)       │
               │                            └─────────────┬──────────────┘
               │                                          ▼
               │        evidence flows back            verdict
               └───── ledger row + explain ◀──────────────┘
```

The **detective** reasons like a Bayesian: it holds a prior over where wins
live (the [playbook's](./playbook.md) cheapest-first ordering), updates it
from accumulated evidence (the append-only ledger — failures included,
because failures are the most informative updates), ranks its next bets
(`crates/policy`, by the Wilson *lower* bound of each class's proven win
rate — even the learning layer bets pessimistically), and reads the
[post-verdict diagnosis](./explain.md) so the next hypothesis is sharper
than the last. Its "estimate likely payoff" is a belief, and stays one:
dollar figures are minted only by the court's
[mechanical report](./roi.md), only from accepted rows.

The **court** reasons like a frequentist, because its audience is a hostile
reviewer who shares none of your priors. Its verdicts are decision
procedures with *measured* long-run error rates: the A/A calibration bounds
the false-positive rate and the injected-regression test bounds the power —
per machine, empirically, before any real measurement is believed. The
accept rule is a conservative bound, not a posterior; there is no prior
anywhere on the accept path, so there is nothing to argue about except the
evidence.

The return edge is what makes the detective Bayesian at all — and note its
direction. **Evidence flows back; authority never does.** The invariant
underneath the whole system:

> **Priors may steer where you look. They may never touch what you
> conclude.**

A wrong prior in the detective costs compute — a few wasted attempts, each
honestly ledgered. A prior in the court could cost a false claim, the one
unrecoverable failure for a project whose product is trust. The gates are
what make speculative, prior-driven exploration *safe*: the detective can
believe anything it likes, because nothing it believes survives into a
number.
