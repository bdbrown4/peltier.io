"""Tool surface exposed to the model (SPEC §3.5).

Exactly six tools, all mediated by the trust-layer harness over IPC —
the agent process has no direct filesystem or shell capability beyond
what these return. Deliberately NOT exposed: shell on the host, writes
outside targets/<name>/, any access to crates/, config/, corpora/.

Phase 2 wiring: each function below becomes a Claude Agent SDK tool
definition whose implementation calls the harness. The signatures and
docstrings are the contract; the bodies are placeholders until the
harness IPC exists (Phase 1 exit criteria).
"""

from __future__ import annotations

from typing import Any

class HarnessUnavailable(RuntimeError):
    """Raised until the Phase 1 trust-layer harness IPC is wired up."""


def read_profile(target: str) -> dict[str, Any]:
    """Latest profile for a target: ranked hotspots (symbol, exclusive %,
    source mapping) plus perf stat counters. Read-only."""
    raise HarnessUnavailable("profiler adapter not wired (Phase 1)")


def read_ledger(target: str) -> list[dict[str, Any]]:
    """All prior attempts against this target — the anti-double-attempt
    memory. The agent must not re-grind a (hotspot, playbook_class,
    hypothesis) combination that already has a verdict."""
    raise HarnessUnavailable("ledger IPC not wired (Phase 1)")


def read_playbook(class_number: int) -> str:
    """Playbook class markdown (1-7): preconditions, procedure,
    verification notes, known failure modes."""
    raise HarnessUnavailable("playbook IPC not wired (Phase 1)")


def read_target_source(target: str, path: str) -> str:
    """Read-only view of the target's source tree at the pinned commit."""
    raise HarnessUnavailable("source IPC not wired (Phase 1)")


def propose_patch(target: str, diff: str, hypothesis: str) -> str:
    """Submit a unified diff plus the hypothesis it tests. The harness
    applies it via `git apply` after a path-allowlist check (SPEC §10);
    a diff touching anything outside targets/<target>/ is auto-rejected
    before any gate runs. Returns a patch id."""
    raise HarnessUnavailable("patch IPC not wired (Phase 1)")


def run_verdict(patch_id: str) -> dict[str, Any]:
    """Run the full gate + bench sequence on a proposed patch and return
    the ledger row (gates, bench CIs, verdict). This is the only way the
    agent ever learns whether a change 'worked' — there is no other
    number to optimize."""
    raise HarnessUnavailable("verdict IPC not wired (Phase 1)")
