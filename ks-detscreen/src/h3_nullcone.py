#!/usr/bin/env python3
"""Do the 33 h=3 mergers sit on the nullcone?

At h^1,1 = 3 the published data contains something no higher Picard number
does: genuine GL(3, Q) equivalences, found by symbolic solution of Eq. (I.3)
rather than by a bounded integer search. Under GL(3, Z) there are 183 classes;
under GL(3, Q) there are 150. The 33 lost classes are the mergers -- pairs of
manifolds that are rationally equivalent but not integrally equivalent.

Those mergers are the only place in the whole dataset where |det P| for a
*known* rational equivalence can be read off rather than guessed. Two things
are asked of them here:

  1. What is det P, actually? (Recomputed from the matrices, not trusted.)
  2. Do they sit on the nullcone?

The second question is the one that decides whether |det P| = 1 is explained.
For h = 3 the invariants of the ternary cubic are I_4 (degree 4, weight 4) and
I_6 (degree 6, weight 6). If a merged pair has some invariant non-zero, then
equality of invariants across the merger *forces* (det P)^4 = (det P)^6 = 1,
so |det P| = 1 is a consequence, not a coincidence. If instead every merger
sits on the nullcone I_4 = I_6 = 0, the invariants impose no constraint on
det P at all -- and the observed |det P| = 1 is unexplained by them.

Usage:  python3 src/h3_nullcone.py
"""

from __future__ import annotations

import json
import os
import sys
from fractions import Fraction

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ksdata  # noqa: E402

I4_KEY = "QuarticInv"  # degree 4, weight 3*4/3 = 4
I6_KEY = "2h-invariant-sextic"  # degree 6, weight 3*6/3 = 6


class UnionFind:
    def __init__(self):
        self.parent: dict = {}

    def find(self, x):
        self.parent.setdefault(x, x)
        while self.parent[x] != x:
            self.parent[x] = self.parent[self.parent[x]]
            x = self.parent[x]
        return x

    def union(self, a, b):
        ra, rb = self.find(a), self.find(b)
        if ra != rb:
            self.parent[ra] = rb

    def classes(self, members):
        out: dict = {}
        for m in members:
            out.setdefault(self.find(m), []).append(m)
        return out


def det(M) -> Fraction:
    """Exact determinant of a small rational matrix (Laplace expansion)."""
    n = len(M)
    if n == 1:
        return Fraction(M[0][0])
    total = Fraction(0)
    for j in range(n):
        minor = [row[:j] + row[j + 1 :] for row in M[1:]]
        total += (-1) ** j * Fraction(M[0][j]) * det(minor)
    return total


def build(h: int, recs, rational: bool):
    """Union-find over manifolds, closed under identical data + known maps."""
    uf = UnionFind()
    by_mpt = {tuple(r["mpt"]): r for r in recs}
    by_pt: dict = {}
    for r in recs:
        by_pt.setdefault((r["mpt"][1], r["mpt"][2]), tuple(r["mpt"]))

    # Numerically identical Wall data is trivially equivalent (P = identity).
    by_digest: dict = {}
    for r in recs:
        by_digest.setdefault(r["digest"], []).append(tuple(r["mpt"]))
    for group in by_digest.values():
        for m in group[1:]:
            uf.union(group[0], m)

    dets = []
    for e in ksdata.load_transformations(h, rational):
        if e["kind"] == "mpt":
            a, b = e["a"], e["b"]
        else:
            a, b = by_pt.get(e["a"]), by_pt.get(e["b"])
        if a not in by_mpt or b not in by_mpt:
            continue
        for P, recorded in e["maps"]:
            d = det(P)
            dets.append((d, Fraction(recorded), a, b, P))
        uf.union(a, b)
    return uf, dets


def main() -> int:
    h = 3
    recs = [r for r in ksdata.load_manifolds(h) if r["simply_connected"]]
    members = [tuple(r["mpt"]) for r in recs]
    by_mpt = {tuple(r["mpt"]): r for r in recs}

    uf_z, dets_z = build(h, recs, rational=False)
    uf_q, dets_q = build(h, recs, rational=True)
    cls_z = uf_z.classes(members)
    cls_q = uf_q.classes(members)

    # --- determinants of the known rational equivalences --------------------
    det_values: dict = {}
    mismatch = 0
    non_integral = 0
    for d, recorded, a, b, P in dets_q:
        det_values[str(d)] = det_values.get(str(d), 0) + 1
        if d != recorded:
            mismatch += 1
        if any(Fraction(x).denominator != 1 for row in P for x in row):
            non_integral += 1

    # --- which integral classes merged under Q ------------------------------
    z_rep_of = {m: uf_z.find(m) for m in members}
    merged = []
    for qroot, group in cls_q.items():
        zclasses = sorted({z_rep_of[m] for m in group})
        if len(zclasses) > 1:
            merged.append((qroot, group, zclasses))

    n_merge_events = sum(len(z) - 1 for _, _, z in merged)

    # --- nullcone test ------------------------------------------------------
    detail = []
    on_nullcone = off_nullcone = 0
    inconsistent = 0
    for qroot, group, zclasses in merged:
        i4 = {by_mpt[m][I4_KEY] for m in group}
        i6 = {by_mpt[m][I6_KEY] for m in group}
        if len(i4) > 1 or len(i6) > 1:
            inconsistent += 1
        null = i4 == {0} and i6 == {0}
        if null:
            on_nullcone += 1
        else:
            off_nullcone += 1
        detail.append(
            {
                "n_integral_classes_merged": len(zclasses),
                "n_manifolds": len(group),
                "I4": sorted(i4),
                "I6": sorted(i6),
                "on_nullcone": null,
                "example_mpts": [list(m) for m in sorted(group)[:4]],
            }
        )

    res = {
        "h11": h,
        "n_manifolds_simply_connected": len(members),
        "n_classes_GL3Z": len(cls_z),
        "n_classes_GL3Q": len(cls_q),
        "n_merge_events": n_merge_events,
        "n_merged_Q_classes": len(merged),
        "rational_maps": {
            "n_matrices": len(dets_q),
            "n_with_non_integer_entries": non_integral,
            "recomputed_determinants": det_values,
            "recorded_vs_recomputed_mismatches": mismatch,
        },
        "nullcone": {
            "merged_classes_on_nullcone": on_nullcone,
            "merged_classes_off_nullcone": off_nullcone,
            "merged_classes_with_unequal_invariants": inconsistent,
        },
        "merged_classes": detail,
    }

    print(f"h^1,1 = 3: {len(members)} simply-connected FRSTs")
    print(f"  GL(3,Z) classes: {len(cls_z)}      GL(3,Q) classes: {len(cls_q)}")
    print(
        f"  merge events: {n_merge_events}  "
        f"(across {len(merged)} rational classes that absorbed >1 integral class)"
    )
    print()
    print(f"  rational maps: {len(dets_q)} matrices, "
          f"{non_integral} with non-integer entries")
    print(f"    determinants found: {det_values}")
    print(f"    recorded vs recomputed mismatches: {mismatch}")
    print()
    print(f"  merged classes ON the nullcone (I4 = I6 = 0):  {on_nullcone}")
    print(f"  merged classes OFF the nullcone:               {off_nullcone}")
    print(f"  merged classes with unequal invariants:        {inconsistent}")
    print()
    for d in detail:
        flag = "nullcone" if d["on_nullcone"] else "OFF-nullcone"
        print(
            f"    {d['n_integral_classes_merged']} Z-classes, "
            f"{d['n_manifolds']:>3} manifolds  I4={d['I4']} I6={d['I6']}  {flag}"
        )

    out = os.path.join(ksdata.REPO, "results")
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "h3_nullcone.json"), "w") as fh:
        json.dump(res, fh, indent=2)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
