"""Phase 2 agent loop (SPEC §3.5): Claude Agent SDK driving the six
harness tools over a standalone stdio MCP server. The agent gets NO
built-in tools — no shell, no file writes; every capability is one of
the six IPC operations, and the trust layer's allowlist/gates remain
the hard boundary (SPEC §10).

Run one attempt:
    python -m hotpath_agent.loop <target> --run-id <id> --repo-root <path>
"""

from __future__ import annotations

import argparse
import asyncio
import os
import sys

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ResultMessage,
    TextBlock,
    query,
)

from .prompts import SYSTEM_PROMPT

HARNESS_TOOLS = [
    "mcp__harness__read_profile",
    "mcp__harness__read_ledger",
    "mcp__harness__read_playbook",
    "mcp__harness__read_target_source",
    "mcp__harness__propose_patch",
    "mcp__harness__run_verdict",
    "mcp__harness__read_verdict",
]

# Built-in tools we deny at the SDK layer. This is DEFENSE IN DEPTH, not the
# trust boundary. SPEC §10's real boundary is OS-level process/user isolation:
# harnessd runs as a separate process the agent can't bypass, target code runs
# in a no-net container. In THIS dev container the nested agent runs as root
# under a parent Claude Code session, so neither --dangerously-skip-permissions
# nor a can_use_tool callback reliably gates built-ins (settings inherited from
# the parent session shadow them). We enumerate what we can; production runs the
# agent process without host-shell capability at all. What holds regardless:
# propose_patch is the only harness-mediated write path (path-allowlist + git
# apply), and the ledger is append-only (mutation-refusing triggers).
BUILTIN_TOOLS_DENIED = [
    "Bash", "Read", "Write", "Edit", "Glob", "Grep",
    "WebSearch", "WebFetch", "Task", "TodoWrite",
]


async def run_attempt(target: str, run_id: str, max_turns: int, repo_root: str) -> None:
    root = os.path.abspath(repo_root)
    options = ClaudeAgentOptions(
        model="claude-fable-5",
        cwd=root,
        mcp_servers={
            "harness": {
                "type": "stdio",
                "command": sys.executable,
                "args": ["-m", "hotpath_agent.mcp_server"],
                "env": {**os.environ, "HOTPATH_REPO_ROOT": root, "PYTHONPATH": os.path.join(root, "agent")},
            }
        },
        allowed_tools=HARNESS_TOOLS,
        disallowed_tools=BUILTIN_TOOLS_DENIED,  # defense in depth; see note above
        system_prompt=SYSTEM_PROMPT,
        # acceptEdits (not bypassPermissions — the CLI refuses
        # --dangerously-skip-permissions under root) with the allowlist runs the
        # loop headless. The real boundary is OS isolation, not this flag.
        permission_mode="acceptEdits",
        max_turns=max_turns,
    )
    task = (
        f"Perform ONE optimization attempt on target '{target}'. You have NO shell, file, or "
        f"web tools — only the seven mcp__harness__ tools. Do not attempt any other tool.\n"
        f"1. read_ledger to see attempted classes; read_profile for hotspots.\n"
        f"2. Pick the cheapest UNTRIED playbook class whose preconditions the profile "
        f"satisfies; read_playbook for it.\n"
        f"3. Read the relevant source with read_target_source — for large files pass "
        f"offset/limit and page through in windows (do NOT attempt any shell). State your "
        f"hypothesis, then propose_patch (unified diff, a/ b/ prefixes, paths relative to the "
        f"target workspace root).\n"
        f"4. run_verdict with run_id '{run_id}', the class number, and the hotspot string. "
        f"It launches a multi-minute pipeline and returns immediately.\n"
        f"5. Poll read_verdict('{run_id}') until it returns a verdict (not a 'running' status). "
        f"The pipeline takes several minutes; keep polling patiently.\n"
        f"6. Report the verdict and speedup CI verbatim. A rejection with a clean ledger row is a "
        f"successful outcome — do not iterate more than twice on a rejected hypothesis."
    )

    async for message in query(prompt=task, options=options):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    print(f"[agent] {block.text}", flush=True)
        elif isinstance(message, ResultMessage):
            print(f"[result] turns={message.num_turns} cost_usd={message.total_cost_usd}", flush=True)


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("target")
    p.add_argument("--run-id", required=True)
    p.add_argument("--max-turns", type=int, default=40)
    p.add_argument("--repo-root", default=".")
    a = p.parse_args()
    asyncio.run(run_attempt(a.target, a.run_id, a.max_turns, a.repo_root))


if __name__ == "__main__":
    main()
