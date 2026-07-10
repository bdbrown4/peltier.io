#!/bin/sh
# Launch one unattended optimization attempt behind an OS-level boundary
# (SPEC §10). Run as the trusted uid (root in this container):
#
#   scripts/agent-isolated.sh <target> <run-id> [max-turns]
#
# Two isolation modes (HOTPATH_ISOLATION, default "mountns"):
#
#   mountns — this container's boundary. The agent process tree (Claude
#     CLI + MCP server + harness client) runs in a private mount
#     namespace where the repo is bind-mounted READ-ONLY, with
#     CAP_SYS_ADMIN and CAP_SYS_PTRACE removed from the bounding set so
#     the mount cannot be undone (verified: remount and userns re-entry
#     both refused). Repo writes fail with EROFS no matter the uid; the
#     CLI's own auth state (under $HOME) stays untouched. No credential
#     copying.
#
#   user — the bench-metal shape (docs/infra.md). The agent loop runs as
#     the unprivileged user 'hpagent' whose only door into the repo is
#     the harnessd socket. Requires the agent user to have its own API
#     credentials (CLAUDE_CODE_OAUTH_TOKEN or ANTHROPIC_API_KEY in its
#     environment) — a parent session's root-owned auth is deliberately
#     NOT copied across the boundary.
#
# In both modes harnessd + the verdict pipeline run OUTSIDE the boundary
# as the trusted uid and own every write into the repo (git apply,
# ledger rows, pending/ evidence). Verify the boundary any time with
# scripts/isolation-check.sh.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
TARGET=${1:?usage: agent-isolated.sh <target> <run-id> [max-turns]}
RUN_ID=${2:?usage: agent-isolated.sh <target> <run-id> [max-turns]}
MAX_TURNS=${3:-40}
MODE=${HOTPATH_ISOLATION:-mountns}

AGENT_USER=hpagent
SOCK_DIR=/run/hotpath
SOCK=$SOCK_DIR/harness.sock

[ "$(id -u)" -eq 0 ] || { echo "must run as the trusted uid (root)" >&2; exit 1; }

# --- no world-writable holes in the repo ------------------------------------
# In user mode the boundary is filesystem permissions; a single o+w path
# would void it. mountns mode doesn't need this, but a hole is still a
# smell worth failing on.
HOLES=$(find "$ROOT" -perm -o+w \( -type f -o -type d \) 2>/dev/null || true)
if [ -n "$HOLES" ]; then
    echo "refusing to launch: world-writable paths inside the repo:" >&2
    echo "$HOLES" >&2
    exit 1
fi

# --- trust-layer daemon (outside the boundary) --------------------------------
( cd "$ROOT" && cargo build -q -p harnessd -p verdict )

mkdir -p "$SOCK_DIR"
if [ ! -S "$SOCK" ]; then
    ( cd "$ROOT" && setsid ./target/debug/harnessd --socket "$SOCK" \
        >>/var/log/hotpath-harnessd.log 2>&1 & )
    for _ in $(seq 1 50); do [ -S "$SOCK" ] && break; sleep 0.1; done
    [ -S "$SOCK" ] || { echo "harnessd socket never appeared" >&2; exit 1; }
fi

run_loop="python3 -m hotpath_agent.loop $TARGET --run-id $RUN_ID --max-turns $MAX_TURNS --repo-root $ROOT"

case "$MODE" in
mountns)
    chmod 600 "$SOCK"   # uid 0 only; the confined tree is still uid 0
    exec unshare -m sh -c "
        mount --bind '$ROOT' '$ROOT' &&
        mount -o remount,bind,ro '$ROOT' &&
        exec setpriv --bounding-set -sys_admin,-sys_ptrace --inh-caps -all \
            env HOTPATH_HARNESS_SOCKET='$SOCK' \
                HOTPATH_REPO_ROOT='$ROOT' \
                PYTHONPATH='$ROOT/agent' \
            $run_loop"
    ;;
user)
    id "$AGENT_USER" >/dev/null 2>&1 || useradd -m -s /bin/sh "$AGENT_USER"
    HP_HOME=$(getent passwd "$AGENT_USER" | cut -d: -f6)
    chown root:"$AGENT_USER" "$SOCK"
    chmod 660 "$SOCK"
    # The container's TLS-intercepting proxy CA lives under /root (mode
    # 700); the agent user needs a readable copy to reach the API.
    CA_SRC=${HOTPATH_CA_SRC:-/root/.ccr/ca-bundle.crt}
    CA=/etc/hotpath-ca.crt
    [ -f "$CA_SRC" ] && install -m 644 "$CA_SRC" "$CA"
    # setpriv preserves the environment (proxy/API routing vars); we
    # override HOME and every CA var that pointed under /root. The agent
    # user must bring its own API credentials — see header.
    exec setpriv --reuid "$AGENT_USER" --regid "$AGENT_USER" --init-groups \
        env HOME="$HP_HOME" USER="$AGENT_USER" LOGNAME="$AGENT_USER" \
            HOTPATH_HARNESS_SOCKET="$SOCK" \
            HOTPATH_REPO_ROOT="$ROOT" \
            PYTHONPATH="$ROOT/agent" \
            SSL_CERT_FILE="$CA" NODE_EXTRA_CA_CERTS="$CA" \
            CURL_CA_BUNDLE="$CA" REQUESTS_CA_BUNDLE="$CA" \
            GIT_SSL_CAINFO="$CA" AWS_CA_BUNDLE="$CA" \
            NIX_SSL_CERT_FILE="$CA" GRPC_DEFAULT_SSL_ROOTS_FILE_PATH="$CA" \
        $run_loop
    ;;
*)
    echo "unknown HOTPATH_ISOLATION mode: $MODE (mountns|user)" >&2
    exit 1
    ;;
esac
