"""Standalone MCP stdio server exposing the six harness tools (SPEC §3.5).

Spawned as a subprocess by the agent loop; speaks MCP over stdio. Wraps
the harnessd IPC client — the trust-layer allowlist and gates remain the
hard boundary. Run standalone for debugging:

    HOTPATH_REPO_ROOT=/path/to/repo python -m hotpath_agent.mcp_server
"""

from __future__ import annotations

import os

from mcp.server.fastmcp import FastMCP

from . import tools as harness

REPO_ROOT = os.environ.get("HOTPATH_REPO_ROOT", ".")
harness.connect(REPO_ROOT)

mcp = FastMCP("harness")


@mcp.tool()
def read_profile(target: str) -> str:
    """Latest profile for a target: ranked hotspots with % of instructions."""
    return str(harness.read_profile(target))


@mcp.tool()
def read_ledger(target: str) -> str:
    """Prior attempts against a target. Never re-attempt a playbook class listed here."""
    return str(harness.read_ledger(target))


@mcp.tool()
def read_playbook(class_number: int) -> str:
    """Optimization playbook class 1-7: preconditions, procedure, verification, failure modes."""
    return harness.read_playbook(class_number)


@mcp.tool()
def read_target_source(target: str, path: str, offset: int = 0, limit: int = 400) -> str:
    """Read one file from the target workspace (path relative to the workspace root),
    as a line window: `offset` (0-based first line) and `limit` (max lines). Large
    files (100KB+) MUST be read in windows — the response is truncated otherwise.
    Returns numbered lines plus total_lines/returned_lines/truncated metadata."""
    return str(harness.read_target_source(target, path, offset, limit))


@mcp.tool()
def propose_patch(target: str, diff: str, hypothesis: str) -> str:
    """Submit a unified git diff (a/ b/ prefixes, paths relative to the target
    workspace, MUST end with a trailing newline) plus the hypothesis it tests.
    Each proposal is STANDALONE: the workspace is reset to pristine before the
    diff is applied, so always submit the complete patch — never an increment
    on top of an earlier proposal. The harness applies it only after a
    path-allowlist check. Returns a patch_id, or an error string if refused."""
    return harness.propose_patch(target, diff, hypothesis)


@mcp.tool()
def run_verdict(patch_id: str, run_id: str, playbook_class: int, hotspot: str) -> str:
    """Launch the gate+bench pipeline on a proposed patch (runs detached, several
    minutes) and return immediately. Poll read_verdict with the same run_id for the
    result; the ledger row is written when the pipeline finishes."""
    return str(harness.run_verdict(patch_id, run_id, playbook_class, hotspot))


@mcp.tool()
def read_verdict(run_id: str) -> str:
    """Poll for a completed verdict by run_id. Returns the verdict and speedup CI once
    the ledger row is written, or a 'running' status while the pipeline is still going."""
    return str(harness.read_verdict(run_id))


if __name__ == "__main__":
    mcp.run()
