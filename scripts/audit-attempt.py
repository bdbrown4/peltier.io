#!/usr/bin/env python3
"""Audit one ledger attempt (SPEC §10: 100% human audit in Phase 2).

Prints the full evidence for a run and re-checks the verdict against the
accept rule mechanically:

    python3 scripts/audit-attempt.py <run_id> [--threshold 0.02]

Checks:
  - verdict vs bench evidence: accepted requires ci_lower >= 1 + threshold
    AND all gates green AND not flagged needs-human-review
  - rejected-bench requires the CI to actually fail the bar
  - patch paths stay inside the target workspace
The human part of the audit — does the patch express the hypothesis, is
the equivalence tier right — is printed for eyeballing, not automated.
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("run_id")
    ap.add_argument("--threshold", type=float, default=0.02)
    args = ap.parse_args()

    con = sqlite3.connect(ROOT / "results/ledger.sqlite")
    con.row_factory = sqlite3.Row
    row = con.execute("SELECT * FROM attempts WHERE run_id = ?", (args.run_id,)).fetchone()
    if row is None:
        print(f"no ledger row for {args.run_id}", file=sys.stderr)
        return 2

    gates = json.loads(row["gates"]) if row["gates"] else {}
    bench = json.loads(row["bench"]) if row["bench"] else {}
    verdict = row["verdict"]

    print(f"run_id       : {row['run_id']}")
    print(f"target       : {row['target']} @ {row['target_commit']}")
    print(f"class/hotspot: {row['playbook_class']} / {row['hotspot']}")
    print(f"hypothesis   : {row['hypothesis']}")
    print(f"verdict      : {verdict}")
    print(f"gates        : {json.dumps(gates, indent=2)}")
    print(f"bench        : {json.dumps(bench, indent=2)}")

    problems: list[str] = []
    ci = bench.get("speedup_ci") or [None, None]
    ci_lo = ci[0]
    bar = 1.0 + args.threshold

    if verdict == "accepted":
        if ci_lo is None or ci_lo < bar:
            problems.append(f"accepted but CI lower bound {ci_lo} < {bar}")
        bad_gates = {k: v for k, v in gates.items() if v not in (True, "pass", "passed", "ok")}
        if bad_gates:
            problems.append(f"accepted with non-green gates: {bad_gates}")
    elif verdict == "rejected-bench":
        if ci_lo is not None and ci_lo >= bar:
            problems.append(f"rejected-bench but CI lower bound {ci_lo} >= {bar}")
    elif verdict == "needs-human-review":
        pass  # always a valid resting state; the ruling is ours
    elif verdict == "rejected-gates":
        if all(v in (True, "pass", "passed", "ok") for v in gates.values()) and gates:
            problems.append("rejected-gates but every recorded gate is green")

    patch = row["patch"] or ""
    for line in patch.splitlines():
        if line.startswith(("--- ", "+++ ")):
            p = line[4:].strip()
            if p.startswith("/") or "../" in p:
                problems.append(f"patch path escapes workspace: {p}")

    print("\npatch:")
    print(patch if patch else "  (none recorded)")

    if problems:
        print("\nAUDIT: PROBLEMS FOUND", file=sys.stderr)
        for p in problems:
            print(f"  - {p}", file=sys.stderr)
        return 1
    print("\nAUDIT: mechanical checks clean (human eyeball still required)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
