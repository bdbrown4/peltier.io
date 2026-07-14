# corpora/

Golden-replay input corpora and expected outputs, one directory per
target. **Trust layer: read-only to the agent** (SPEC §10).

Every corpus directory carries a `MANIFEST.sha256` (`sha256sum` format:
`<digest>  <relative-path>` per line). `diff-test` verifies the manifest
before every run and refuses to run on any mismatch — a tampered corpus
is a stop-the-line event.

`just pin-check <target>` regenerates the corpus and verifies it against
the manifest; it never rewrites the manifest. Regenerate a manifest only
as a deliberate human action:

```sh
just pin-corpus <target>    # gen-corpus.sh --pin
```

A directory may also carry a `TESTSUITE.sha256` pinning the upstream test
suite (repo-root-relative paths). `diff-test` verifies it before running
upstream tests and `targets/fetch.sh` verifies it after every fetch;
generate it with `scripts/pin-testsuite.sh <target> <path>...`.
