#!/bin/sh
# Install the coz causal profiler (SPEC §12). Tries the distro package
# first, then a source build. Idempotent — skips if `coz` is already on
# PATH. Run as a user that can apt-install (root in the dev container).
set -eu

if command -v coz >/dev/null 2>&1; then
    echo "coz already installed: $(command -v coz)"
    exit 0
fi

echo "installing coz…"
if apt-get install -y coz-profiler >/dev/null 2>&1; then
    echo "installed via apt: $(command -v coz)"
    exit 0
fi

# Source build fallback.
apt-get install -y python3 libelf-dev libdwarf-dev build-essential git >/dev/null 2>&1 || true
SRC=/opt/coz-src
rm -rf "$SRC"
git clone --depth 1 https://github.com/plasma-umass/coz.git "$SRC"
make -C "$SRC" >/dev/null
make -C "$SRC" install >/dev/null
command -v coz >/dev/null 2>&1 && echo "installed from source: $(command -v coz)" || {
    echo "coz install failed" >&2
    exit 1
}
