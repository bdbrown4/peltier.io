//! harnessd — the only door between the agent and the trust layer
//! (SPEC §3.5, §10). One JSON request per stdin line, one JSON response
//! per stdout line. The agent gets exactly seven operations (the six
//! synchronous ops plus the async read_verdict poll); everything else
//! (shell, trust-layer writes, patches outside the target workspace)
//! simply has no code path here. Phase 1 caveat, recorded: same-uid
//! filesystem read-only enforcement still requires the separate-user/
//! container setup (Phase 1 infra gap).
//!
//! The daemon itself is POSIX-only (Unix sockets, setsid); the diff
//! allowlist is a portable module so its escape-regression tests run on
//! every development platform.

// On non-Unix only the allowlist tests compile against this module.
#[cfg_attr(not(unix), allow(dead_code))]
mod allowlist;
#[cfg(unix)]
mod daemon;

#[cfg(unix)]
fn main() -> anyhow::Result<()> {
    daemon::run()
}

#[cfg(not(unix))]
fn main() {
    eprintln!("harnessd requires a Unix host (Unix sockets, setsid)");
    std::process::exit(1);
}
