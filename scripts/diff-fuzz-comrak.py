#!/usr/bin/env python3
"""Differential fuzz for comrak attempts: pristine vs candidate must
produce byte-identical HTML (stdout), stderr, and exit code on mutated
markdown. comrak is a single-threaded stdin->stdout filter, so raw byte
comparison needs no canonicalization.

    python3 scripts/diff-fuzz-comrak.py <workdir> <iters> <candidate-bin>

Seeds: windows of the pinned Pro Git corpus. Mutations focus on inline/
block structure the parser and teardown paths exercise: emphasis, code
fences, links, HTML blocks, list markers, plus raw byte noise.
"""

import random
import subprocess
import sys
from pathlib import Path

ROOT = Path("/home/user/peltier.io")
BASE = ROOT / "targets/comrak/baseline/release/comrak"
CAND = ROOT / sys.argv[3]
CORPUS = ROOT / "corpora/comrak/progit.md"
WORK = Path(sys.argv[1])
ITERS = int(sys.argv[2])

TOKENS = [
    b"**", b"*", b"__", b"_", b"`", b"```", b"~~~", b"~~", b"[", b"](", b")",
    b"![", b"#", b"##", b">", b"- ", b"1. ", b"|", b"---", b"<div>", b"</div>",
    b"<!--", b"-->", b"\\", b"&amp;", b"<http://x>", b"    ", b"\t",
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
            del b[pos:min(len(b), pos + rng.randrange(1, 16))]
        elif op == 2 and pos < len(b):
            b[pos] = rng.randrange(256)
        elif op == 3:
            end = min(len(b), pos + rng.randrange(1, 64))
            b[end:end] = b[pos:end]
        else:
            b[pos:pos] = b"\n\n"
    return bytes(b)


def run(binary: Path, payload: bytes) -> tuple[bytes, bytes, int]:
    r = subprocess.run([str(binary)], input=payload, capture_output=True, timeout=60)
    return r.stdout, r.stderr, r.returncode


def main() -> int:
    rng = random.Random(0xC0FFEE)
    corpus = CORPUS.read_bytes()
    # Seed windows across the whole book, varied sizes.
    seeds = [
        corpus[o : o + n]
        for o in range(0, len(corpus) - 65536, len(corpus) // 300)
        for n in (512, 4096, 32768)
    ]
    WORK.mkdir(parents=True, exist_ok=True)
    divergences = 0
    for i in range(ITERS):
        payload = mutate(rng.choice(seeds), rng)
        a = run(BASE, payload)
        b = run(CAND, payload)
        if a != b:
            divergences += 1
            keep = WORK / f"divergence-{i}"
            keep.mkdir()
            (keep / "input.md").write_bytes(payload)
            (keep / "baseline.out").write_bytes(a[0] + b"\n--stderr--\n" + a[1] + f"\nrc={a[2]}".encode())
            (keep / "candidate.out").write_bytes(b[0] + b"\n--stderr--\n" + b[1] + f"\nrc={b[2]}".encode())
            print(f"DIVERGENCE at iter {i}; kept in {keep}")
        if (i + 1) % 1000 == 0:
            print(f"{i + 1}/{ITERS}, {divergences} divergences", flush=True)
    print(f"done: {ITERS} inputs, {divergences} divergences")
    return 1 if divergences else 0


if __name__ == "__main__":
    sys.exit(main())
