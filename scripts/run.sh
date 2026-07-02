#!/usr/bin/env bash
# Canonical launcher for the Rust Star Fox HD binary.
#
# Assets and config are resolved cwd-relative (starfox.ini -> AssetDir=data),
# so this script always runs the binary FROM THE REPO ROOT. No CMake / build
# staging step is required — `data/` and `starfox.ini` are read in place.
#
# Run inside the nix devshell (provides SDL3 / libGL on LD_LIBRARY_PATH):
#   nix develop --command ./scripts/run.sh
# Extra args are forwarded to the binary (e.g. --config, --asset-root,
# --shader-dir). Env knobs (SF_HIDDEN, SF_AUTOPLAY, SF_MAX_TICKS,
# SF_DUMP_PPM, ...) are honoured as usual.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

PROFILE="${SF_PROFILE:-debug}"
BIN="rust/target/${PROFILE}/starfox-hd-rs"

if [[ ! -x "$BIN" ]]; then
    echo "Building starfox-hd-rs ($PROFILE)..." >&2
    if [[ "$PROFILE" == "release" ]]; then
        ( cd rust && cargo build --release -p sf-app )
    else
        ( cd rust && cargo build -p sf-app )
    fi
fi

exec "$BIN" "$@"
