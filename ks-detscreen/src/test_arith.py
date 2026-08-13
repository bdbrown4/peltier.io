#!/usr/bin/env python3
"""Self-tests for the exact arithmetic. Run: python3 src/test_arith.py

The screen's whole claim rests on one equivalence:

    residue_signature(a, w) == residue_signature(b, w)
        <=>  a/b is exactly the w-th power of a rational

The first side is computed by factoring; the second by integer root extraction.
They share no code path, so agreeing on randomised input is real evidence.
"""

from __future__ import annotations

import os
import random
import sys
from fractions import Fraction

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import arith  # noqa: E402

fails = 0


def check(cond, msg):
    global fails
    if not cond:
        fails += 1
        print(f"FAIL: {msg}")


# ---- integer roots --------------------------------------------------------
for n in list(range(0, 200)) + [10**12, 2**64, 3**30 - 1, 3**30]:
    for k in (1, 2, 3, 6, 7):
        r = arith.iroot(n, k)
        check(r**k <= n < (r + 1) ** k, f"iroot({n},{k}) = {r}")

check(arith.is_perfect_power(64, 6), "64 = 2^6")
check(not arith.is_perfect_power(65, 6), "65 not a 6th power")
check(arith.is_perfect_power(729, 6), "729 = 3^6")
check(arith.exact_root(4096, 6) == 4, "4096^(1/6) = 4")
check(arith.exact_root(4097, 6) is None, "4097 has no exact 6th root")

# ---- factorisation --------------------------------------------------------
for n in [1, 2, 512, -512, 1166400, 19712896, 9818076672, 2**20 * 3**7 * 5**3]:
    f = arith.factorise(n)
    prod = 1
    for p, e in f.items():
        check(arith.factorise(p) == {p: 1}, f"{p} prime")
        prod *= p**e
    check(prod == abs(n), f"factorise({n}) reconstructs")

rng = random.Random(20260813)
for _ in range(300):
    n = rng.randrange(2, 10**9)
    f = arith.factorise(n)
    prod = 1
    for p, e in f.items():
        prod *= p**e
    check(prod == n, f"factorise({n}) reconstructs")

# ---- the equivalence the screen depends on --------------------------------
W = 6
vals = [rng.randrange(-10**7, 10**7) for _ in range(400)]
vals = [v for v in vals if v != 0]

# Families whose members differ by exact sixth powers. Random pairs almost
# never land on a positive case, so the positive branch is driven explicitly:
# every within-family pair must be accepted, every cross-family pair rejected
# unless the cores coincide.
families = [[s * m**W for m in (1, 2, 3, 4, 5)] for s in (7, -7, 12, -512, 1024, 3, -3)]
for fam in families:
    vals.extend(fam)

pairs = [(a, b) for fam in families for a in fam for b in fam if a != b]
pairs += [(rng.choice(vals), rng.choice(vals)) for _ in range(4000)]

agree = disagree = positive = 0
for a, b in pairs:
    sig = arith.residue_signature(a, W) == arith.residue_signature(b, W)
    exact = arith.ratio_is_wth_power(a, b, W)
    if sig == exact:
        agree += 1
    else:
        disagree += 1
        print(f"FAIL: a={a} b={b} signature={sig} exact={exact}")
    if exact:
        positive += 1
check(disagree == 0, f"{disagree} signature/exact-test disagreements")
check(positive > 100, f"only {positive} positive cases exercised")

# ---- the root actually is the claimed |det P| -----------------------------
for _ in range(500):
    a, b = rng.choice(vals), rng.choice(vals)
    r = arith.wth_root_of_ratio(b, a, W)
    if r is not None:
        check(r**W == Fraction(b, a), f"({b}/{a})^(1/{W}) = {r} but {r}^{W} != ratio")
        check(r > 0, f"root {r} should be positive")

# ---- sign handling: w even forbids a negative ratio -----------------------
check(not arith.ratio_is_wth_power(-64, 1, 6), "-64 is not a 6th power")
check(arith.ratio_is_wth_power(-32, 1, 5), "-32 = (-2)^5")
check(
    arith.residue_signature(-512, 6) != arith.residue_signature(512, 6),
    "sign is part of the signature for even w",
)

print("FAILURES:", fails) if fails else print("all arithmetic self-tests passed")
raise SystemExit(1 if fails else 0)
