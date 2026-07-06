# targets/

One directory per optimization target. This is the **only**
agent-writable path in the repository (SPEC §10); everything else is
trust layer.

## Layout

```
targets/<name>/
  target.toml         # upstream repo URL, pinned commit, build & bench commands
  equivalence.toml    # optional; absent = byte-identical required (SPEC §3.2)
  workspace/          # agent's checkout — patches land here via harness git-apply
```

Selection criteria (SPEC §6): CPU-bound (>70% user time), meaningful
test suite, scriptable workload, permissive license, active but not
hyper-optimized, builds cleanly in a container. Good hunting: codecs,
parsers, serializers, image processing, compression, static-site
generators, linters.

## equivalence.toml

Default (no file) is byte-identical outputs. FP-producing targets may
declare an explicit tolerance policy:

```toml
mode = "fp-tolerance"
abs = 1e-9    # max absolute difference per numeric value
rel = 1e-6    # max relative difference per numeric value
```

Declaring a tolerance covers output comparison only; FP *flag* changes
still route to needs-human-review (SPEC §8).
