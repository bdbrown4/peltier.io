# attempt — the full loop, inside a peltier checkout

Profile → hypothesize → patch → gated verdict → ledger row. This mode only
applies when you are working **inside a peltier checkout** on a target that has
a `targets/<name>/target.toml`. Everywhere else, use
[verify.md](verify.md).

Requires Linux (harnessd uses Unix sockets and `setsid`; every gate shells
through `sh`).

## Before you touch anything

**Read the ledger first.** It records every attempt ever made on this target,
including the failures — which is exactly what stops you re-running a dead
end:

    cargo run -p policy          # ranks playbook classes by their evidence

`policy` reads the ledger and ranks optimization classes by the Wilson lower
bound of their shippable-win rate. It is **advisory** — a learned prior over
the fixed cheapest-first order. The gates still decide every verdict.

**Then respect the order.** Cheapest wins first: build flags → LTO → PGO →
allocator swap → *then* code changes. Do not hand-write SIMD before trying
`-O3`. The playbook (`playbook/`) has the preconditions and known risks for
each class.

## The boundary you may not cross

The agent may modify **only** allowlisted paths under `targets/<name>/`.
`crates/`, `config/`, `corpora/`, and the upstream test suites are read-only:
the thing being measured does not get to grade itself. `harnessd` enforces
this on every proposed diff (path allowlist over every diff header, including
`rename`/`copy` — a patch that tries to escape is rejected, not sanitized).

## Log the hypothesis *before* patching

A hypothesis written after seeing the numbers is not a hypothesis. State what
you think is slow, why, and what you expect the change to buy — then patch.

## Run the gates

    just gates <target>

Corpus pin → test-suite pin (when `corpora/<t>/TESTSUITE.sha256` exists) →
upstream tests → golden replay. Note what this **does not** do: differential
fuzz needs a pristine baseline to differ against, so it reports `Skipped` here
and runs for real on the accept path below. `just gates` is a fast equivalence
check, not a fuzz run — do not report it as one.

## The verdict — the only thing that can accept a change

    just verdict <target> <candidate-bin> <run-id> <class> "<hypothesis>" "<hotspot>" \
        --patch-file <diff>

This rebuilds a **pristine baseline** from the pinned upstream commit (not
whatever is lying around in your tree), runs the full gate set against the
candidate, benches interleaved A/B against that baseline, and writes one
append-only ledger row with the verdict and the complete evidence.

It will refuse to hand you an `accepted` unless:

- the upstream test suite is green **and** golden replay is byte-identical (or
  within the target's declared FP tolerance);
- the **differential-fuzz gate actually passed** — a real run reporting zero
  divergences. No fuzz run, no machine accept. Ever;
- ASan/UBSan are clean (and TSan, when the target configures it);
- the bootstrap 95% CI lower bound of the speedup clears the threshold in
  `config/accept.toml` (default 2%);
- and a conservative lexical risk classifier finds **no** concurrency, unsafe,
  or floating-point signals in the patch. If it does, the verdict is capped at
  `needs-human-review` no matter how good the numbers are.

Any of these failing produces a rejected row. **That is a complete, valid
outcome** — the row is the deliverable. Write it and move on. Do not iterate on
a rejected patch more than twice without a genuinely new hypothesis.

## Report

    just report <run-id>

Mechanically generates the ROI from the ledger row: throughput → cores →
dollars, and/or latency percentiles — every figure with its CI, workload, and
methodology printed inline. It flags any row that was not accepted, so a
rejected attempt cannot be laundered into a dollar figure.

## Unattended

    just agent-attempt <target> <run-id>

Runs one full attempt behind the OS boundary: `harnessd` as the trusted uid on
a Unix socket, the agent loop as an unprivileged user, the verdict pipeline
wrapped in a network namespace (`scripts/no-net.sh`) that **fails closed** — if
namespaces are unavailable it refuses to run rather than quietly running with
the network up. `just isolation-check` verifies the boundary from the agent's
side.

## When the numbers are good and the change is scary

FP-ordering, concurrency primitives, anything UB-adjacent: `needs-human-review`,
always, regardless of the speedup. This is not a formality. The one false
accept in peltier's history (`phase2-comrak-010`) passed its bench and was
caught by a human audit afterwards — which is precisely why the sanitizer lane
is now machine-enforced and why this rule does not bend.
