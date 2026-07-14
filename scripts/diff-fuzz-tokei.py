#!/usr/bin/env python3
"""Differential fuzz for tokei attempts: pristine vs candidate tokei
must produce byte-identical output on mutated source inputs targeting
the patched code path (quote/comment token scanning).

    python3 scripts/diff-fuzz-tokei.py <workdir> <iters> <baseline-bin> <candidate-bin>

All four arguments are required and are supplied by the harness: diff-test
substitutes {iters}, {baseline} and {candidate} into the target's
[gates].fuzz template and runs it from the repo root. Both binary paths may
be repo-root-relative OR absolute (the accept path rebuilds the pristine
baseline to an absolute path) — relative ones resolve against the repo root.
Nothing here may hardcode a baseline location: the harness owns where the
pristine build lands, and a hardcoded path is a gate that always hard-fails.

Final stdout line is the machine contract diff-test parses:
FUZZ-RESULT iters=<n> divergences=<m>. Exit 0 iff the run completed with
zero divergences, 1 on divergence (the line is still printed, with the real
count), 2 on a usage/setup error — which prints no FUZZ-RESULT and is
therefore graded a hard gate FAILURE, never a pass. `iters` counts mutated
files (tokei scans a directory, so files are batched per invocation).

Seeds: files from the pinned corpus (many languages). Mutations focus on
quote/comment structure: inserting/deleting/duplicating quote chars,
comment openers/closers, backslashes, and random bytes; plus boundary
slices. Deterministic RNG for reproducibility.
"""

import random
import subprocess
import sys
from pathlib import Path
from typing import NoReturn

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpora/tokei/input/copy1"

USAGE = "usage: diff-fuzz-tokei.py <workdir> <iters> <baseline-bin> <candidate-bin>"

TOKENS = [
    b'"', b"'", b'`', b'"""', b"'''", b"/*", b"*/", b"//", b"#", b"--",
    b"<!--", b"-->", b"(*", b"*)", b"{-", b"-}", b"\\", b"\\\\", b'\\"',
    b"r#\"", b"\"#", b"=begin", b"=end", b"'''", b'f"""', b"%{", b"}",
]


def die(msg: str) -> NoReturn:
    """Fatal setup error: exit 2 having printed NO FUZZ-RESULT line. diff-test
    grades a fuzz command that never reports the line as a hard gate failure,
    which is the right posture for a gate that could not run at all — it must
    never be gradeable as a pass."""
    print(f"diff-fuzz-tokei: {msg}", file=sys.stderr)
    raise SystemExit(2)


def parse_args(argv: list[str]) -> tuple[Path, int, Path, Path]:
    """<workdir> <iters> <baseline-bin> <candidate-bin>, all positional."""
    if len(argv) != 5:
        die(f"expected 4 arguments, got {len(argv) - 1}\n{USAGE}")
    work, raw_iters, baseline, candidate = argv[1:5]
    if not raw_iters.isdigit():
        die(f"iters must be a non-negative integer, got {raw_iters!r}\n{USAGE}")
    iters = int(raw_iters)
    if iters < 1:
        # A zero-iteration run would print FUZZ-RESULT iters=0 divergences=0
        # and exit 0 — a fuzz gate that passes without fuzzing anything.
        die(f"iters must be >= 1, got {iters}\n{USAGE}")
    # `ROOT / p` keeps a repo-root-relative path anchored at the repo root and
    # leaves an absolute path untouched (pathlib drops the left operand when
    # the right side is absolute). The harness supplies either form.
    return ROOT / work, iters, ROOT / baseline, ROOT / candidate


def need_binary(path: Path, label: str) -> Path:
    if not path.is_file():
        die(
            f"{label} binary not found: {path}\nthe harness substitutes "
            "{baseline}/{candidate} in [gates].fuzz — check targets/tokei/target.toml"
        )
    return path


def keep_dir(work: Path, tag: str) -> Path:
    """First unused divergence-<tag> directory under `work`.

    The RNG is seeded, so re-running in the same work dir re-diverges at the
    same batch: the kept-divergence directory already exists. The old plain
    `rename()` raised (the destination is a non-empty directory) and killed
    the run *before* the FUZZ-RESULT line, so diff-test reported a genuine
    divergence — the whole point of the gate — as "fuzz command did not
    report FUZZ-RESULT". This never raises; it picks the next free name.
    """
    path, n = work / f"divergence-{tag}", 2
    while path.exists():
        path = work / f"divergence-{tag}-{n}"
        n += 1
    return path


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


def main(argv: list[str]) -> int:
    work, iters, baseline, candidate = parse_args(argv)
    need_binary(baseline, "baseline")
    need_binary(candidate, "candidate")
    if not CORPUS.is_dir():
        die(f"corpus not found: {CORPUS} (generate it with corpora/tokei/gen-corpus.sh)")

    rng = random.Random(0xB16B00B5)
    seeds = sorted(CORPUS.rglob("*"))
    seeds = [p for p in seeds if p.is_file() and p.stat().st_size < 200_000][:400]
    if not seeds:
        die(f"no seeds found under {CORPUS}")
    work.mkdir(parents=True, exist_ok=True)
    fdir = work / "case"
    divergences = 0
    done = 0
    runs = 0
    # Batch: mutate BATCH files per tokei invocation (tokei scans a dir),
    # keeping each seed's extension so language detection stays realistic.
    BATCH = 50
    while done < iters:
        if fdir.exists():
            for f in fdir.iterdir():
                f.unlink()
        fdir.mkdir(exist_ok=True)
        batch = min(BATCH, iters - done)
        for j in range(batch):
            seed = rng.choice(seeds)
            name = f"m{done + j}{seed.suffix or '.txt'}"
            (fdir / name).write_bytes(mutate(seed.read_bytes(), rng))
        a_canon, a_raw, a_rc = run(baseline, fdir)
        b_canon, b_raw, b_rc = run(candidate, fdir)
        if a_canon != b_canon:
            divergences += 1
            # Keeping the reproducer is best-effort: an I/O failure here must
            # never cost the FUZZ-RESULT line and turn a real divergence into
            # an unparseable gate error (see keep_dir). If the rename fails,
            # fdir survives and the next batch cleans it out.
            try:
                keep = keep_dir(work, str(done))
                fdir.rename(keep)
                (keep / "_baseline.out").write_bytes(a_raw + f"\nrc={a_rc}".encode())
                (keep / "_candidate.out").write_bytes(b_raw + f"\nrc={b_rc}".encode())
                print(f"DIVERGENCE at batch starting {done}; inputs+outputs kept in {keep}")
            except OSError as e:
                print(f"DIVERGENCE at batch starting {done}; could not keep artifacts: {e}")
        done += batch
        runs += 1
        if done % 1000 == 0:
            print(f"{done}/{iters} mutated files, {divergences} divergences", flush=True)
    print(f"done: {done} mutated files across {runs} runs, {divergences} divergences")
    # Machine contract, always the final stdout line on any completing run.
    print(f"FUZZ-RESULT iters={done} divergences={divergences}", flush=True)
    return 1 if divergences else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
