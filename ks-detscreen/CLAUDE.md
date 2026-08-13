# ks-detscreen

Tests whether `|det P| = ±1` in the rational equivalences between Calabi-Yau
Wall data is a theorem or a coincidence. Data: the CCFHL Kreuzer-Skarke
equivalence files (arXiv:2310.05909), fetched by `src/fetch_data.sh` into
`data/raw/` and verified against `data/SHA256SUMS`. `data/raw/` and
`data/cache/` are not committed; everything in `results/` is.

Read `docs/ALGORITHM.md` before writing code — it states the screen precisely,
including what it does not prove. `docs/FINDINGS.md` holds the results and the
evidence ledger; read it when a question needs the history.

Exact integer arithmetic only: `int` and `Fraction`, never a float, including
in printed percentages. Never filter rational-equivalence candidates with a
`GL(h,Z)`-only invariant (the GCD family) — see ALGORITHM.md §6.

Done means: the claim is in `docs/FINDINGS.md` under one of established /
folklore / new / taken-on-trust, with the number that supports it and the
command that regenerates it.
