# hotpath task runner. `just --list` for a summary.

# Build the trust-layer workspace
build:
    cargo build --workspace

# Run all trust-layer unit tests
test:
    cargo test --workspace

# Lint: warnings are errors in the trust layer
lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check

# A/A self-test of the bench harness against a sample command.
# Must print a null verdict; an "accept" is a calibration failure.
aa cmd="sleep 0.05":
    cargo run -p bench-runner -- --config config/accept.toml aa --cmd "{{cmd}}"

# Interleaved A/B comparison of two shell commands
compare baseline candidate:
    cargo run -p bench-runner -- --config config/accept.toml compare \
        --baseline "{{baseline}}" --candidate "{{candidate}}"

# Verify a target corpus against its pinned manifest
# (wired through diff-test CLI once it exists; manual check for now)
pin-check target:
    cd corpora/{{target}} && sh gen-corpus.sh

# Automated calibration: N A/A sessions (<5% false-positive required) +
# N injected-slowdown sessions (>=95% detection required), JSON evidence
calibrate cmd out sessions="20":
    cargo run -p bench-runner -- --config config/accept.toml calibrate \
        --cmd "{{cmd}}" --sessions {{sessions}} --out "{{out}}"

# Equivalence gates for a target: corpus pin (refuses on mismatch) ->
# upstream tests -> golden replay. Fuzz/sanitizers stay per-attempt.
gates target:
    cargo run -p diff-test -- {{target}}

# One-shot attempt verdict: gates on candidate -> interleaved A/B vs a
# pristine-rebuilt baseline -> ledger row. Extra flags pass through
# (--patch-file, --needs-human-review, --baseline-bin).
verdict target candidate run_id class hypothesis hotspot *flags:
    cargo run -p verdict -- {{target}} --rebuild-baseline \
        --candidate-bin "{{candidate}}" --run-id "{{run_id}}" \
        --playbook-class {{class}} --hypothesis "{{hypothesis}}" \
        --hotspot "{{hotspot}}" {{flags}}

# One unattended agent attempt behind the SPEC §10 OS boundary:
# harnessd (trusted uid) on a Unix socket, agent loop as user hpagent.
agent-attempt target run_id max_turns="40":
    sh scripts/agent-isolated.sh {{target}} {{run_id}} {{max_turns}}

# Verify the OS boundary from the agent user's side (negative + positive)
isolation-check:
    sh scripts/isolation-check.sh
