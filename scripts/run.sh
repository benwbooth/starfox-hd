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

# --- Display backend: prefer native Wayland + Vulkan (wgpu) when a Wayland
# session is present. This avoids Xwayland entirely. SDL3 needs libdecor for
# window decorations on Wayland (provided via the flake's runtimeLibs), and
# wgpu needs the system Vulkan userspace driver. Everything here is overridable
# and only engages under Wayland, so X11-only setups are unaffected.
if [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
    export SDL_VIDEO_DRIVER="${SDL_VIDEO_DRIVER:-wayland}"
    export WGPU_BACKEND="${WGPU_BACKEND:-vulkan}"
    if [[ -d /run/opengl-driver/lib ]]; then
        export LD_LIBRARY_PATH="/run/opengl-driver/lib:${LD_LIBRARY_PATH:-}"
    fi
    # Pin the Vulkan ICD to the installed GPU so the loader doesn't scan (and
    # potentially stall on) every foreign-GPU driver in the NixOS graphics stack.
    if [[ -z "${VK_ICD_FILENAMES:-}" ]]; then
        icddir=/run/opengl-driver/share/vulkan/icd.d
        vendor="$(cat /sys/class/drm/card0/device/vendor 2>/dev/null || true)"
        case "$vendor" in
            0x1002) icd="$icddir/radeon_icd.x86_64.json" ;;   # AMD (RADV)
            0x8086) icd="$icddir/intel_icd.x86_64.json"  ;;   # Intel (ANV)
            0x10de) icd="$icddir/nouveau_icd.x86_64.json" ;;  # NVIDIA (NVK)
            *)      icd="" ;;
        esac
        [[ -n "$icd" && -f "$icd" ]] && export VK_ICD_FILENAMES="$icd"
    fi
fi

PROFILE="${SF_PROFILE:-debug}"
BIN="rust/target/${PROFILE}/starfox-hd-rs"

echo "Checking starfox-hd-rs build ($PROFILE)..." >&2
if [[ "$PROFILE" == "release" ]]; then
    ( cd rust && cargo build --release -p sf-app )
else
    ( cd rust && cargo build -p sf-app )
fi

exec "$BIN" "$@"
