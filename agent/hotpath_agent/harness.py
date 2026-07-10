"""Client for harnessd — the agent's only door to the trust layer.

Two transports:
- Unix socket (production/SPEC §10): if HOTPATH_HARNESS_SOCKET is set,
  connect to a daemon owned by the trusted uid. The agent process runs
  unprivileged and cannot write the repo except through this door.
- Spawned stdio subprocess (dev fallback): same uid, defense-in-depth
  only — the filesystem boundary does not hold in this mode.

The client is deliberately dumb: no retries that could double-apply a
patch, no local fallbacks that could fabricate evidence.
"""

from __future__ import annotations

import json
import os
import socket
import subprocess
from typing import Any


class HarnessError(RuntimeError):
    """The harness refused or failed an operation."""


class HarnessClient:
    def __init__(self, repo_root: str, binary: str = "target/debug/harnessd"):
        sock_path = os.environ.get("HOTPATH_HARNESS_SOCKET")
        self._proc: subprocess.Popen[str] | None = None
        self._sock: socket.socket | None = None
        if sock_path:
            self._sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self._sock.connect(sock_path)
            self._io = self._sock.makefile("rw", encoding="utf-8", newline="\n")
        else:
            self._proc = subprocess.Popen(
                [binary],
                cwd=repo_root,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                text=True,
            )

    def call(self, op: str, **kwargs: Any) -> dict[str, Any]:
        payload = json.dumps({"op": op, **kwargs}) + "\n"
        if self._sock is not None:
            self._io.write(payload)
            self._io.flush()
            line = self._io.readline()
        else:
            assert self._proc and self._proc.stdin and self._proc.stdout
            self._proc.stdin.write(payload)
            self._proc.stdin.flush()
            line = self._proc.stdout.readline()
        if not line:
            raise HarnessError("harnessd connection closed")
        resp = json.loads(line)
        if not resp.get("ok"):
            raise HarnessError(resp.get("error", "unknown harness error"))
        return resp["result"]

    def close(self) -> None:
        if self._sock is not None:
            self._io.close()
            self._sock.close()
        if self._proc is not None:
            if self._proc.stdin:
                self._proc.stdin.close()
            self._proc.wait(timeout=10)
