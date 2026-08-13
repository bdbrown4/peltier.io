#!/usr/bin/env bash
# Fetch the CCFHL Kreuzer-Skarke equivalence data into data/raw/.
#
# Source: http://www-thphys.physics.ox.ac.uk/projects/CalabiYau/KSEquiv
# Paper:  arXiv:2310.05909, Chandra, Constantin, Fraser-Taliente, Harvey, Lukas
#
# Checksums in data/SHA256SUMS are the bytes this analysis was actually run
# against (fetched 2026-08-13). If the upstream files are ever revised the
# verification below fails loudly rather than silently changing the results.
set -euo pipefail

BASE="https://www-thphys.physics.ox.ac.uk/projects/CalabiYau/KSEquiv"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RAW="$HERE/data/raw"

mkdir -p "$RAW"
cd "$RAW"

for f in ManifoldData.zip EquivalenceData.zip IndicesForPolytopesAndTriangulations.zip; do
  if [ -f "$f" ]; then
    echo "have $f"
  else
    echo "fetching $f"
    curl -fSL --retry 4 --retry-delay 2 --max-time 900 -O "$BASE/$f"
  fi
done

echo "verifying checksums"
sha256sum -c "$HERE/data/SHA256SUMS"

# Only h = 3, 4, 5 are needed. The h = 6 manifold file is 1.6 GB uncompressed
# and the index archive is 323 MB; neither is used, so neither is unpacked.
echo "extracting h = 3, 4, 5"
unzip -o -q ManifoldData.zip \
  'ManifoldData/H11is3*' 'ManifoldData/H11is4*' 'ManifoldData/H11is5*'
unzip -o -q EquivalenceData.zip -x '*/.*'

echo "done:"
ls -la "$RAW/ManifoldData" "$RAW/EquivalenceData"
