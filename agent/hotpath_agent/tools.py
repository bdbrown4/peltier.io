"""Tool surface exposed to the model (SPEC §3.5).

Exactly seven tools, all mediated by harnessd over line-delimited JSON —
the agent process has no direct filesystem or shell capability beyond
what these return. Deliberately NOT exposed: shell on the host, writes
outside targets/<name>/, any access to crates/, config/, corpora/.

Phase 2 wiring: each function becomes a Claude Agent SDK tool
definition. The bodies talk to the live harness; construct a client
with `connect(repo_root)` first.
"""

from __future__ import annotations

from typing import Any

from .harness import HarnessClient

_client: HarnessClient | None = None


def connect(repo_root: str, binary: str = "target/debug/harnessd") -> None:
    global _client
    _client = HarnessClient(repo_root, binary)


def _c() -> HarnessClient:
    if _client is None:
        raise RuntimeError("call connect(repo_root) first")
    return _client


def read_profile(target: str) -> dict[str, Any]:
    """Latest profile for a target: ranked hotspots. Read-only."""
    return _c().call("read_profile", target=target)


def read_ledger(target: str) -> dict[str, Any]:
    """Prior attempts against this target — the anti-double-attempt
    memory. Never re-grind a class that already has a verdict."""
    return _c().call("read_ledger", target=target)


def read_playbook(class_number: int) -> str:
    """Playbook class markdown (1-7)."""
    return _c().call("read_playbook", **{"class": class_number})["markdown"]


def read_target_source(target: str, path: str, offset: int = 0, limit: int = 400) -> dict[str, Any]:
    """Read-only line window of a target workspace file at the pinned commit."""
    return _c().call("read_target_source", target=target, path=path, offset=offset, limit=limit)


def propose_patch(target: str, diff: str, hypothesis: str) -> str:
    """Submit a unified diff plus its hypothesis. The harness applies it
    via `git apply` after a path-allowlist check (SPEC §10); anything
    outside targets/<target>/workspace is rejected before any gate
    runs. Returns a patch id."""
    return _c().call("propose_patch", target=target, diff=diff, hypothesis=hypothesis)["patch_id"]


def run_verdict(patch_id: str, run_id: str, playbook_class: int, hotspot: str) -> dict[str, Any]:
    """Launch the gate + bench pipeline for a proposed patch (runs detached).
    Returns immediately; poll run_verdict's result via read_verdict(run_id)."""
    return _c().call(
        "run_verdict",
        patch_id=patch_id,
        run_id=run_id,
        playbook_class=str(playbook_class),
        hotspot=hotspot,
    )


def read_verdict(run_id: str) -> dict[str, Any]:
    """Poll for a completed verdict: verdict + speedup CI, or a running status."""
    return _c().call("read_verdict", run_id=run_id)
