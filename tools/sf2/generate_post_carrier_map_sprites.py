#!/usr/bin/env python3
"""Generate semantic map actors for a certified late-campaign checkpoint.

The oracle-side backdrop generator removes the complete object layer.  This
tool subtracts that backdrop from bounded, named actor regions and emits a
normal transparent RGBA atlas for the shipping renderer.  No object memory or
source-machine addressing crosses this asset boundary.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path

from generate_backdrop import FNV_OFFSET_BASIS, FNV_PRIME, fnv1a
from generate_map_sprites import encode, read_capture
from generate_strategic_map_variant import generated_backdrop_rgb


CHANNELS_PER_PIXEL = 4
RGB_CHANNELS_PER_PIXEL = 3
CELL_SIZE = 48
ATLAS_HEIGHT = CELL_SIZE


@dataclass(frozen=True)
class ActorRegion:
    name: str
    left: int
    top: int
    width: int
    height: int
    force_opaque: tuple[tuple[int, int], ...] = ()


def opaque_region(name: str, left: int, top: int, width: int, height: int) -> ActorRegion:
    return ActorRegion(
        name,
        left,
        top,
        width,
        height,
        tuple((x, y) for y in range(height) for x in range(width)),
    )


ACTORS = (
    ActorRegion("NORTH_INSTALLATION_LEFT", 16, 14, 48, 48),
    ActorRegion("SOUTH_INSTALLATION_LEFT", 208, 110, 48, 48),
    ActorRegion("DISTANT_CARRIER_LEFT", 220, 7, 36, 48),
    ActorRegion("ENEMY_FORMATION_LEFT", 25, 78, 33, 33),
    ActorRegion("ATTACKING_FIGHTER_LEFT", 125, 112, 42, 24),
    ActorRegion("PATROL_SHIP_LEFT", 12, 150, 16, 16),
    ActorRegion("UNKNOWN_SIGNAL_LEFT", 54, 123, 16, 16),
    ActorRegion("MISSILE_TRAIL_LEFT", 8, 80, 16, 16),
    ActorRegion("MISSILE_LEFT", 24, 111, 16, 16),
    ActorRegion("FIGHTER_PROJECTILE_LEFT", 49, 148, 16, 16),
    ActorRegion(
        "CRAFT_CURSOR_LEFT",
        76,
        98,
        8,
        8,
        (
            (2, 6),
            (3, 6),
            (4, 6),
            (5, 6),
            (2, 7),
            (3, 7),
            (4, 7),
            (5, 7),
        ),
    ),
    ActorRegion("PRIMARY_SHIELD_LEFT", 114, 197, 40, 8),
    ActorRegion("WINGMATE_SHIELD_LEFT", 114, 205, 40, 8),
    ActorRegion("GAUGE_BARS_LEFT", 240, 182, 8, 16),
)
POST_LEON_ACTORS = tuple(
    ActorRegion("MISSILE_TRAIL_LEFT", 8, 80, 16, 32)
    if actor.name == "MISSILE_TRAIL_LEFT"
    else opaque_region("CRAFT_MARKER_LEFT", 72, 96, 16, 24)
    if actor.name == "CRAFT_CURSOR_LEFT"
    else actor
    for actor in ACTORS
) + (opaque_region("WINGMATE_PILOT_LEFT", 93, 196, 16, 16),)
POST_MIRAGE_ACTORS = (
    ActorRegion("NORTH_INSTALLATION_LEFT", 16, 14, 48, 48),
    ActorRegion("SOUTH_INSTALLATION_LEFT", 208, 110, 48, 48),
    ActorRegion("DISTANT_CARRIER_LEFT", 220, 7, 36, 48),
    ActorRegion("ENEMY_FORMATION_LEFT", 25, 78, 33, 33),
    ActorRegion("ATTACKING_FIGHTER_LEFT", 14, 120, 24, 24),
    ActorRegion("PATROL_SHIP_LEFT", 12, 150, 16, 16),
    ActorRegion("DEFENSE_PLATFORM_LEFT", 72, 102, 24, 24),
    ActorRegion("UNKNOWN_SIGNAL_LEFT", 39, 143, 24, 24),
    ActorRegion("FIGHTER_PROJECTILE_LEFT", 104, 113, 8, 8),
    opaque_region("CRAFT_MARKER_LEFT", 72, 102, 16, 24),
    ActorRegion("PRIMARY_SHIELD_LEFT", 114, 197, 40, 8),
    ActorRegion("WINGMATE_SHIELD_LEFT", 114, 205, 40, 8),
    ActorRegion("GAUGE_BARS_LEFT", 240, 182, 8, 16),
    opaque_region("WINGMATE_PILOT_LEFT", 93, 196, 16, 16),
)


def render_atlas(
    capture: bytes, backdrop: bytes, actors: tuple[ActorRegion, ...]
) -> bytes:
    atlas_width = CELL_SIZE * len(actors)
    pixels = bytearray(atlas_width * ATLAS_HEIGHT * CHANNELS_PER_PIXEL)
    for actor_index, actor in enumerate(actors):
        atlas_left = actor_index * CELL_SIZE
        for local_y in range(actor.height):
            screen_y = actor.top + local_y
            if not 0 <= screen_y < 224:
                continue
            for local_x in range(actor.width):
                screen_x = actor.left + local_x
                if not 0 <= screen_x < 256:
                    continue
                screen_offset = (
                    screen_y * 256 + screen_x
                ) * RGB_CHANNELS_PER_PIXEL
                visible = capture[
                    screen_offset : screen_offset + RGB_CHANNELS_PER_PIXEL
                ]
                if visible == backdrop[
                    screen_offset : screen_offset + RGB_CHANNELS_PER_PIXEL
                ] and (local_x, local_y) not in actor.force_opaque:
                    continue
                atlas_offset = (
                    local_y * atlas_width + atlas_left + local_x
                ) * CHANNELS_PER_PIXEL
                pixels[atlas_offset : atlas_offset + CHANNELS_PER_PIXEL] = bytes(
                    (*visible, 255)
                )
    return bytes(pixels)


def rust_source(
    source_name: str,
    palette: list[tuple[int, ...]],
    runs: list[tuple[int, int]],
    source_hash: int,
    variant_name: str,
    test_name: str,
    actors: tuple[ActorRegion, ...],
) -> str:
    atlas_width = CELL_SIZE * len(actors)
    lines = [
        f"//! Generated semantic actors for the native {variant_name} map.",
        "//!",
        f"//! Source: certified oracle capture `{source_name}`.",
        "//! Regenerate with `tools/sf2/generate_post_carrier_map_sprites.py`.",
        "",
        f"pub const WIDTH: usize = {atlas_width};",
        f"pub const HEIGHT: usize = {ATLAS_HEIGHT};",
        f"pub const CELL_SIZE: i32 = {CELL_SIZE};",
    ]
    for actor_index, actor in enumerate(actors):
        lines.append(
            f"pub const {actor.name}: i32 = {actor_index * CELL_SIZE};"
        )
    lines.extend(
        [
            f"const CHANNELS_PER_PIXEL: usize = {CHANNELS_PER_PIXEL};",
            "#[cfg(test)]",
            f"const SOURCE_RGBA_FNV1A: u32 = 0x{source_hash:08X};",
            f"const PALETTE: [[u8; CHANNELS_PER_PIXEL]; {len(palette)}] = [",
        ]
    )
    lines.extend(
        f"    [{red}, {green}, {blue}, {alpha}],"
        for red, green, blue, alpha in palette
    )
    lines.extend(["];", f"const RUNS: [(u8, u16); {len(runs)}] = ["])
    for offset in range(0, len(runs), 8):
        lines.append(
            "    "
            + ", ".join(
                f"({length}, {palette_index})"
                for length, palette_index in runs[offset : offset + 8]
            )
            + ","
        )
    lines.extend(
        [
            "];",
            "",
            "pub fn decode_rgba() -> Vec<u8> {",
            "    let mut rgba = Vec::with_capacity(WIDTH * HEIGHT * CHANNELS_PER_PIXEL);",
            "    for (length, palette_index) in RUNS {",
            "        let color = PALETTE[usize::from(palette_index)];",
            "        for _ in 0..length {",
            "            rgba.extend_from_slice(&color);",
            "        }",
            "    }",
            "    assert_eq!(rgba.len(), WIDTH * HEIGHT * CHANNELS_PER_PIXEL);",
            "    rgba",
            "}",
            "",
            "#[cfg(test)]",
            "mod tests {",
            "    use super::*;",
            "",
            "    #[test]",
            f"    fn {test_name}() {{",
            "        let rgba = decode_rgba();",
            f"        let hash = rgba.into_iter().fold({FNV_OFFSET_BASIS}, |value, byte| {{",
            f"            (value ^ u32::from(byte)).wrapping_mul({FNV_PRIME})",
            "        });",
            "        assert_eq!(hash, SOURCE_RGBA_FNV1A);",
            "    }",
            "}",
            "",
        ]
    )
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("capture", type=Path)
    parser.add_argument("backdrop", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--variant-name", default="post-carrier")
    parser.add_argument(
        "--test-name",
        default="post_carrier_actors_decode_to_the_certified_atlas",
    )
    args = parser.parse_args()

    actors = (
        POST_MIRAGE_ACTORS
        if args.variant_name == "post-mirage"
        else POST_LEON_ACTORS
        if args.variant_name == "post-leon"
        else ACTORS
    )
    pixels = render_atlas(
        read_capture(args.capture), generated_backdrop_rgb(args.backdrop), actors
    )
    palette, runs = encode(pixels)
    args.output.write_text(
        rust_source(
            args.capture.name,
            palette,
            runs,
            fnv1a(pixels),
            args.variant_name,
            args.test_name,
            actors,
        ),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
