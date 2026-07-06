# Class 1 — Build configuration

Zero source changes. Always first: a large fraction of real-world CPU is
lost to shipping defaults.

## Preconditions
- Any CPU-bound target not already using an optimized release profile.
- Profile signature: flat-ish flamegraph with time spread across many
  small frames (inlining starved); heavy call overhead into tiny leaf
  functions; or a build that visibly lacks `-O2`/`--release`.
- Check before anything else: `opt-level`, `lto`, `codegen-units`,
  `panic`, `target-cpu` (Rust); `-O`, `-march`, `-flto` (C/C++).

## Procedure (cheapest first)
1. Confirm release profile: `opt-level=3`, `panic=abort` where the target
   permits, `codegen-units=1`.
2. `target-cpu=native` / `-march=native` — only when the deployment
   fleet's ISA baseline is known; record the chosen baseline in the
   hypothesis.
3. LTO: thin, then fat. Watch build-time cost; fat LTO on huge targets
   can be a net loss for the engagement.
4. PGO: instrument with the golden-replay corpus as the training run,
   rebuild with the profile. The corpus is representative by
   construction.
5. BOLT (post-link layout) where the binary and toolchain permit.

## Verification notes
- Flags must not include any `-ffast-math`-class option — that's an
  equivalence change, auto-routed to `needs-human-review` (SPEC §8).
- Byte-identical golden replay still applies: build flags occasionally
  change FP contraction on some ISAs (`-march` enabling FMA); if golden
  replay diverges on numeric output, stop — this class is not
  equivalence-preserving for that target without a tolerance policy.

## Known failure modes
- `target-cpu=native` on the bench machine but not the fleet: a real win
  that the customer cannot deploy. Pin the ISA baseline to the fleet.
- PGO trained on a non-representative workload overfits the benchmark —
  hold out workloads (SPEC §11).
- Layout changes from LTO/BOLT interact with layout bias (§7): measured
  wins here *especially* need the randomize-and-aggregate protocol.
