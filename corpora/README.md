# corpora/

Golden-replay input corpora and expected outputs, one directory per
target. **Trust layer: read-only to the agent** (SPEC §10).

Every corpus directory carries a `MANIFEST.sha256` (`sha256sum` format:
`<digest>  <relative-path>` per line). `diff-test` verifies the manifest
before every run and refuses to run on any mismatch — a tampered corpus
is a stop-the-line event.

Regenerate a manifest only as a deliberate human action:

```sh
cd corpora/<target> && find . -type f ! -name MANIFEST.sha256 -exec sha256sum {} + > MANIFEST.sha256
```
