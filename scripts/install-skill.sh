#!/bin/sh
# install-skill.sh — stamp the peltier Agent Skill into a consumer repo.
#
#   sh scripts/install-skill.sh <target-repo> [--layout claude|agents|both]
#   sh scripts/install-skill.sh --zeroclaw-variant <out-dir>
#
# The skill is one directory (SKILL.md + references/ + scripts/ + assets/),
# already in the Agent Skills format (agentskills.io) that Claude Code, Codex,
# Copilot, VS Code, Cursor, Gemini CLI, opencode, Goose, Amp, Zed and others
# load natively. All this script does is copy that one canonical directory to
# the discovery path(s) a harness actually reads:
#
#   .claude/skills/peltier/   Claude Code (also read by Copilot, VS Code,
#                             opencode, Amp, and Cursor in compat mode)
#   .agents/skills/peltier/   the cross-tool convergence path (Codex, Zed,
#                             Cursor, Copilot, VS Code, Gemini CLI, opencode,
#                             Goose, Amp)
#
# --layout both (the default) writes both. That is two copies in YOUR repo;
# keep them identical (re-run this script to update both; peltier's own CI
# shows the diff -r pattern for enforcing it).
#
# --zeroclaw-variant writes a copy WITHOUT scripts/ to <out-dir>: ZeroClaw's
# skill security policy blocks script files by default (skills.allow_scripts
# is off), so its variant runs preflight from the peltier checkout instead —
# the skill's SKILL.md already explains that path. Then install it with:
#   zeroclaw skills install <out-dir>
#
# The skill still needs a peltier checkout at runtime (bench-runner lives
# there). Set PELTIER_HOME in the environment your agent runs in.
set -eu

usage() {
    sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
}

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
src="$here/../.claude/skills/peltier"
[ -f "$src/SKILL.md" ] || { echo "install-skill: cannot find the skill at $src" >&2; exit 1; }

copy_skill() { # $1 = dest dir, $2 = "full" | "noscripts"
    rm -rf "$1"
    mkdir -p "$1"
    cp -R "$src/." "$1/"
    if [ "$2" = "noscripts" ]; then rm -rf "$1/scripts"; fi
}

case "${1:-}" in
    ""|-h|--help) usage ;;
    --zeroclaw-variant)
        out=${2:-}; [ -n "$out" ] || usage
        copy_skill "$out" noscripts
        echo "wrote script-less variant to $out (ZeroClaw blocks script files in skills by default)"
        echo "next: zeroclaw skills install \"$out\"    # optionally --bundle <alias>, then add the"
        echo "      bundle to agents.<alias>.skill_bundles so your agent loads it"
        echo "note: at runtime the skill runs preflight from your peltier checkout —"
        echo "      set PELTIER_HOME to that checkout for the agent's environment"
        exit 0
        ;;
esac

target=$1; shift
layout=both
while [ $# -gt 0 ]; do
    case "$1" in
        --layout) layout=${2:-}; shift 2 ;;
        *) usage ;;
    esac
done
case "$layout" in claude|agents|both) ;; *) usage ;; esac
[ -d "$target" ] || { echo "install-skill: target repo '$target' is not a directory" >&2; exit 1; }

wrote=""
if [ "$layout" = claude ] || [ "$layout" = both ]; then
    copy_skill "$target/.claude/skills/peltier" full
    wrote="$wrote $target/.claude/skills/peltier"
fi
if [ "$layout" = agents ] || [ "$layout" = both ]; then
    copy_skill "$target/.agents/skills/peltier" full
    wrote="$wrote $target/.agents/skills/peltier"
fi

echo "installed:$wrote"
echo "next: set PELTIER_HOME=/path/to/peltier.io in the environment your agent runs in"
echo "      (the skill drives the real bench-runner from that checkout; preflight refuses without it)"
