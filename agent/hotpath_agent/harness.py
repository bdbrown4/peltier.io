"""Client for harnessd — the agent's only door to the trust layer.

Spawns the Rust daemon and speaks one JSON object per line over its
stdio. The client is deliberately dumb: no retries that could double-
apply a patch, no local fallbacks that could fabricate evidence.
"""

from __future__ import annotations

import json
import subprocess
from typing import Any


class HarnessError(RuntimeError):
    """The harness refused or failed an operation."""


class HarnessClient:
    def __init__(self, repo_root: str, binary: str = "target/debug/harnessd"):
        self._proc = subprocess.Popen(
            [binary],
            cwd=repo_root,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )

    def call(self, op: str, **kwargs: Any) -> dict[str, Any]:
        assert self._proc.stdin and self._proc.stdout
        self._proc.stdin.write(json.dumps({"op": op, **kwargs}) + "\n")
        self._proc.stdin.flush()
        line = self._proc.stdout.readline()
        if not line:
            raise HarnessError("harnessd exited")
        resp = json.loads(line)
        if not resp.get("ok"):
            raise HarnessError(resp.get("error", "unknown harness error"))
        return resp["result"]

    def close(self) -> None:
        if self._proc.stdin:
            self._proc.stdin.close()
        self._proc.wait(timeout=10)
