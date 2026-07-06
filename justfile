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
