# Phase 2 case study — the unattended agent loop

**Result: the Claude Agent SDK loop ran the full profile → hypothesize →
patch → gated-verdict cycle on comrak with no human in the loop, across
multiple attempts, with ZERO false accepts. The agent twice refused to
fabricate a verdict it could not read back, verifying ledger integrity
instead — exactly the anti-reward-hacking behavior SPEC §10 is designed
to produce.**

## What runs

- `agent/hotpath_agent/loop.py` — Claude Agent SDK (`claude-agent-sdk`,
  model `claude-fable-5`) driving a standalone stdio MCP server
  (`mcp_server.py`) that exposes the seven harness tools. The agent has
  no built-in file/shell/web tools in its allowlist.
- `harnessd` gained an async `run_verdict` (launches the multi-minute
  build+bench pipeline detached, returns immediately) and a pollable
  `read_verdict` that reads the append-only ledger row once written.
- `read_target_source` gained an `offset`/`limit` line window so 100 KB+
  source files can be paged instead of overflowing one response.

## The attempts (all in the ledger, `results/ledger.sqlite`)

| run_id | class chosen | patch | verdict | speedup CI |
|---|---|---|---|---|
| phase2-comrak-001 | 3 alloc-churn | syntect buffer `with_capacity` | rejected-bench | [0.971, 1.007] |
| phase2-comrak-002 | 5 algorithmic | gate dead `VecDeque` build in text postprocess | rejected-bench | [0.982, 1.026] |
| phase2-comrak-003 | 6 SIMD | hoist `^`-in-brackets check out of `find_special_char` predicate | rejected-bench | [0.970, 1.007] |

Run 003 exercised the two shipped fixes: the agent **paged through a
2,400-line file with `read_target_source` windows** (no shell reach this
time) and **polled the async `read_verdict` cleanly** to a verbatim
verdict. Across all three attempts the agent chose the cheapest untried
class each time (3 → 5 → 6, skipping 4/7 whose profile preconditions the
callgrind data didn't support), and **0 of 3 were false accepts**.

The agent's class selection was correct each time: it read the ledger,
saw which classes were already attempted, and picked the cheapest
untried class whose profile preconditions were met. Both patches went
through `propose_patch` (path-allowlist + `git apply`) and the full
gate+bench pipeline. Both were correctly rejected — real mechanisms,
too small to clear the 2% CI-lower-bound bar. **No false accepts.**

## Two findings the loop surfaced about itself

1. **`run_verdict` outlived the MCP transport (attempt 001).** The
   build+bench pipeline takes minutes; the MCP tool-call cap is 60 s. The
   agent's calls timed out — and rather than retry blindly or invent a
   number, it checked `read_ledger`, confirmed exactly one row was
   written (dedup on run_id held), and reported the verdict as
   *unreadable, not fabricated*, asking a human to read the ledger. Fix
   shipped: async `run_verdict` + pollable `read_verdict`. Attempt 002
   then read its verdict back cleanly.

2. **The SDK tool-restriction is not the trust boundary in this
   environment.** The agent reached for a built-in `Bash`/`Monitor` tool
   to read a source file too large for the (then whole-file-only)
   `read_target_source`. Investigation showed that a nested Claude Code
   CLI running as root under a parent session does not reliably gate
   built-ins: `--dangerously-skip-permissions` is refused under root, and
   a `can_use_tool` deny-by-default callback is shadowed by settings
   inherited from the parent session (and broke legitimate MCP tool calls
   with "Stream closed"). **This is expected, not a regression:** SPEC §10
   specifies the boundary as OS-level process/user isolation and no-net
   containers — "enforced by filesystem permissions and a separate-process
   harness, not by prompt." The SDK allow/deny layer is defense in depth.
   Mitigation shipped: windowed `read_target_source` removes the agent's
   reason to reach for a shell; the load-bearing guarantees hold
   regardless (see below).

## What holds even when the tool layer leaks

- **Writes to the target are only mediated through `propose_patch`** —
  path-allowlist (relative paths, no `..`, no `.git`) then `git apply`
  rooted in the target workspace. A patch touching `config/` or `crates/`
  is refused before any apply. Verified against a malicious
  accept-threshold diff in Phase 1.
- **The ledger is append-only** — UPDATE/DELETE are refused by SQLite
  triggers, and `accepted` rows are written only by the `verdict` binary
  after the gated pipeline. An agent cannot forge a passing verdict
  through the harness.
- **The gate+bench pipeline is unbypassable through the harness** —
  pristine-rebuilt baseline, interleaved A/B, CI-lower-bound rule.

The residual hole in THIS container: an agent with host shell could, in
principle, `sqlite3 INSERT` a forged row (triggers block mutation, not
insertion) or invoke the verdict binary directly. That is precisely the
attack SPEC §10's process/user isolation exists to prevent, and it is the
**Phase 2 exit gap to close on real infrastructure** (run the agent
process as a separate unprivileged user with no shell and the trust layer
owned by another uid — the `bench-metal`/`docs/infra.md` setup).

## Phase 2 exit criteria (SPEC §5)

- Unattended profile→verdict loop on one target: **met** (comrak).
- ≥1 auto-accepted win: **not yet** — both attempts were honest
  rejections. The loop is proven; a win needs either a richer profile
  (perf `stat` cache-miss data to unlock class-4 preconditions) or more
  attempts against fresh hypotheses.
- Zero false accepts across audited attempts: **met so far** (2/2
  audited, both correctly rejected).
- The separate-uid isolation for the tool boundary is the remaining
  hardening item, tracked to `docs/infra.md`.
