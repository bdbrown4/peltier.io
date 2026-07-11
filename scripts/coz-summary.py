#!/usr/bin/env python3
"""Summarize a coz causal profile: rank source lines by their estimated
causal effect on throughput. coz writes newline-delimited JSON records;
for each (line, virtual-speedup) it records progress-point throughput.
We fit the slope of throughput-gain vs virtual-speedup per line — a
steeper positive slope means "speeding up this line most raises overall
throughput", which is exactly the class-selection signal a flat profiler
can't give.

    python3 scripts/coz-summary.py results/<target>/coz/profile.coz
"""

from __future__ import annotations

import collections
import json
import sys
from pathlib import Path


def main() -> int:
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else None
    if not path or not path.exists():
        print("usage: coz-summary.py <profile.coz>", file=sys.stderr)
        return 2

    # experiments: line -> list of (speedup, progress-rate)
    exps: dict[str, list[tuple[float, float]]] = collections.defaultdict(list)
    baseline: dict[str, float] = {}
    for raw in path.read_text().splitlines():
        raw = raw.strip()
        if not raw:
            continue
        try:
            rec = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if rec.get("type") != "experiment":
            continue
        line = rec.get("selected", "?")
        speedup = float(rec.get("speedup", 0.0))
        duration = float(rec.get("duration", 0.0))
        delta = float(rec.get("delta", 0.0))
        if delta <= 0:
            continue
        rate = duration / delta  # time per progress unit; lower = faster
        exps[line].append((speedup, rate))
        if speedup == 0.0:
            baseline[line] = rate

    rows = []
    for line, pts in exps.items():
        base = baseline.get(line)
        if not base or len(pts) < 2:
            continue
        # % throughput improvement at the max virtual speedup tried
        pts.sort()
        s_max, r_max = pts[-1]
        if s_max <= 0 or r_max <= 0:
            continue
        gain = (base - r_max) / base  # fractional throughput gain
        rows.append((gain, s_max, line))

    rows.sort(reverse=True)
    print(f"{'throughput_gain':>16}  {'@speedup':>9}  line")
    print("-" * 70)
    for gain, s_max, line in rows[:25]:
        print(f"{gain * 100:>14.1f}%  {s_max * 100:>7.0f}%  {line}")
    if not rows:
        print("(no usable experiments — did the run mark progress points?)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
