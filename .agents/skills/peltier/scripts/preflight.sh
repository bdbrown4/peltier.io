#!/bin/sh
# peltier preflight — locate the trust layer, check this host can RUN it, and
# build the real bench-runner.
#
# STATUS=ok means the harness is present and runnable here. It does NOT mean
# this host can measure your change: only the A/A self-test decides that, and
# it decides it empirically. Do not read STATUS=ok as "calibrated".
#
# A refusal (STATUS=refuse, nonzero exit) is a complete, valid outcome. It
# ends the *claim*, not the work: you may still write and explain a patch, you
# just may not attach a number to it. Never work around a refusal with a
# hand-rolled timing loop.
set -eu

say() { printf '%s\n' "$1"; }
refuse() {
    say "STATUS=refuse"
    say "REASON=$1"
    exit 1
}
abspath() { ( CDPATH= cd -- "$1" 2>/dev/null && pwd ); }

# --- host -------------------------------------------------------------
# bench-runner shells every timed run through `sh -c`, and the in-repo
# pipeline (harnessd, service mode, the sh-based gates) is POSIX-only.
# Windows compiles the workspace but cannot run it.
uname_s=$(uname -s 2>/dev/null || echo unknown)
case "$uname_s" in
    Linux)  host=linux ;;
    Darwin) host=darwin ;;
    *) refuse "unsupported host '$uname_s' — peltier's harness is Linux/POSIX-only at runtime. Run it on Linux (or macOS for verify mode). Do not substitute another benchmark." ;;
esac

# --- trust layer ------------------------------------------------------
is_peltier() { [ -f "$1/crates/bench-runner/Cargo.toml" ] && [ -f "$1/config/accept.toml" ]; }

# Walk up from a starting dir looking for a checkout. Absolute POSIX paths
# always terminate at "/", so this cannot spin.
walk_up() {
    d=$1
    while [ -n "$d" ] && [ "$d" != "/" ]; do
        if is_peltier "$d"; then printf '%s' "$d"; return 0; fi
        d=$(dirname "$d")
    done
    is_peltier / && printf '/'
    return 0
}

home=""
if [ -n "${PELTIER_HOME:-}" ]; then
    canon=$(abspath "$PELTIER_HOME" || true)
    [ -n "$canon" ] || refuse "PELTIER_HOME='$PELTIER_HOME' does not exist"
    is_peltier "$canon" || refuse "PELTIER_HOME='$PELTIER_HOME' is not a peltier checkout (no crates/bench-runner/Cargo.toml + config/accept.toml)"
    home=$canon
else
    # cwd first (working inside a checkout), then this script's own location
    # (the skill still living inside one). A skill copied into another project
    # finds neither — which is why PELTIER_HOME exists.
    home=$(walk_up "$(pwd)")
    [ -n "$home" ] || home=$(walk_up "$(abspath "$(dirname -- "$0")")")
fi
[ -n "$home" ] || refuse "no peltier checkout found — clone https://github.com/bdbrown4/peltier.io and set PELTIER_HOME=/path/to/peltier.io. The statistics live in bench-runner; do not reimplement them."

command -v cargo >/dev/null 2>&1 || refuse "cargo not on PATH — needed to build bench-runner from $home"

# --- build the real harness -------------------------------------------
# Always build. cargo is a no-op when current, and skipping this because a
# binary happens to exist would silently measure with a stale copy of the
# statistics after a `git pull`.
br="$home/target/release/bench-runner"
( cd "$home" && cargo build --release -q -p bench-runner ) \
    || refuse "bench-runner failed to build in $home — fix the build before claiming any number"
{ [ -f "$br" ] && [ -x "$br" ]; } || refuse "bench-runner did not appear at $br after a successful build"

# --- pinning ----------------------------------------------------------
# NOTE: config/accept.toml's `pin_prefix` is read ONLY by the in-repo verdict
# pipeline. For compare/aa/calibrate you must wrap the commands yourself
# (`taskset -c N <cmd>` on both sides). PIN_SUPPORTED just says whether that
# tool is available here.
if [ "$host" = linux ] && command -v taskset >/dev/null 2>&1; then
    pin=yes
else
    pin=no
fi

say "STATUS=ok"
say "PELTIER_HOME=$home"
say "BENCH_RUNNER=$br"
say "HOST=$host"
say "PIN_SUPPORTED=$pin"
say "NOTE=harness runnable here; the A/A self-test decides whether it can measure your change"
