# Isolation & anti-reward-hacking

The guarantees only matter if the proposer cannot cheat. Peltier's defense
is **structural**: the agent is denied the capabilities it would need to
manufacture a false win, at the OS level, not by prompt.

## What holds even if the proposer is adversarial

- **Writes go through one door.** `propose_patch` is the only harness-
  mediated write path: a diff is path-allowlisted (relative paths only, no
  `..`, no `.git`, nothing outside `targets/<name>/workspace`) and then
  `git apply`-ed rooted in that workspace. A patch aimed at `config/` or
  `crates/` is refused before any apply. Verified against a malicious
  accept-threshold diff.
- **The ledger is append-only.** `UPDATE` and `DELETE` are refused by
  SQLite triggers; `accepted` rows are written only by the `verdict`
  binary after the gated pipeline. The agent cannot forge a passing
  verdict through the harness.
- **The baseline is unbypassable.** It is rebuilt from a pristine checkout;
  the agent's workspace never becomes the comparison.

## OS-level isolation (SPEC §10)

The SDK's tool allow/deny list is defense-in-depth, not the boundary —
`scripts/agent-isolated.sh` provides the real one, verified by
`just isolation-check` (19 checks, both modes):

- **`mountns` mode** — the whole agent process tree runs in a private mount
  namespace with the repo bind-mounted **read-only** and `CAP_SYS_ADMIN`/
  `CAP_SYS_PTRACE` dropped from the bounding set. Repo writes fail with
  `EROFS`; a forged `INSERT`, a direct `verdict` invocation, a `mount -o
  remount,rw`, and even userns re-entry to undo the mount are all refused.
- **`user` mode** — the agent runs as an unprivileged user whose only write
  path into the repo is the harnessd Unix socket; the trust layer is owned
  by another uid. This is the production shape (`docs/infra.md`).

## Network isolation on the verdict path

Target code should not be able to phone home during a measurement. What
is actually enforced, stated honestly:

- **The agent path is wrapped.** When `harnessd` launches the verdict
  pipeline, it execs through `scripts/no-net.sh`, which runs it inside an
  unprivileged **network namespace** (`unshare --net --map-current-user`).
  `HOTPATH_VERDICT_WRAPPER` overrides the wrapper.
- **It fails closed.** Where namespaces are unavailable the wrapper **exits
  97 without running the pipeline** — it does not degrade silently. The one
  bypass, `HOTPATH_ALLOW_UNISOLATED=1`, runs the pipeline **with full
  network access**; it warns loudly on stderr *and* the resulting ledger row
  records the isolation note `no-net.sh (HOTPATH_ALLOW_UNISOLATED=1 —
  network NOT isolated)`. A run cannot claim an isolation it did not have.
- **In CI**, the bench workload runs under `docker run --network=none`.

Two limits, stated plainly rather than buried:

- **Only the harnessd (agent) path is wrapped.** A human who invokes
  `just verdict` directly runs it **unwrapped on the host**; those rows are
  stamped `isolation: "unwrapped-host"` in the ledger. The wrapper is a
  property of the agent pipeline, not of the `verdict` binary.
- **Full-container isolation is still an open gap.** The namespace isolates
  the *network* — not the filesystem, not the syscall surface. The
  fully containerized, seccomp-restricted bench container of SPEC §10 is
  **not built**. The agent-side OS boundary above is a separate mechanism
  and *is* shipped.

## The two times the pipeline was wrong

Anti-reward-hacking is not "it never fails" — it is "when it fails, the
audit catches it and the system gets harder." That happened twice, and
both are documented, not hidden:

1. A leaking teardown patch the bench loved was **auto-accepted** and then
   **overturned** by the 100% human audit's LeakSanitizer run. The fix made
   the sanitizer gate machine-enforced on every accept.
2. During the [adversarial review](https://github.com/bdbrown4/peltier.io/blob/main/results/adversarial-review.md),
   the ROI report generator was found to mint a clean dollar figure for that
   same overturned row (it trusted the immutable ledger's historical
   `accepted` verdict). The fix made the report treat an accepted-but-not-
   sanitizer-clean row as **not shippable**.

Both defects were found by attacking the system's own output, fixed, and
turned into permanent gates. **Zero false accepts shipped.**
