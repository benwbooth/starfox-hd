#!/usr/bin/env bash
# Star Fox HD — one-command launcher.
#
#   ./play.sh              # build (release) if needed, then play
#   ./play.sh --help       # forwarded to the binary
#   SF_PROFILE=debug ./play.sh
#
# Enters the nix dev shell (SDL3 / GL / Vulkan / Wayland / libdecor) and runs
# scripts/run.sh, which selects native Wayland + Vulkan when a Wayland session
# is present (falls back to the system default otherwise).
set -euo pipefail
cd "$(dirname "$0")"
export SF_PROFILE="${SF_PROFILE:-release}"
exec nix develop --command ./scripts/run.sh "$@"
