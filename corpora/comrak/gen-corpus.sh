#!/bin/sh
# Regenerate the comrak bench corpus from the pinned progit checkout
# (targets/comrak/target.toml [source.submodules]), then verify the pin.
set -eu
cd "$(dirname "$0")"
cat ../../targets/comrak/workspace/vendor/progit/*/*/*.markdown > progit.md
sha256sum -c MANIFEST.sha256
