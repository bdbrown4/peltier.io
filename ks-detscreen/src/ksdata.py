"""Loaders for the CCFHL Kreuzer-Skarke equivalence data (arXiv:2310.05909).

Data source: http://www-thphys.physics.ox.ac.uk/projects/CalabiYau/KSEquiv
Fetch with src/fetch_data.sh, which also verifies data/SHA256SUMS.

Two formats are handled:

  ManifoldData/H11is<h>DataAndInvariants.json
      A JSON list of records, one per favourable FRST, carrying the triple
      intersection form, c2, the Hodge numbers and the evaluated invariants.
      Records are streamed one at a time: the h=5 file is 101 MB and only a
      few scalar fields per record are ever needed.

  EquivalenceData/EquivalenceDataPicard<h>upto<k>.txt
      Mathematica source, not JSON. Three sections keyed by string:
        "SameInvClasses/UniqueManifoldsWithEntriesin<k>" -> {class, class, ...}
        "Assoc of transformation matrices"               -> <|pair -> {{P,det}}|>
        "Duplicate PolyTriangs Assoc"                    -> <|mpt -> {mpt,...}|>
      Manifolds are identified throughout by the triple
      (ManifoldIndex, polytope, triangulation number), written {m, p, t}.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
from fractions import Fraction

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RAW = os.path.join(REPO, "data", "raw")
CACHE = os.path.join(REPO, "data", "cache")

# The degree-2h PDET invariant of Eq. (II.9), under the name it carries in the
# published JSON. Its degree is 2h for every h, hence its weight is 3*(2h)/h = 6.
TWO_H_INVARIANT_KEY = {
    2: "2h-invariant-quartic",
    3: "2h-invariant-sextic",
    4: "2h-invariant-octic",
    5: "2h-invariant-decic",
}

# Lowest-degree invariant of the trilinear form, from Table 1 of the paper.
# For h = 4, 5 this coincides with the degree-2h invariant above.
LOWEST_DEGREE = {2: 4, 3: 4, 4: 8, 5: 10}

# Additional invariants of known weight, used only as cross-checks.
#   key -> (degree in d, degree in c2)  =>  weight (3*a + b)/h
MIXED_INVARIANTS = {
    5: {"CEpsDeg8Invariant": (8, 6)},
}


def manifold_path(h: int) -> str:
    return os.path.join(RAW, "ManifoldData", f"H11is{h}DataAndInvariants.json")


def stream_records(path: str):
    """Yield each top-level JSON object in a large JSON array, one at a time."""
    dec = json.JSONDecoder()
    with open(path, encoding="utf-8") as fh:
        s = fh.read()
    i = s.index("[") + 1
    n = len(s)
    while True:
        while i < n and s[i] in " \t\r\n,":
            i += 1
        if i >= n or s[i] == "]":
            return
        obj, i = dec.raw_decode(s, i)
        yield obj


def wall_data_digest(rec: dict) -> str:
    """A stable digest of the numerical Wall data (d_ijk, c_2) of one record.

    Used to collapse FRSTs whose topological data is numerically identical --
    the paper's "Duplicate PolyTriangs". Two records with the same digest are
    the same point of Wall data, so they carry no information about each other.
    """
    payload = json.dumps(
        [rec["intersec"], rec["c2"]], separators=(",", ":"), sort_keys=True
    )
    return hashlib.blake2b(payload.encode(), digest_size=16).hexdigest()


def load_manifolds(h: int, use_cache: bool = True) -> list[dict]:
    """Projected manifold records for Picard number h.

    Returns one dict per FRST with keys: mpt, h11, h12, simply_connected,
    digest, and every scalar invariant of known weight for this h.
    """
    os.makedirs(CACHE, exist_ok=True)
    cache = os.path.join(CACHE, f"h{h}_projected.json")
    src = manifold_path(h)
    if use_cache and os.path.exists(cache):
        if os.path.getmtime(cache) >= os.path.getmtime(src):
            with open(cache, encoding="utf-8") as fh:
                return json.load(fh)

    inv_keys = [TWO_H_INVARIANT_KEY[h]]
    inv_keys += list(MIXED_INVARIANTS.get(h, {}))
    if h == 3:
        inv_keys.append("QuarticInv")

    out = []
    for rec in stream_records(src):
        row = {
            "mpt": [
                rec["ManifoldIndex"],
                rec["polytope"],
                rec["triangulation number"],
            ],
            "h11": rec["h11"],
            "h12": rec["h12"],
            "simply_connected": bool(rec["SimplyConnected"]),
            "digest": wall_data_digest(rec),
        }
        for k in inv_keys:
            row[k] = rec.get(k)
        out.append(row)

    with open(cache, "w", encoding="utf-8") as fh:
        json.dump(out, fh)
    return out


# ------------------------------------------------- Mathematica list parsing


def _parse_nested(s: str, i: int):
    """Parse a Mathematica brace-list of numbers into nested Python lists.

    Entries are ints, or Fractions where the file writes a rational such as
    3/2 -- the GL(h, Q) files contain genuinely non-integral matrices.
    """
    assert s[i] == "{"
    i += 1
    out: list = []
    while True:
        while s[i] in " \t\r\n,":
            i += 1
        if s[i] == "}":
            return out, i + 1
        if s[i] == "{":
            v, i = _parse_nested(s, i)
            out.append(v)
        else:
            j = i
            while s[j] not in ",}":
                j += 1
            tok = s[i:j].strip()
            out.append(Fraction(tok) if "/" in tok else int(tok))
            i = j


def equivalence_path(h: int, rational: bool = False) -> str:
    d = os.path.join(RAW, "EquivalenceData")
    if rational:
        return os.path.join(d, f"GLNQEquivalenceDataPicard{h}.txt")
    hits = [f for f in os.listdir(d) if re.match(rf"EquivalenceDataPicard{h}\D", f)]
    if not hits:
        raise FileNotFoundError(f"no equivalence file for h={h} in {d}")
    return os.path.join(d, sorted(hits)[0])


def load_same_inv_classes(h: int, rational: bool = False) -> list[list[list[int]]]:
    """The list of classes of FRSTs sharing all evaluated invariants.

    Each class is a list of {m, p, t} triples. A class with more than one
    member is a set of manifolds the paper could not separate by invariants
    and could not link by an explicit GL(h, Z) transformation.
    """
    path = equivalence_path(h, rational)
    with open(path, encoding="utf-8") as fh:
        s = fh.read()
    m = re.search(r'"SameInvClasses[^"]*"\s*->\s*', s)
    if not m:
        raise ValueError(f"no SameInvClasses section in {path}")
    classes, _ = _parse_nested(s, s.index("{", m.end()))
    return classes


def _association_items(s: str, after: int):
    """Yield (key, value) pairs of a Mathematica association.

    The published files use two spellings for the same thing: `<| k -> v |>`
    (h = 4, 5 and the h = 3 rational file) and plain `{ k -> v }` (the h = 3
    integral file). Both are accepted. Keys are either a nested brace-list or
    a quoted string; values are always nested brace-lists.
    """
    i = after
    while s[i] in " \t\r\n":
        i += 1
    if s.startswith("<|", i):
        i += 2
        closer = "|"
    elif s[i] == "{":
        i += 1
        closer = "}"
    else:
        raise ValueError(f"not an association at offset {i}: {s[i:i + 40]!r}")
    while True:
        while i < len(s) and s[i] in " \t\r\n,":
            i += 1
        if i >= len(s) or s[i] == closer:
            return
        if s[i] == '"':
            j = s.index('"', i + 1)
            key = s[i + 1 : j]
            i = j + 1
        else:
            key, i = _parse_nested(s, i)
        i = s.index("->", i) + 2
        while s[i] in " \t\r\n":
            i += 1
        val, i = _parse_nested(s, i)
        yield key, val


def load_transformations(h: int, rational: bool = False) -> list[dict]:
    """The explicitly found basis transformations, as a list of entries.

    Two key forms occur in the published files and both are returned:
      kind "mpt": a = (m, p, t) triples          (h = 4 uses this throughout)
      kind "pt" : a = (polytope, triangulation)  (h = 5 uses this for all but
                  22 of its 5352 entries, written as the string "{p,t}-{p,t}")
    Each entry carries maps = [(P, det P), ...].
    """
    path = equivalence_path(h, rational)
    with open(path, encoding="utf-8") as fh:
        s = fh.read()
    m = re.search(r'"Assoc of transformation matrices"\s*->\s*', s)
    if not m:
        return []
    out: list[dict] = []
    for key, val in _association_items(s, m.end()):
        if isinstance(key, str):
            lhs, rhs = key.split("}-{")
            a = tuple(int(x) for x in lhs.strip("{}").split(","))
            b = tuple(int(x) for x in rhs.strip("{}").split(","))
            kind = "pt"
        else:
            a, b = tuple(key[0]), tuple(key[1])
            kind = "mpt"
        out.append(
            {"kind": kind, "a": a, "b": b, "maps": [(v[0], v[1]) for v in val]}
        )
    return out


def load_duplicates(h: int, rational: bool = False) -> dict:
    """{mpt: [mpt, ...]} FRSTs whose topological data is numerically identical."""
    path = equivalence_path(h, rational)
    with open(path, encoding="utf-8") as fh:
        s = fh.read()
    m = re.search(r'"Duplicate PolyTriangs Assoc"\s*->\s*', s)
    if not m:
        return {}
    out: dict = {}
    for key, val in _association_items(s, m.end()):
        out[tuple(key)] = [tuple(v) for v in val]
    return out


# ------------------------------------------------------------- universes


def build_universe(h: int, universe: str) -> list[dict]:
    """Select the set of manifolds the screen runs over.

    all      every simply-connected favourable FRST in the published data
    distinct one representative per numerically distinct (d_ijk, c_2)
    unique   the paper's own list of manifolds that remain unseparated after
             invariants and explicit GL(h, Z) searches -- i.e. every FRST
             named in SameInvClasses (see docs/ALGORITHM.md for why the
             choice of universe changes the pair counts but not the verdict)
    """
    recs = [r for r in load_manifolds(h) if r["simply_connected"]]
    if universe == "all":
        return recs
    if universe == "distinct":
        seen: dict[str, dict] = {}
        for r in recs:
            seen.setdefault(r["digest"], r)
        return list(seen.values())
    if universe == "unique":
        index = {tuple(r["mpt"]): r for r in recs}
        allrecs = {tuple(r["mpt"]): r for r in load_manifolds(h)}
        wanted = [tuple(x) for cls in load_same_inv_classes(h) for x in cls]
        unknown = [k for k in wanted if k not in allrecs]
        if unknown:
            raise ValueError(f"{len(unknown)} listed manifolds absent from ManifoldData")
        # Data anomaly: the paper states it considered only SimplyConnected=True,
        # but a few entries of its own SameInvClasses list are flagged False.
        # Applying the stated scope uniformly and recording what that drops.
        dropped = [k for k in wanted if k not in index]
        build_universe.dropped_not_simply_connected = dropped
        return [index[k] for k in wanted if k in index]
    raise ValueError(f"unknown universe {universe!r}")


build_universe.dropped_not_simply_connected = []
