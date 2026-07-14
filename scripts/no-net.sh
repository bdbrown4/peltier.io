#!/bin/sh
# Network-isolation wrapper for the local verdict path (SPEC §10): exec the
# given command inside a fresh network namespace so target code built and
# run by the pipeline cannot reach the network. harnessd invokes it as
# `no-net.sh sh -c <pipeline-script>` (HOTPATH_VERDICT_WRAPPER overrides).
# CI gets the same guarantee from `docker --network=none` instead.
set -eu
if [ $# -eq 0 ]; then
    echo "usage: scripts/no-net.sh <command> [args...]" >&2
    exit 2
fi
if unshare --net --map-current-user true 2>/dev/null; then
    exec unshare --net --map-current-user -- "$@"
fi
if [ "${HOTPATH_ALLOW_UNISOLATED:-}" = "1" ]; then
    echo "no-net.sh: WARNING: unshare --net unavailable and HOTPATH_ALLOW_UNISOLATED=1 —" >&2
    echo "no-net.sh: WARNING: running WITHOUT network isolation. Target code can reach the network." >&2
    exec "$@"
fi
echo "no-net.sh: cannot create a network namespace (unshare --net --map-current-user failed)." >&2
echo "no-net.sh: refusing to run target code unisolated. Options:" >&2
echo "no-net.sh:   - run on a host with unprivileged user namespaces enabled" >&2
echo "no-net.sh:   - in CI, run the whole pipeline under docker --network=none" >&2
echo "no-net.sh:   - set HOTPATH_ALLOW_UNISOLATED=1 to override (loud warning, not for accepts)" >&2
exit 97
