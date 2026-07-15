# peltier.io — agent entry point

This file exists for AGENTS.md-reading harnesses (Codex, Jules, Cursor, Zed,
Amp, Copilot coding agent, Gemini CLI, …). It is deliberately thin: the
binding project rules live in one place, not here.

## Working on this repo

Read **[CLAUDE.md](CLAUDE.md)** first and treat it as binding regardless of
which harness you are. It holds the non-negotiables (no unverified performance
claims, measure-before-optimizing, hard equivalence gates, the append-only
ledger, the trust-layer write boundary) and the honest current state of every
subsystem. The engineering spec is [SPEC.md](SPEC.md). Commands live in the
[justfile](justfile). The pipeline is Linux/POSIX-only at runtime; Windows
compiles the workspace but cannot run it.

Two hard rules that bear repeating even in a pointer file:

- **Never claim a performance number without its confidence interval and
  workload.** If you did not run the gates, the number does not exist.
- **The trust layer (`crates/`, `config/`, `corpora/`) is read-only to
  optimization agents.** Patches touch only allowlisted paths under
  `targets/<name>/`.

## Verifying performance changes (any repo, any harness)

This repo ships its trust layer as an Agent Skill:
**[.claude/skills/peltier/SKILL.md](.claude/skills/peltier/SKILL.md)** —
mirrored byte-identically at `.agents/skills/peltier/` for harnesses that
discover skills there (CI enforces the mirror). If your harness auto-loads
skills from either path, use it whenever a task involves proving or refuting
a speedup. If not, read that SKILL.md and follow it manually — starting with
`scripts/preflight.sh`, whose refusal is final.
