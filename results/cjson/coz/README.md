# coz on cjson — integration status

coz is **wired** into the harness (SPEC §12): `targets/cjson/harness.c`
carries a `-DHOTPATH_COZ` throughput progress point, and
`scripts/coz-profile.sh` + `just coz cjson` build the instrumented binary
and run `coz run`. `scripts/coz-summary.py` ranks lines by causal
throughput effect.

**Runtime limitation in this container:** the distro `coz-profiler`
(apt) aborts inside its own `init_coz` during startup —
`pthread_create` interposition recurses ("init_coz in progress, do not
recurse" → SIGABRT) before `main` runs. This is a known coz/glibc
incompatibility on newer glibc, independent of the harness. It reproduces
on the pristine binary with a bare `coz run`, so it is not our
instrumentation.

**What we use instead:** callgrind `--cache-sim --branch-sim`
(`results/cjson/hotspots.txt`) is the working profiler and gives an
unambiguous class-selection signal for cjson — glibc float
serialization/parsing (`__printf_fp`, `strtod`) plus cJSON
`parse_value`/`print_value` dominate. The coz path is ready for an
environment with a compatible coz runtime (bench-metal / a pinned coz
build); no code change is needed there.
