#!/usr/bin/env python3
"""Differential fuzz for cjson attempts: pristine vs candidate harness must
produce byte-identical stdout (serialization + checksum) AND identical exit
code on mutated JSON. cJSON is a deterministic single-threaded parser, so
raw byte comparison needs no canonicalization.

    python3 scripts/diff-fuzz-cjson.py <workdir> <iters> <candidate-bin>

Seeds: slices of the pinned corpus plus hand JSON fragments. Mutations
target parser edges: structural tokens, string escapes, number forms
(leading zeros, +/- exponents, huge/deep values), and raw byte noise.
"""

import random
import subprocess
import sys
from pathlib import Path

ROOT = Path("/home/user/peltier.io")
BASE = ROOT / "targets/cjson/baseline/cjson-bench"
CAND = ROOT / sys.argv[3] if len(sys.argv) > 3 else ROOT / "targets/cjson/baseline/cjson-bench"
CORPUS = ROOT / "corpora/cjson/input/big.json"
WORK = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/tmp/difffuzz-cjson")
ITERS = int(sys.argv[2]) if len(sys.argv) > 2 else 10000

TOKENS = [
    b"{", b"}", b"[", b"]", b'"', b":", b",", b"\\", b"\\n", b"\\u00e9", b"\\uD83D",
    b"true", b"false", b"null", b"0", b"-0", b"1e999", b"-1.5E-3", b"00", b"1.",
    b".5", b"1e", b"NaN", b"Infinity", b'\\"', b'"key"', b"[]", b"{}", b'""',
    b"1234567890123456789012345", b"\x00", b"\xff", b"\t", b"\n",
]

SEEDS_STATIC = [
    b'{}', b'[]', b'{"a":1}', b'[1,2,3]', b'"hello"', b'true', b'null',
    b'{"n":-1.5e10,"s":"a\\tb","u":"\\u00e9","arr":[null,false,{}]}',
    b'[[[[[[1]]]]]]', b'{"a":{"b":{"c":{"d":1}}}}', b'123.456e-78',
    b'{"esc":"\\"\\\\\\/\\b\\f\\n\\r\\t"}', b'  {  "x"  :  [ 1 , 2 ]  }  ',
]


def mutate(data: bytes, rng: random.Random) -> bytes:
    b = bytearray(data)
    for _ in range(rng.randrange(1, 8)):
        if not b:
            b = bytearray(rng.choice(TOKENS))
            continue
        op = rng.randrange(5)
        pos = rng.randrange(len(b) + 1)
        if op == 0:
            b[pos:pos] = rng.choice(TOKENS)
        elif op == 1 and len(b) > 2:
            del b[pos:min(len(b), pos + rng.randrange(1, 12))]
        elif op == 2 and pos < len(b):
            b[pos] = rng.randrange(256)
        elif op == 3:
            end = min(len(b), pos + rng.randrange(1, 48))
            b[end:end] = b[pos:end]
        else:
            b[pos:pos] = rng.choice(TOKENS)
    return bytes(b)


def run(binary: Path, path: Path) -> tuple[bytes, int]:
    r = subprocess.run([str(binary), str(path), "1"], capture_output=True, timeout=30)
    return r.stdout, r.returncode


def main() -> int:
    rng = random.Random(0x1CE_B00C)
    corpus = CORPUS.read_bytes()
    seeds = list(SEEDS_STATIC)
    # windows of the real corpus (often invalid mid-slice — exercises the
    # error path, which must diverge identically or not at all)
    for o in range(0, len(corpus) - 4096, len(corpus) // 200):
        for n in (64, 512, 4096):
            seeds.append(corpus[o : o + n])
    WORK.mkdir(parents=True, exist_ok=True)
    case = WORK / "case.json"
    divergences = 0
    for i in range(ITERS):
        payload = mutate(rng.choice(seeds), rng)
        case.write_bytes(payload)
        a_out, a_rc = run(BASE, case)
        b_out, b_rc = run(CAND, case)
        if a_out != b_out or a_rc != b_rc:
            divergences += 1
            keep = WORK / f"divergence-{i}"
            keep.mkdir(exist_ok=True)
            (keep / "input.json").write_bytes(payload)
            (keep / "baseline.out").write_bytes(a_out + f"\nrc={a_rc}".encode())
            (keep / "candidate.out").write_bytes(b_out + f"\nrc={b_rc}".encode())
            print(f"DIVERGENCE at iter {i} (rc {a_rc} vs {b_rc}); kept in {keep}")
        if (i + 1) % 1000 == 0:
            print(f"{i + 1}/{ITERS}, {divergences} divergences", flush=True)
    print(f"done: {ITERS} inputs, {divergences} divergences")
    return 1 if divergences else 0


if __name__ == "__main__":
    sys.exit(main())
