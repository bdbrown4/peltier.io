#!/bin/sh
# Deterministically generate the cjson corpus — a large synthetic JSON
# document exercising every parse path: nested objects/arrays, strings
# with escapes and unicode, ints, floats (the strtod/sprintf hot path),
# booleans, nulls. No randomness — byte-identical on every machine, so
# the MANIFEST.sha256 pin is reproducible. Run from repo root:
#   sh corpora/cjson/gen-corpus.sh
set -eu
DIR="$(cd "$(dirname "$0")" && pwd)"
OUT="$DIR/input/big.json"
mkdir -p "$DIR/input"

python3 - "$OUT" <<'PY'
import json, sys

# Deterministic: a fixed linear-congruential stream, no `random` module,
# so output is identical everywhere.
class Det:
    def __init__(self, seed=2463534242):
        self.s = seed
    def next(self):
        self.s ^= (self.s << 13) & 0xFFFFFFFF
        self.s ^= (self.s >> 17)
        self.s ^= (self.s << 5) & 0xFFFFFFFF
        return self.s & 0xFFFFFFFF
    def rng(self, n):
        return self.next() % n

d = Det()
WORDS = ["alpha","bravo","charlie","delta","echo","foxtrot","golf","hotel",
         "quote\"inside","tab\tchar","new\nline","back\\slash","unécode","☃snow"]

def record(i):
    return {
        "id": i,
        "guid": f"{d.next():08x}-{d.next():04x}",
        "active": bool(d.rng(2)),
        "score": d.rng(1000000) / 1000.0,     # float -> strtod/sprintf path
        "ratio": d.rng(1 << 30) / (1 << 30),  # dense fractional float
        "count": d.rng(1 << 31) - (1 << 30),  # signed int
        "name": WORDS[d.rng(len(WORDS))] + str(d.rng(9999)),
        "tags": [WORDS[d.rng(len(WORDS))] for _ in range(d.rng(6))],
        "coords": [d.rng(1 << 20) / 997.0 for _ in range(3)],
        "meta": None if d.rng(4) == 0 else {
            "created": d.next(),
            "nested": {"a": d.rng(100), "b": [d.rng(50) for _ in range(d.rng(4))]},
        },
    }

doc = {
    "schema": "hotpath.cjson.corpus/v1",
    "generated_by": "gen-corpus.sh (deterministic)",
    "records": [record(i) for i in range(20000)],
}
with open(sys.argv[1], "w") as f:
    json.dump(doc, f, ensure_ascii=False, separators=(",", ":"))
PY

echo "wrote $OUT ($(wc -c < "$OUT") bytes)"
# Pin the corpus and regenerate the manifest.
( cd "$DIR/input" && find . -type f -print0 | sort -z | xargs -0 sha256sum ) > "$DIR/MANIFEST.sha256"
echo "manifest: $DIR/MANIFEST.sha256"
