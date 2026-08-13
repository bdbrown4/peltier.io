# Findings

Run date 2026-08-13. Data fetched the same day from
`www-thphys.physics.ox.ac.uk/projects/CalabiYau/KSEquiv`; checksums in
`data/SHA256SUMS`. Everything below is reproducible with
`src/fetch_data.sh && python3 src/det_screen.py --all && python3 src/h3_nullcone.py`.

Read `docs/ALGORITHM.md` first — the screen's logic, and in particular what it
does *not* prove, is stated there.

---

## 0. Headline

- **The screen speaks about most of the space.** The silent set — pairs where
  the invariant vanishes on both sides and the screen says nothing — is
  **1.18%** of same-Hodge pairs at `h = 4` and **0.70%** at `h = 5` (paper's
  `unique` universe; 5.4% / 3.1% in the widest universe). Vanishing is
  *informative* rather than silent whenever it is one-sided: a further 17.6%
  (`h = 4`) and 14.8% (`h = 5`) of pairs are **excluded outright** because one
  invariant vanishes and the other does not.
- **`h = 5` produces no surviving falsifier candidate.** The primary screen
  leaves 167 candidate value-pairs; a second, independent weight-6 invariant in
  the same published data (`CEpsDeg8Invariant`) kills **all 167**.
- **`h = 4` leaves 13 candidate value-pairs unrefuted** (24 manifold pairs in
  the `unique` universe), with forced `|det P| ∈ {1/2, 2, 1/3, 3, 2/3, 3/2, 3/4}`.
  Nothing in the published `h = 4` data can refute them, because no second
  invariant of known weight was published at `h = 4`. These are candidates, not
  counterexamples.
- **The `h = 3` mergers do *not* sit on the nullcone.** Of the 22 rational
  classes that absorb more than one integral class, **17 are off the nullcone
  and only 5 are on it**. For those 17, `|det P| = 1` is *forced* by the
  measured invariants — it is a theorem there, not a coincidence. The
  coincidence question survives only on the 5 nullcone classes.

---

## 1. The screen (task 1: h = 4 and h = 5)

`δ = 8` at `h = 4` (`2h-invariant-octic`), `δ = 10` at `h = 5`
(`2h-invariant-decic`), both weight `w = 6`. Values taken from the published
data; none recomputed.

| h | universe | manifolds | I = 0 | factorisations | same-Hodge pairs |
|---|----------|-----------|-------|----------------|------------------|
| 4 | unique | 1183 | 103 | 634 | 24,413 |
| 4 | distinct | 1948 | 288 | 634 | 70,401 |
| 4 | all | 5324 | 767 | 634 | 558,551 |
| 5 | unique | 8023 | 638 | 5168 | 1,019,394 |
| 5 | distinct | 13,330 | 1638 | 5168 | 2,782,233 |
| 5 | all | 56,714 | 7460 | 5168 | 51,651,614 |

The reduction the brief predicted holds: **634 and 5168 factorisations** — one
per distinct non-zero invariant value — cover up to 51.6 million pairs.

### The silent set (task 3)

"Invariant equal to zero" splits into two cases that behave oppositely, so both
are reported. A pair with *one* vanishing invariant is not silent: since
`I' = (det P)^6 I`, vanishing is preserved, so such a pair is **excluded** from
rational equivalence altogether.

| h | universe | silent (both I = 0) | excluded (exactly one I = 0) | screened (both I ≠ 0) |
|---|----------|--------------------|------------------------------|----------------------|
| 4 | unique | 289 (**1.18%**) | 4,306 (17.64%) | 19,818 (81.18%) |
| 4 | distinct | 3,776 (5.36%) | 19,018 (27.01%) | 47,607 (67.62%) |
| 4 | all | 29,869 (5.35%) | 169,103 (30.28%) | 359,579 (64.38%) |
| 5 | unique | 7,123 (**0.70%**) | 150,433 (14.76%) | 861,838 (84.54%) |
| 5 | distinct | 82,779 (2.98%) | 655,250 (23.55%) | 2,044,204 (73.47%) |
| 5 | all | 1,575,197 (3.05%) | 13,468,807 (26.08%) | 36,607,610 (70.87%) |

At the manifold level the invariant vanishes for 767/5324 (14.4%) of `h = 4`
FRSTs and 7460/56714 (13.2%) of `h = 5` FRSTs.

### Falsifier candidates (task 4)

Of the screened pairs, the overwhelming majority are excluded by signature
(19,537 of 19,818 at `h = 4` unique; 860,201 of 861,838 at `h = 5` unique). The
remainder split into pairs with *identical* invariants (ratio 1, consistent
with `|det P| = 1`) and the candidates.

| h | universe | same invariant | excluded by signature | candidates (manifold pairs) | candidate value-pairs |
|---|----------|----------------|----------------------|------------------------------|----------------------|
| 4 | unique | 257 | 19,537 | 24 | 13 |
| 4 | distinct | 1,772 | 45,763 | 72 | 13 |
| 4 | all | 25,779 | 333,401 | 399 | 13 |
| 5 | unique | 1,199 | 860,201 | 438 | 167 |
| 5 | distinct | 18,172 | 2,023,908 | 2,124 | 167 |
| 5 | all | 820,049 | 35,740,144 | 47,417 | 167 |

The candidate **value**-pairs are identical across universes; only how many
manifold pairs realise them changes.

**The 13 `h = 4` candidates, in full** (`unique` universe; `|det P|` is the
exact sixth root of the ratio, and is what a rational equivalence between the
pair would be forced to have):

| h^1,2 | I(X) | I(X') | ratio | forced abs det P | manifold pairs |
|------|------|-------|-------|------------------|----------------|
| 52 | −131072 | −23328 | 729/4096 | 3/4 | 3 |
| 56 | −16384 | −256 | 1/64 | 1/2 | 2 |
| 58 | −5832 | −8 | 1/729 | 1/3 | 2 |
| 60 | −4096 | −64 | 1/64 | 1/2 | 1 |
| 64 | −46656 | −4096 | 64/729 | 2/3 | 1 |
| 68 | −200448 | −3132 | 1/64 | 1/2 | 1 |
| 68 | −52488 | −4608 | 64/729 | 2/3 | 2 |
| 70 | 9 | 6561 | 729 | 3 | 1 |
| 70 | −23328 | −32 | 1/729 | 1/3 | 1 |
| 72 | −16384 | −256 | 1/64 | 1/2 | 4 |
| 76 | 1024 | 11664 | 729/64 | 3/2 | 1 |
| 106 | 4 | 256 | 64 | 2 | 1 |
| 214 | −512 | −8 | 1/64 | 1/2 | 4 |

Full records, with example `{m, p, t}` indices for each side, are in
`results/screen_h4_unique.json`.

At `h = 5` the 167 candidates carry forced `|det P|` values dominated by 1/2
(75 cases), 2/3 (22), 3/4 (14), 2 (13), 3/2 (12), 1/3 (11), with singletons out
to 2/5 and 4. **All 167 are killed** by the `CEpsDeg8Invariant` cross-check
(§4 below): 0 survive, 0 undecidable.

---

## 2. The h = 3 mergers and the nullcone (task 2)

`h = 3` is the only Picard number where the published data contains *genuine*
rational equivalences, obtained by symbolic solution rather than bounded
integer search. Reconstructing the class structure by union-find over identical
Wall data plus the published maps reproduces the paper's counts exactly:

- **183** `GL(3,Z)` classes, **150** `GL(3,Q)` classes among 517
  simply-connected FRSTs → **33 merge events**, distributed across **22**
  rational classes that absorb more than one integral class.

**Determinants, recomputed rather than trusted.** The rational file holds 313
matrices, of which 291 involve two simply-connected manifolds. **167 have
genuinely non-integer entries** (e.g. `3/2`, `7/4`, `2/3`). Every determinant
recomputed by exact Laplace expansion is `±1`, and agrees with the value the
file records, in **all 313** cases (0 mismatches). So non-integrality is real
and common; determinant ≠ ±1 never occurs.

**Nullcone test.** For `h = 3` the invariants are `I_4` (`QuarticInv`, weight 4)
and `I_6` (`2h-invariant-sextic`, weight 6). The nullcone is `I_4 = I_6 = 0`.

| | count |
|---|---|
| merged rational classes **off** the nullcone | **17** |
| merged rational classes **on** the nullcone | **5** |
| merged classes with unequal invariants across the merger | 0 |

**So the answer to the second task is no — the mergers do not sit on the
nullcone.** This matters more than it looks. For each of the 17 off-nullcone
classes, the merged manifolds have equal *and non-zero* invariants, so
`I' = (det P)^w I` with `I = I' ≠ 0` gives `(det P)^w = 1`, hence `|det P| = 1`.
That is a derivation from measured invariant values, independent of anyone
having inspected `P`. On those classes `|det P| = 1` is a **theorem**.

The five nullcone classes are where the question genuinely remains open: there
the invariants impose no constraint on `det P` whatsoever, and `|det P| = 1` is
observed only from the explicit matrices. Sizes: 3 integral classes / 15
manifolds, 2/4, 3/22, 2/10, 3/17. If a rational equivalence with `|det P| ≠ 1`
exists anywhere in this dataset, the nullcone is where it lives — and by
construction the residue-signature screen is blind there.

---

## 3. Validation performed

These are checks that could have failed and did not.

1. **The invariants really are invariants.** Every pair linked by an explicit
   transformation in the published data must have *equal* invariants (weight 6,
   `det = ±1`, so the factor is `(±1)^6 = 1`). Checked: **1655** linked pairs at
   `h = 4` (0 mismatches on the octic) and **5352** at `h = 5` (0 mismatches on
   the decic *and* 0 on `CEpsDeg8Invariant`). All recorded determinants ±1.
2. **The signature hash is exactly the sixth-power predicate.** Every value-pair
   within every Hodge class was re-tested with a factorisation-free method
   (reduce the ratio, test numerator and denominator for exact sixth powers):
   **13,525** value-pairs at `h = 4` and **671,887** at `h = 5`, with **0
   disagreements**. The screen aborts on any disagreement.
3. **Arithmetic self-tests.** `python3 src/test_arith.py` — integer roots,
   factorisation round-trips, sign handling, and ~4,100 randomised
   signature-vs-exact-test comparisons including explicitly constructed
   sixth-power families. All pass.
4. **h = 3 determinants recomputed** from the matrices by exact Laplace
   expansion, matching the file in all 313 cases.

---

## 4. Where the h = 5 candidates died

`CEpsDeg8Invariant` is `I_(8,6),5(d, c_2)` — degree 8 in `d_ijk`, degree 6 in
`c_2,i` — so its weight is `(3·8 + 6)/5 = 6`, identical to the decic's. A
rational equivalence must scale both by the same `(det P)^6`, so the two ratios
must agree. They never do. Examples:

| h^1,2 | decic ratio | CEps values | verdict |
|------|-------------|-------------|---------|
| 35 | 1/64 | −16794790133760 → −37201969152 | ratio ≈ 1/451, not 1/64 |
| 41 | 1/64 | −322645524480 → +2135754915840 | sign flips; `(det P)^6 > 0` |
| 47 | 64 | −20283136671744 → −4617694347264 | ratio < 1, not 64 |

This is a real filter, not a degenerate one: `CEpsDeg8Invariant` vanishes for
only 570 of 56,714 `h = 5` manifolds.

---

## 5. Discrepancies and data anomalies

**The expected pair counts could not be reproduced.** The brief anticipated
21,098 (`h = 4`) and 779,571 (`h = 5`) same-Hodge pairs. No convention tried
yields both:

| convention | h = 4 | h = 5 |
|---|---|---|
| all simply-connected FRSTs | 558,551 | 51,651,614 |
| distinct Wall data | 70,401 | 2,782,233 |
| `SameInvClasses`, all listed | 24,417 | 1,019,394 |
| `SameInvClasses`, SC only — **used here** | 24,413 | 1,019,394 |
| one representative per class | 21,433 | 917,121 |
| one representative per class, SC only | 21,430 | 917,121 |
| one rep per class, SC, `I ≠ 0` | 17,894 | 791,718 |
| **brief expected** | **21,098** | **779,571** |

"One rep per class, SC" lands within 1.6% at `h = 4` but is 17.6% off at
`h = 5`; "one rep per class, SC, `I ≠ 0`" is within 1.6% at `h = 5` but 15% off
at `h = 4`. Since the counts drive nothing in the conclusion — the candidate
value-pairs are identical in every universe — this is recorded rather than
resolved. If the earlier session's convention is recoverable it should be
compared directly.

**Anomalies in the published data**, all recorded rather than silently absorbed:

- The paper states it considered only `SimplyConnected = True`, but 3 of the
  1186 entries of its own `h = 4` `SameInvClasses` list are flagged `False`
  (`{1,0,0}`, `{21,3,0}`, `{54,12,0}`). The stated scope is applied uniformly
  here, dropping those 3; they are listed in every result JSON under
  `dropped_not_simply_connected`. Their invariants are duplicated by other
  manifolds in the same Hodge class, so nothing structural changes. The same
  situation occurs in the `h = 3` lists (3 of 186).
- `Table 3` of the paper lists `I(4,4),4MixedInvariant` as evaluated for
  `h = 4`, but no such key exists in the published `h = 4` JSON — all 5330
  records share one identical key set. This is exactly the invariant that would
  have done at `h = 4` what `CEpsDeg8Invariant` did at `h = 5`.
- The three files use three different serialisation conventions for the same
  structures: `h = 4` keys transformations by `{m,p,t}` triples, `h = 5` keys
  5330 of its 5352 entries by the string `"{p,t}-{p,t}"`, and the `h = 3`
  integral file writes its association as `{...}` where every other file uses
  `<|...|>`. All three are handled in `src/ksdata.py`.

---

## 6. Ledger

**Established** (in the published literature, and independently reconfirmed here)

- Wall's theorem: `(h^1,1, h^1,2, d_ijk, c_2)` determines the diffeomorphism
  class of a simply-connected CY threefold with torsion-free cohomology.
- The transformation law `I(d') = (det P)^(3δ/h) I(d)` — stated in the paper's
  Appendix B, and independently *forced* by homogeneity plus `SL`-invariance
  (`docs/ALGORITHM.md` §2), so it is not taken on trust.
- `h = 3` has exactly 183 `GL(3,Z)` classes and 150 `GL(3,Q)` classes.
  Reproduced here by union-find from the raw files.
- **The `|det P| = ±1` observation is the paper's own**, stated explicitly:
  "all the `GL(h,Q)` transformations have determinant `±1`", called "somewhat
  surprising", with the caveat that "we should really only trust relative
  invariants, which exist for `h = 3` as `I_4^3/I_6^2`". It was *not* imposed
  as a constraint: the `h ≤ 3` solve used Mathematica's `Reduce` on Eq. (I.3)
  "only demanding that the Hodge numbers should be equal". So the ±1 is an
  output of that computation, which is what makes it evidence at all.

**Folklore** (widely assumed, not proven, and the target of this work)

- That `|det P| = 1` in *every* rational equivalence between CY Wall data, at
  every Picard number. The evidence for it is: `h ≤ 3` exhaustively (313
  matrices), plus bounded integer searches at `h = 4, 5, 6` which cannot see a
  non-unimodular map even in principle, since they search `GL(h, Z)`.

**New here**

- The weight is `6` for the degree-`2h` invariant at *every* `h`, since
  `w = 3(2h)/h`. The `w = 6` used at `h = 4, 5` is structural, not a numerical
  accident of those two cases.
- Quantification of the silent set: 1.18% (`h = 4`) and 0.70% (`h = 5`) of
  same-Hodge pairs are beyond the screen's reach; a further 17.6% / 14.8% are
  *excluded* by one-sided vanishing.
- 13 falsifier candidates at `h = 4` and 167 at `h = 5`, each with its exact
  forced `|det P|`.
- All 167 `h = 5` candidates are eliminated by a second weight-6 invariant
  already present in the published data — an application of `CEpsDeg8Invariant`
  as a rational-equivalence screen that the paper does not make.
- **The 33 `h = 3` mergers are 17-off / 5-on the nullcone**, so for the majority
  `|det P| = 1` is a consequence of the measured invariants rather than a
  coincidence; the coincidence question is confined to the 5 nullcone classes.

**Taken on trust** (not verified here, and each one is a place an error could hide)

- The invariant values in `ManifoldData` are correct as published. They were
  read, never recomputed, exactly as the brief specified. The internal
  consistency check available — equality across all 7,007 explicitly linked
  pairs — passes, but that would not catch a systematic error in the invariant
  evaluation itself.
- `CEpsDeg8Invariant` is `I_(8,6),5(d, c_2)` with degrees (8 in `d`, 6 in
  `c_2`) as read from Table 3. The alternative reading — (degree 8, weight 6) —
  is arithmetically impossible (it needs 11 factors of `d` in a degree-8
  polynomial), and the (10,6) entry for `h = 6` gives weight 6 under the same
  reading, so the interpretation is well-supported but is an interpretation.
- That the `SameInvClasses` lists and the transformation associations are
  complete as published; the union-find reconstruction agrees with the paper's
  published class counts, which is good evidence but not proof.
- That `cytools`' FRST enumeration and the `SimplyConnected` flags are correct.

---

## 7. What would settle it

The `h = 4` candidates are the live end. Two routes, in order of cost:

1. **Evaluate a second `h = 4` invariant of known weight** on the ~26 manifolds
   named in the 13 candidate value-pairs, and check its ratio against the
   octic's. `I_(4,4),4(d, c_2)` (weight `(12+4)/4 = 4`) is the natural choice —
   Table 3 says it was computed, but it was not published. This is cheap if the
   contraction is reconstructed, and would very likely close `h = 4` the same
   way `CEpsDeg8Invariant` closed `h = 5`.
2. **Attempt to construct `P` directly** for the surviving candidates, over `Q`,
   with `|det P|` fixed to the known forced value. A success falsifies the
   folklore outright; a symbolic proof of non-existence confirms it for those
   pairs. This is the `h ≤ 3` method that does not scale, but 13 pairs with the
   determinant already pinned is a far smaller problem than the general one.

Separately, the 5 `h = 3` nullcone classes deserve direct attention: they are a
small, fully explicit set where the invariants are known to be powerless and the
answer is already computable by the paper's own symbolic method.
