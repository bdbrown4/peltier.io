# Case studies

Five verified case studies, each reproducible from a pinned corpus through
the same gated pipeline, each number carrying its 95% CI and workload.

- [tokei — compounding algorithmic wins (Rust)](./tokei.md) — three class-5
  wins on the same hot function, each unattended and audited.
- [Cheap wins first: build flags & allocators](./build-flags.md) — LTO
  accepted (+10%), mimalloc held for a human ruling then accepted-scoped.
- [The caught false-accept (comrak)](./comrak-false-accept.md) — the
  leaking teardown the bench loved, overturned by audit. The most important
  study.
- [cJSON — the cross-language proof (C)](./cjson.md) — a sub-threshold win
  rejected, then a byte-identical win accepted, on a C target.
- [Services & scale: latency under load](./service.md) — the cJSON win
  measured as a latency delta under coordinated-omission-correct replay,
  turned into a dollar figure mechanically.
