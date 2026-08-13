#!/usr/bin/env python3
"""The residue-signature determinant screen.

Question: in a rational equivalence between two Calabi-Yau Wall data sets,
is |det P| = 1 a theorem or a coincidence?

If X and X' have Wall data related by P in GL(h, Q), then a degree-delta
invariant I of the trilinear form obeys

    I(X') = (det P)^w I(X),        w = 3*delta/h                     (paper, App. B)

For the degree-2h PDET invariant of Eq. (II.9) this gives w = 3*(2h)/h = 6 for
every h. So a rational equivalence forces I(X')/I(X) to be an exact sixth power
of a rational, and |det P| = 1 forces it to be exactly 1. A pair whose ratio is
an exact sixth power *other than* 1 is therefore a falsifier candidate: if such
a pair is rationally equivalent at all, it is equivalent with |det P| != 1.

The screen is a necessary condition, never a sufficient one. It rules pairs
out; it never rules a pair in. See docs/ALGORITHM.md.

Usage:
    python3 src/det_screen.py --h 4 --universe unique
    python3 src/det_screen.py --all
"""

from __future__ import annotations

import argparse
import collections
import json
import os
import sys
from fractions import Fraction

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import arith  # noqa: E402
import ksdata  # noqa: E402

WEIGHT = 6  # of the degree-2h invariant, for every h
UNIVERSES = ("unique", "distinct", "all")


def _pairs(n: int) -> int:
    return n * (n - 1) // 2


def screen(h: int, universe: str) -> dict:
    key = ksdata.TWO_H_INVARIANT_KEY[h]
    delta = 2 * h
    assert 3 * delta % h == 0 and 3 * delta // h == WEIGHT

    ksdata.build_universe.dropped_not_simply_connected = []
    recs = ksdata.build_universe(h, universe)
    dropped = list(ksdata.build_universe.dropped_not_simply_connected)
    for r in recs:
        if r[key] is None:
            raise ValueError(f"manifold {r['mpt']} has no {key}")

    by_hodge = collections.defaultdict(list)
    for r in recs:
        by_hodge[r["h12"]].append(r)

    # ---- step 3: the silent set -------------------------------------------
    tot = zero_zero = one_zero = both_nz = 0
    for group in by_hodge.values():
        z = sum(1 for r in group if r[key] == 0)
        nz = len(group) - z
        tot += _pairs(len(group))
        zero_zero += _pairs(z)
        one_zero += z * nz
        both_nz += _pairs(nz)
    assert tot == zero_zero + one_zero + both_nz

    n_zero = sum(1 for r in recs if r[key] == 0)

    # ---- step 4: factor once per distinct value, hash by exponents mod w ---
    distinct_vals = sorted({r[key] for r in recs if r[key] != 0})
    sig_of = {v: arith.residue_signature(v, WEIGHT) for v in distinct_vals}

    buckets = collections.defaultdict(list)  # (h12, signature) -> records
    for r in recs:
        if r[key] != 0:
            buckets[(r["h12"], sig_of[r[key]])].append(r)

    candidates = []
    cand_manifold_pairs = 0
    consistent_pairs = 0  # same invariant exactly: ratio 1, |det P| = 1
    for (h12, sig), group in sorted(buckets.items(), key=lambda kv: (kv[0][0],)):
        counts = collections.Counter(r[key] for r in group)
        consistent_pairs += sum(_pairs(n) for n in counts.values())
        vals = sorted(counts)
        for i, a in enumerate(vals):
            for b in vals[i + 1 :]:
                # Independent of the signature grouping: does the ratio really
                # have an exact sixth root? Factorisation is not used here.
                root = arith.wth_root_of_ratio(b, a, WEIGHT)
                if root is None:
                    raise AssertionError(
                        f"signature bucket disagrees with the exact test: {a} vs {b}"
                    )
                n_pairs = counts[a] * counts[b]
                cand_manifold_pairs += n_pairs
                candidates.append(
                    {
                        "h12": h12,
                        "signature": _sig_str(sig),
                        "I_a": a,
                        "I_b": b,
                        "ratio_I_b_over_I_a": str(Fraction(b, a)),
                        "abs_det_P": str(root),
                        "n_manifold_pairs": n_pairs,
                        "example_a": [r["mpt"] for r in group if r[key] == a][:2],
                        "example_b": [r["mpt"] for r in group if r[key] == b][:2],
                    }
                )

    excluded_by_signature = both_nz - consistent_pairs - cand_manifold_pairs

    # ---- audit: confirm the grouping did not miss a pair -------------------
    # Exhaustive over distinct values within each Hodge class, using the
    # factorisation-free test. This is the check that the mod-w hash is doing
    # exactly what it claims, not an approximation of it.
    audit_checked = audit_disagreements = 0
    for h12, group in by_hodge.items():
        vals = sorted({r[key] for r in group if r[key] != 0})
        for i, a in enumerate(vals):
            for b in vals[i + 1 :]:
                audit_checked += 1
                same_bucket = sig_of[a] == sig_of[b]
                exact = arith.ratio_is_wth_power(b, a, WEIGHT)
                if same_bucket != exact:
                    audit_disagreements += 1

    result = {
        "h11": h,
        "delta": delta,
        "weight": WEIGHT,
        "invariant_key": key,
        "universe": universe,
        "n_manifolds": len(recs),
        "dropped_not_simply_connected": [list(k) for k in dropped],
        "n_hodge_classes": len(by_hodge),
        "n_zero_invariant": n_zero,
        "n_distinct_nonzero_values": len(distinct_vals),
        "n_factorisations": len(distinct_vals),
        "pairs": {
            "same_hodge_total": tot,
            "silent_both_zero": zero_zero,
            "excluded_exactly_one_zero": one_zero,
            "screened_both_nonzero": both_nz,
            "of_which_same_invariant": consistent_pairs,
            "of_which_excluded_by_signature": excluded_by_signature,
            "of_which_falsifier_candidates": cand_manifold_pairs,
        },
        "audit": {
            "value_pairs_checked": audit_checked,
            "disagreements_with_exact_test": audit_disagreements,
        },
        "falsifier_candidates": candidates,
    }

    cross = _cross_check(h, recs, key, candidates)
    if cross is not None:
        result["mixed_invariant_cross_check"] = cross
    return result


def _sig_str(sig) -> str:
    sign, residues = sig
    body = " * ".join(f"{p}^{e}" for p, e in residues) or "1"
    return ("-" if sign < 0 else "+") + body


def _cross_check(h: int, recs, key: str, candidates: list) -> dict | None:
    """Second, independent weight-6 invariant, where the data provides one.

    For h = 5 the published data also carries CEpsDeg8Invariant, the mixed
    invariant I_(8,6),5(d, c_2) of Table 3. Its weight is (3*8 + 6)/5 = 6, the
    same as the decic, so a rational equivalence must scale BOTH by the same
    factor (det P)^6. A candidate whose two ratios disagree is dead.
    """
    mixed = ksdata.MIXED_INVARIANTS.get(h)
    if not mixed:
        return None
    mkey, (a_deg, b_deg) = next(iter(mixed.items()))
    w2 = (3 * a_deg + b_deg) // h
    if (3 * a_deg + b_deg) % h != 0:
        return None

    by_val = collections.defaultdict(list)
    for r in recs:
        by_val[r[key]].append(r)

    survivors = killed = undecidable = 0
    notes = []
    for c in candidates:
        ma = {r[mkey] for r in by_val[c["I_a"]] if r["h12"] == c["h12"]}
        mb = {r[mkey] for r in by_val[c["I_b"]] if r["h12"] == c["h12"]}
        if None in ma or None in mb:
            undecidable += 1
            continue
        want = Fraction(c["I_b"], c["I_a"])
        ok = False
        for x in ma:
            for y in mb:
                if x == 0 and y == 0:
                    ok = True  # carries no information
                elif x == 0 or y == 0:
                    continue
                elif Fraction(y, x) == want:
                    ok = True
        if ok:
            survivors += 1
        else:
            killed += 1
            notes.append(
                {
                    "h12": c["h12"],
                    "I_a": c["I_a"],
                    "I_b": c["I_b"],
                    "decic_ratio": c["ratio_I_b_over_I_a"],
                    f"{mkey}_a": sorted(ma),
                    f"{mkey}_b": sorted(mb),
                }
            )
    return {
        "invariant_key": mkey,
        "degree_in_d": a_deg,
        "degree_in_c2": b_deg,
        "weight": w2,
        "candidates_surviving": survivors,
        "candidates_killed": killed,
        "candidates_undecidable": undecidable,
        "killed_detail": notes[:50],
    }


def report(res: dict) -> str:
    p = res["pairs"]
    tot = p["same_hodge_total"]

    def pct(n):
        # Display only, and still integer-exact: hundredths of a percent,
        # rounded half up. No float is constructed anywhere in this package.
        if tot == 0:
            return "n/a"
        hundredths = (2 * n * 10000 // tot + 1) // 2
        return f"{hundredths // 100}.{hundredths % 100:02d}%"

    L = []
    L.append(
        f"h^1,1 = {res['h11']}   delta = {res['delta']}   weight w = {res['weight']}"
        f"   universe = {res['universe']}"
    )
    L.append(f"  invariant: {res['invariant_key']}")
    L.append(
        f"  {res['n_manifolds']} manifolds over {res['n_hodge_classes']} Hodge classes; "
        f"{res['n_zero_invariant']} have I = 0"
    )
    L.append(
        f"  {res['n_factorisations']} factorisations "
        f"(one per distinct non-zero value) cover {tot} same-Hodge pairs"
    )
    L.append("")
    L.append(f"  same-Hodge pairs                     {tot}")
    L.append(
        f"    silent  (both I = 0)               {p['silent_both_zero']:>10}"
        f"   {pct(p['silent_both_zero'])}"
    )
    L.append(
        f"    excluded (exactly one I = 0)       {p['excluded_exactly_one_zero']:>10}"
        f"   {pct(p['excluded_exactly_one_zero'])}"
    )
    L.append(
        f"    screened (both I != 0)             {p['screened_both_nonzero']:>10}"
        f"   {pct(p['screened_both_nonzero'])}"
    )
    L.append(
        f"      same invariant  (|det P| = 1)    {p['of_which_same_invariant']:>10}"
    )
    L.append(
        f"      excluded by signature            {p['of_which_excluded_by_signature']:>10}"
    )
    L.append(
        f"      FALSIFIER CANDIDATES             {p['of_which_falsifier_candidates']:>10}"
    )
    L.append("")
    a = res["audit"]
    L.append(
        f"  audit: {a['value_pairs_checked']} value-pairs re-tested without "
        f"factorisation, {a['disagreements_with_exact_test']} disagreements"
    )
    cc = res.get("mixed_invariant_cross_check")
    if cc:
        L.append(
            f"  cross-check ({cc['invariant_key']}, weight {cc['weight']}): "
            f"{cc['candidates_surviving']} survive, {cc['candidates_killed']} killed, "
            f"{cc['candidates_undecidable']} undecidable"
        )
    L.append("")
    if not res["falsifier_candidates"]:
        L.append("  no falsifier candidates: every screened pair has ratio exactly 1")
    else:
        L.append(f"  {len(res['falsifier_candidates'])} candidate value-pairs:")
        for c in res["falsifier_candidates"]:
            L.append(
                f"    h12={c['h12']:<4} I={c['I_a']} vs {c['I_b']}  "
                f"ratio={c['ratio_I_b_over_I_a']}  |det P|={c['abs_det_P']}  "
                f"({c['n_manifold_pairs']} manifold pairs)  sig {c['signature']}"
            )
    return "\n".join(L)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--h", type=int, choices=(2, 3, 4, 5))
    ap.add_argument("--universe", choices=UNIVERSES, default="unique")
    ap.add_argument("--all", action="store_true", help="h=4,5 over every universe")
    ap.add_argument("--out", default=os.path.join(ksdata.REPO, "results"))
    args = ap.parse_args()

    jobs = (
        [(h, u) for h in (4, 5) for u in UNIVERSES]
        if args.all
        else [(args.h, args.universe)]
    )
    if jobs[0][0] is None:
        ap.error("pass --h or --all")

    os.makedirs(args.out, exist_ok=True)
    for h, u in jobs:
        res = screen(h, u)
        print(report(res))
        print("-" * 72)
        with open(os.path.join(args.out, f"screen_h{h}_{u}.json"), "w") as fh:
            json.dump(res, fh, indent=2)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
