"""hotpath agent (SPEC §3.5).

The agent is the *untrusted* half of the system. It reads profiles, the
ledger, and the playbook; it proposes patches and requests verdicts. It
never holds a shell on the host, never writes outside targets/<name>/,
and never touches crates/, config/, or corpora/ — enforced by the
harness and filesystem permissions, not by this code or any prompt.
"""

__version__ = "0.1.0"
