# The agent loop

The proposer is a Claude Agent SDK loop (`agent/hotpath_agent/`) driving
the seven harness tools over a standalone MCP server. It has no built-in
file, shell, or web tools in its allowlist — only the harness door.

## One attempt, start to finish

1. **Read the ledger.** What classes and hypotheses were already tried on
   this target? The ledger is the anti-double-attempt memory: a class may
   be re-entered with a *materially new* hypothesis, never a duplicate.
2. **Read the profile.** callgrind cache-sim ranks hotspots with per-
   function cache-miss and branch-mispredict columns, so the agent can tell
   an allocation-churn hotspot from a branch-bound one and pick the right
   playbook class.
3. **Pick the cheapest untried class** whose profile preconditions are met,
   and read its playbook entry.
4. **Read the source** (windowed, at the pinned commit) and **propose a
   patch** with a stated hypothesis.
5. **Run the verdict** and **poll** for the result.
6. **Report it verbatim.** A rejection with a clean ledger row is a
   *successful* outcome. Do not iterate more than twice on a rejected
   hypothesis without a new idea.

## What the loop proved about itself

Across an unattended grind of 20+ audited attempts, the loop demonstrated
the behavior the architecture is designed to produce:

- It chose the cheapest untried class each time, reading the ledger to
  avoid re-grinding a dead end.
- Every patch went through `propose_patch` (path-allowlist + `git apply`)
  and the full gate+bench pipeline. Malformed diffs were refused by the
  harness, not the agent's goodwill.
- **It twice refused to fabricate a verdict it could not read back.** When
  a multi-minute pipeline outran the MCP transport's timeout, the agent
  checked the ledger, confirmed the row wasn't written, and reported the
  verdict as *unreadable, not invented* — asking a human to read the
  ledger rather than guessing a number. That refusal, under a real failure
  mode, is the anti-reward-hacking design working.

## The honest-failure record

The ledger records **every** attempt, including the rejections, because a
rejection is training data and a demonstration of the bar. Of the audited
unattended attempts, most were honest rejections — real mechanisms that
simply didn't clear the 2% CI-lower-bound bar — a handful were verified
wins, and one was a `rejected-gate` where golden replay caught the agent's
patch producing wrong output that a green test suite would have shipped.

The load-bearing number is **zero shipped false accepts** across the whole
project.
