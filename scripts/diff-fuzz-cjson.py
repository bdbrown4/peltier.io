#!/usr/bin/env python3
"""Differential fuzz for cjson attempts: pristine vs candidate harness must
produce byte-identical stdout (serialization + checksum) AND identical exit
code on mutated JSON. cJSON is a deterministic single-threaded parser, so
raw byte comparison needs no canonicalization.

    python3 scripts/diff-fuzz-cjson.py <workdir> <iters> <baseline-bin> <candidate-bin>

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
therefore graded a hard gate FAILURE, never a pass.

Seeds: slices of the pinned corpus plus hand JSON fragments. Mutations
target parser edges: structural tokens, string escapes, number forms
(leading zeros, +/- exponents, huge/deep values), and raw byte noise.
"""

import random
import subprocess
import sys
from pathlib import Path
from typing import NoReturn

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpora/cjson/input/big.json"

USAGE = "usage: diff-fuzz-cjson.py <workdir> <iters> <baseline-bin> <candidate-bin>"

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


def die(msg: str) -> NoReturn:
    """Fatal setup error: exit 2 having printed NO FUZZ-RESULT line. diff-test
    grades a fuzz command that never reports the line as a hard gate failure,
    which is the right posture for a gate that could not run at all — it must
    never be gradeable as a pass."""
    print(f"diff-fuzz-cjson: {msg}", file=sys.stderr)
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
            "{baseline}/{candidate} in [gates].fuzz — check targets/cjson/target.toml"
        )
    return path


def keep_dir(work: Path, tag: str) -> Path:
    """First unused divergence-<tag> directory under `work`.

    The RNG is seeded, so re-running in the same work dir re-diverges at the
    same iteration: the kept-divergence directory already exists. The old
    plain `mkdir()` raised FileExistsError there and killed the run *before*
    the FUZZ-RESULT line, so diff-test reported a genuine divergence — the
    whole point of the gate — as "fuzz command did not report FUZZ-RESULT".
    This never raises; it picks the next free name instead.
    """
    path, n = work / f"divergence-{tag}", 2
    while path.exists():
        path = work / f"divergence-{tag}-{n}"
        n += 1
    return path


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


def main(argv: list[str]) -> int:
    work, iters, baseline, candidate = parse_args(argv)
    need_binary(baseline, "baseline")
    need_binary(candidate, "candidate")
    if not CORPUS.is_file():
        die(f"corpus not found: {CORPUS} (generate it with corpora/cjson/gen-corpus.sh)")

    rng = random.Random(0x1CE_B00C)
    corpus = CORPUS.read_bytes()
    seeds = list(SEEDS_STATIC)
    # windows of the real corpus (often invalid mid-slice — exercises the
    # error path, which must diverge identically or not at all). The max(1, …)
    # guards are no-ops on the pinned 5.6 MB corpus (identical seed set, so
    # the fuzz stays reproducible) and stop a degenerate tiny corpus from
    # raising a zero-step ValueError before FUZZ-RESULT is ever printed.
    for o in range(0, max(1, len(corpus) - 4096), max(1, len(corpus) // 200)):
        for n in (64, 512, 4096):
            seeds.append(corpus[o : o + n])
    work.mkdir(parents=True, exist_ok=True)
    case = work / "case.json"
    divergences = 0
    for i in range(iters):
        payload = mutate(rng.choice(seeds), rng)
        case.write_bytes(payload)
        a_out, a_rc = run(baseline, case)
        b_out, b_rc = run(candidate, case)
        if a_out != b_out or a_rc != b_rc:
            divergences += 1
            # Keeping the reproducer is best-effort: an I/O failure here must
            # never cost the FUZZ-RESULT line and turn a real divergence into
            # an unparseable gate error (see keep_dir).
            try:
                keep = keep_dir(work, str(i))
                keep.mkdir(parents=True, exist_ok=True)
                (keep / "input.json").write_bytes(payload)
                (keep / "baseline.out").write_bytes(a_out + f"\nrc={a_rc}".encode())
                (keep / "candidate.out").write_bytes(b_out + f"\nrc={b_rc}".encode())
                print(f"DIVERGENCE at iter {i} (rc {a_rc} vs {b_rc}); kept in {keep}")
            except OSError as e:
                print(f"DIVERGENCE at iter {i} (rc {a_rc} vs {b_rc}); "
                      f"could not keep artifacts: {e}")
        if (i + 1) % 1000 == 0:
            print(f"{i + 1}/{iters}, {divergences} divergences", flush=True)
    print(f"done: {iters} inputs, {divergences} divergences")
    # Machine contract, always the final stdout line on any completing run.
    print(f"FUZZ-RESULT iters={iters} divergences={divergences}", flush=True)
    return 1 if divergences else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
