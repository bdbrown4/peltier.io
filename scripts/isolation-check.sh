#!/bin/sh
# Verify the SPEC §10 OS boundary from the agent's side of it. Run as the
# trusted uid (root). Checks BOTH isolation modes of agent-isolated.sh:
# every write the agent could use to game the harness must fail; the
# harnessd socket door must work. Exits nonzero on any hole.
set -u

ROOT=$(cd "$(dirname "$0")/.." && pwd)
AGENT_USER=hpagent
SOCK=${HOTPATH_HARNESS_SOCKET:-/run/hotpath/harness.sock}
FAIL=0

note_fail() { echo "FAIL (boundary hole): $1"; FAIL=1; }
note_ok()   { echo "ok: $1"; }

[ -S "$SOCK" ] || { echo "FAIL: harnessd socket $SOCK missing (run agent-isolated.sh or start the daemon)"; exit 1; }

# Probe scripts as files: they run nested inside sh -c layers where
# inline `python3 -c` quoting is unmanageable.
PROBE_DIR=$(mktemp -d)
trap 'rm -rf "$PROBE_DIR"' EXIT
chmod 755 "$PROBE_DIR"

cat > "$PROBE_DIR/sock_check.py" <<EOF
import json, socket, sys
s = socket.socket(socket.AF_UNIX); s.settimeout(15); s.connect('$SOCK')
f = s.makefile('rw')
f.write(json.dumps({'op': 'read_ledger', 'target': 'comrak'}) + '\n'); f.flush()
resp = json.loads(f.readline())
assert resp['ok'], resp
EOF

cat > "$PROBE_DIR/escape_check.py" <<EOF
import json, socket
s = socket.socket(socket.AF_UNIX); s.settimeout(15); s.connect('$SOCK')
f = s.makefile('rw')
diff = '--- a/../../../config/accept.toml\n+++ b/../../../config/accept.toml\n@@ -1 +1 @@\n-x\n+y\n'
f.write(json.dumps({'op': 'propose_patch', 'target': 'comrak', 'diff': diff, 'hypothesis': 'probe'}) + '\n'); f.flush()
resp = json.loads(f.readline())
assert not resp['ok'] and 'escape' in resp['error'], resp
EOF

cat > "$PROBE_DIR/ledger_insert.py" <<EOF
import sqlite3
c = sqlite3.connect('$ROOT/results/ledger.sqlite')
c.execute("INSERT INTO attempts (run_id,timestamp,target,target_commit,phase,hotspot,playbook_class,hypothesis,patch,gates,bench,verdict,tokens_spent,wall_time_s) VALUES ('forged','','t','','','',1,'','','{}','{}','accepted',0,0)")
c.commit()
EOF
chmod 644 "$PROBE_DIR"/*.py

# ============================ mountns mode ====================================
echo "--- mountns mode (confined uid-0 tree, repo read-only) ---"
MNT_PREFIX="unshare -m"
CONFINE="mount --bind '$ROOT' '$ROOT' && mount -o remount,bind,ro '$ROOT' && exec setpriv --bounding-set -sys_admin,-sys_ptrace --inh-caps -all"

mnt_must_fail() {
    desc=$1; cmd=$2
    if $MNT_PREFIX sh -c "$CONFINE sh -c \"$cmd\"" >/dev/null 2>&1; then
        note_fail "[mountns] $desc succeeded"
    else
        note_ok "[mountns] $desc refused"
    fi
}
mnt_must_pass() {
    desc=$1; cmd=$2
    if $MNT_PREFIX sh -c "$CONFINE sh -c \"$cmd\"" >/dev/null 2>&1; then
        note_ok "[mountns] $desc works"
    else
        note_fail "[mountns] $desc broken"
    fi
}

mnt_must_fail "write into crates/ (trust layer)"      "touch '$ROOT/crates/.probe'"
mnt_must_fail "edit config/accept.toml (thresholds)"  "echo x >> '$ROOT/config/accept.toml'"
mnt_must_fail "write a target workspace directly"     "touch '$ROOT/targets/comrak/workspace/.probe'"
mnt_must_fail "INSERT a forged ledger row"            "python3 $PROBE_DIR/ledger_insert.py"
mnt_must_fail "overwrite results/ evidence"           "echo x > '$ROOT/results/.probe'"
mnt_must_fail "replace a trust binary"                "echo x > '$ROOT/target/debug/verdict'"
mnt_must_fail "remount the repo read-write"           "mount -o remount,bind,rw '$ROOT'"
mnt_must_fail "userns re-entry to undo the ro mount"  "unshare -Ur -m mount -o remount,bind,rw '$ROOT'"
mnt_must_pass "harnessd socket read_ledger"           "python3 $PROBE_DIR/sock_check.py"

# ============================ user mode =======================================
if id "$AGENT_USER" >/dev/null 2>&1; then
    echo "--- user mode (unprivileged $AGENT_USER) ---"
    HP_HOME=$(getent passwd "$AGENT_USER" | cut -d: -f6)
    as_agent() {
        setpriv --reuid "$AGENT_USER" --regid "$AGENT_USER" --init-groups \
            env HOME="$HP_HOME" "$@"
    }
    must_fail() {
        desc=$1; shift
        if as_agent "$@" >/dev/null 2>&1; then
            note_fail "[user] $desc succeeded as $AGENT_USER"
        else
            note_ok "[user] $desc refused"
        fi
    }
    must_pass() {
        desc=$1; shift
        if as_agent "$@" >/dev/null 2>&1; then
            note_ok "[user] $desc works"
        else
            note_fail "[user] $desc broken for $AGENT_USER"
        fi
    }
    # user mode needs group access on the socket for the positive checks
    SOCK_GRP_SAVED=$(stat -c '%U:%G %a' "$SOCK")
    chown root:"$AGENT_USER" "$SOCK"; chmod 660 "$SOCK"

    must_fail "write into crates/ (trust layer)"      sh -c "touch '$ROOT/crates/.probe'"
    must_fail "edit config/accept.toml (thresholds)"  sh -c "echo x >> '$ROOT/config/accept.toml'"
    must_fail "write into corpora/ (pinned inputs)"   sh -c "touch '$ROOT/corpora/.probe'"
    must_fail "write a target workspace directly"     sh -c "touch '$ROOT/targets/comrak/workspace/.probe'"
    must_fail "INSERT a forged ledger row"            python3 "$PROBE_DIR/ledger_insert.py"
    must_fail "overwrite results/ evidence"           sh -c "echo x > '$ROOT/results/.probe'"
    must_fail "replace a trust binary"                sh -c "echo x > '$ROOT/target/debug/verdict'"
    must_fail "run the verdict binary to write a row" sh -c "cd '$ROOT' && ./target/debug/verdict comrak --candidate-bin /bin/true --run-id forged-direct --playbook-class 1 --hypothesis x --hotspot x"
    must_pass "harnessd socket read_ledger"           python3 "$PROBE_DIR/sock_check.py"
    must_pass "harnessd refuses ../ escape in a diff" python3 "$PROBE_DIR/escape_check.py"

    chown "${SOCK_GRP_SAVED%% *}" "$SOCK"; chmod "${SOCK_GRP_SAVED##* }" "$SOCK"
else
    echo "--- user mode skipped (no $AGENT_USER user) ---"
fi

if [ "$FAIL" -eq 0 ]; then
    echo "isolation-check: ALL CHECKS PASSED"
else
    echo "isolation-check: BOUNDARY HOLES FOUND" >&2
fi
exit "$FAIL"
