# Human ruling — `phase0-comrak-002` (mimalloc allocator swap)

**Ruling: ACCEPTED, scoped to the measured workload.**
**Ruled by:** project owner (repo maintainer), recorded 2026-07-12.
**Ledger row:** `phase0-comrak-002` remains `needs-human-review` (the
append-only ledger records what the *pipeline* decided; this document is
the human decision layered on top).

## What was measured

Swapping comrak's global allocator to
[mimalloc](https://github.com/microsoft/mimalloc):

- **Speedup: 1.046 median, 95% CI [1.033, 1.058]** (+4.6%) on the comrak
  CLI rendering the pinned Pro Git corpus (11.0 MB markdown, single
  stream), interleaved A/B vs a pristine-rebuilt baseline.
- Byte-identical golden replay. The pipeline routed it to
  `needs-human-review` per SPEC §8 because a global allocator swap is a
  behavioral change no single benchmark can fully clear.

## The ruling and its scope

**Accepted** as a real, CI-significant win **for the comrak CLI's
single-stream markdown-render workload** — the workload measured, on the
calibrated hardware in `results/calibration/`.

This is explicitly **not** a blanket "always link mimalloc" mandate:

- The upside (+4.6%) is genuine but modest; it does not justify making
  mimalloc a default across unrelated targets or workloads without their
  own measurement.
- The comrak CLI on this workload is single-threaded, which is what makes
  the ruling low-risk here — the concurrency/fragmentation concerns SPEC
  §8 flags are minimal for a short-lived, single-stream CLI process. A
  long-running or heavily-multithreaded comrak *library* embedding would
  need its own review before inheriting this.

## Why this is the honest resolution

The `needs-human-review` tier exists precisely to force this judgment
instead of letting a green benchmark auto-ship a global behavioral
change. The judgment: yes for the measured CLI workload, with the scope
written down so the number can't be over-generalized later. The tier did
its job; the win is real; the ruling is bounded.
