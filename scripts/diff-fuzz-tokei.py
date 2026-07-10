#!/usr/bin/env python3
"""Differential fuzz for phase2-tokei-001: pristine vs candidate tokei
must produce byte-identical output on mutated source inputs targeting
the patched code path (quote/comment token scanning).

Seeds: files from the pinned corpus (many languages). Mutations focus on
quote/comment structure: inserting/deleting/duplicating quote chars,
comment openers/closers, backslashes, and random bytes; plus boundary
slices. Deterministic RNG for reproducibility.
"""

import random
import subprocess
import sys
from pathlib import Path

ROOT = Path("/home/user/peltier.io")
BASE = ROOT / "targets/tokei/baseline/release/tokei"
CAND = ROOT / "targets/tokei/candidate-4e3da7e6bd1c/release/tokei"
CORPUS = ROOT / "corpora/tokei/input/copy1"
WORK = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/tmp/difffuzz")
ITERS = int(sys.argv[2]) if len(sys.argv) > 2 else 10000

TOKENS = [
    b'"', b"'", b'`', b'"""', b"'''", b"/*", b"*/", b"//", b"#", b"--",
    b"<!--", b"-->", b"(*", b"*)", b"{-", b"-}", b"\\", b"\\\\", b'\\"',
    b"r#\"", b"\"#", b"=begin", b"=end", b"'''", b'f"""', b"%{", b"}",
]

def mutate(data: bytes, rng: random.Random) -> bytes:
    ops = rng.randrange(1, 6)
    b = bytearray(data)
    for _ in range(ops):
        if not b:
            b = bytearray(rng.choice(TOKENS))
            continue
        op = rng.randrange(5)
        pos = rng.randrange(len(b) + 1)
        if op == 0:      # insert a token
            b[pos:pos] = rng.choice(TOKENS)
        elif op == 1 and len(b) > 2:  # delete a slice
            end = min(len(b), pos + rng.randrange(1, 8))
            del b[pos:end]
        elif op == 2:    # flip to a random byte
            if pos < len(b):
                b[pos] = rng.randrange(256)
        elif op == 3:    # duplicate a slice
            end = min(len(b), pos + rng.randrange(1, 32))
            b[end:end] = b[pos:end]
        else:            # newline injection (line-structure stress)
            b[pos:pos] = b"\n"
    return bytes(b)

def canonicalize(payload: bytes) -> str:
    """Parse tokei JSON and sort every list so benign output-order
    nondeterminism (parallel walker arrival order) doesn't count as a
    divergence; counts/stats still must match exactly."""
    import json

    def norm(o):
        if isinstance(o, dict):
            return {k: norm(v) for k, v in sorted(o.items())}
        if isinstance(o, list):
            return sorted((norm(v) for v in o), key=lambda x: json.dumps(x, sort_keys=True))
        return o

    return json.dumps(norm(json.loads(payload)), sort_keys=True)


def run(binary: Path, d: Path) -> tuple[str, bytes, int]:
    r = subprocess.run(
        [str(binary), "--sort", "code", "--output", "json", str(d)],
        capture_output=True, env={"RAYON_NUM_THREADS": "1", "PATH": "/usr/bin:/bin"},
    )
    try:
        canon = canonicalize(r.stdout)
    except Exception as e:  # unparseable output is always a divergence candidate
        canon = f"UNPARSEABLE:{e}:{r.stdout[:200]!r}"
    return canon + f"||rc={r.returncode}", r.stdout, r.returncode

def main() -> int:
    rng = random.Random(0xB16B00B5)
    seeds = sorted(CORPUS.rglob("*"))
    seeds = [p for p in seeds if p.is_file() and p.stat().st_size < 200_000][:400]
    if not seeds:
        print("no seeds found", file=sys.stderr)
        return 2
    WORK.mkdir(parents=True, exist_ok=True)
    fdir = WORK / "case"
    divergences = 0
    done = 0
    # Batch: mutate BATCH files per tokei invocation (tokei scans a dir),
    # keeping each seed's extension so language detection stays realistic.
    BATCH = 50
    while done < ITERS:
        if fdir.exists():
            for f in fdir.iterdir():
                f.unlink()
        fdir.mkdir(exist_ok=True)
        for j in range(min(BATCH, ITERS - done)):
            seed = rng.choice(seeds)
            name = f"m{done + j}{seed.suffix or '.txt'}"
            (fdir / name).write_bytes(mutate(seed.read_bytes(), rng))
        a_canon, a_raw, a_rc = run(BASE, fdir)
        b_canon, b_raw, b_rc = run(CAND, fdir)
        if a_canon != b_canon:
            divergences += 1
            keep = WORK / f"divergence-{done}"
            fdir.rename(keep)
            (keep / "_baseline.out").write_bytes(a_raw + f"\nrc={a_rc}".encode())
            (keep / "_candidate.out").write_bytes(b_raw + f"\nrc={b_rc}".encode())
            print(f"DIVERGENCE at batch starting {done}; inputs+outputs kept in {keep}")
        done += BATCH
        if done % 1000 == 0:
            print(f"{done}/{ITERS} mutated files, {divergences} divergences", flush=True)
    print(f"done: {done} mutated files across {done // BATCH} runs, {divergences} divergences")
    return 1 if divergences else 0

if __name__ == "__main__":
    sys.exit(main())
