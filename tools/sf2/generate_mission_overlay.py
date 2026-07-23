#!/usr/bin/env python3
"""Generate native SF2 mission overlay art from isolated oracle frames.

The layer-isolated frames make it possible to retain invariant Super FX HUD
pixels without baking world geometry into the port. The output contains a
transparent static overlay plus the cool and warm blaster palette phases.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from generate_aim_sight import source_frame
from generate_backdrop import (
    EXPECTED_HEIGHT,
    EXPECTED_WIDTH,
    FNV_OFFSET_BASIS,
    FNV_PRIME,
    MAX_RUN_LENGTH,
    fnv1a,
)


CHANNELS_PER_PIXEL = 4
ATLAS_WIDTH = EXPECTED_WIDTH
STATIC_HEIGHT = EXPECTED_HEIGHT
BLASTER_WIDTH = 32
BLASTER_HEIGHT = 8
BLASTER_LEFT = 208
BLASTER_TOP = 167
COOL_ATLAS_TOP = STATIC_HEIGHT
WARM_ATLAS_TOP = COOL_ATLAS_TOP + BLASTER_HEIGHT
ATLAS_HEIGHT = STATIC_HEIGHT + BLASTER_HEIGHT * 2

# Stable across the isolated oracle sequence: the opening target class marker
# and the two right-edge targeting guides. Moving target labels and all world
# geometry are deliberately excluded.
STATIC_REGIONS = (
    (132, 26, 8, 8),
    (232, 91, 7, 1),
    (232, 131, 7, 1),
)


def rgb_at(frame: bytes, x: int, y: int) -> tuple[int, int, int]:
    offset = (y * EXPECTED_WIDTH + x) * 3
    return tuple(frame[offset : offset + 3])


def store_pixel(
    atlas: bytearray,
    left: int,
    top: int,
    color: tuple[int, int, int],
) -> None:
    if color == (0, 0, 0):
        return
    offset = (top * ATLAS_WIDTH + left) * CHANNELS_PER_PIXEL
    atlas[offset : offset + CHANNELS_PER_PIXEL] = bytes((*color, 255))


def render_atlas(cool_frame: bytes, warm_frame: bytes) -> bytes:
    atlas = bytearray(ATLAS_WIDTH * ATLAS_HEIGHT * CHANNELS_PER_PIXEL)
    for left, top, width, height in STATIC_REGIONS:
        for y in range(top, top + height):
            for x in range(left, left + width):
                store_pixel(atlas, x, y, rgb_at(cool_frame, x, y))

    for frame, atlas_top in (
        (cool_frame, COOL_ATLAS_TOP),
        (warm_frame, WARM_ATLAS_TOP),
    ):
        for local_y in range(BLASTER_HEIGHT):
            for local_x in range(BLASTER_WIDTH):
                store_pixel(
                    atlas,
                    local_x,
                    atlas_top + local_y,
                    rgb_at(frame, BLASTER_LEFT + local_x, BLASTER_TOP + local_y),
                )
    return bytes(atlas)


def encode(pixels: bytes) -> tuple[list[tuple[int, ...]], list[tuple[int, int]]]:
    palette: list[tuple[int, ...]] = []
    palette_indices: dict[tuple[int, ...], int] = {}
    runs: list[tuple[int, int]] = []
    for offset in range(0, len(pixels), CHANNELS_PER_PIXEL):
        rgba = tuple(pixels[offset : offset + CHANNELS_PER_PIXEL])
        index = palette_indices.get(rgba)
        if index is None:
            index = len(palette)
            if index > 255:
                raise SystemExit("mission overlay needs more than 256 colors")
            palette_indices[rgba] = index
            palette.append(rgba)
        if runs and runs[-1][1] == index and runs[-1][0] < MAX_RUN_LENGTH:
            length, _ = runs[-1]
            runs[-1] = (length + 1, index)
        else:
            runs.append((1, index))
    return palette, runs


def rust_source(
    cool_name: str,
    warm_name: str,
    palette: list[tuple[int, ...]],
    runs: list[tuple[int, int]],
    source_hash: int,
) -> str:
    lines = [
        "//! Generated native SF2 mission overlay art.",
        "//!",
        f"//! Sources: isolated oracle frames `{cool_name}` and `{warm_name}`.",
        "//! Regenerate with `tools/sf2/generate_mission_overlay.py`.",
        "",
        f"pub const WIDTH: usize = {ATLAS_WIDTH};",
        f"pub const HEIGHT: usize = {ATLAS_HEIGHT};",
        f"pub const STATIC_HEIGHT: i32 = {STATIC_HEIGHT};",
        f"pub const BLASTER_WIDTH: i32 = {BLASTER_WIDTH};",
        f"pub const BLASTER_HEIGHT: i32 = {BLASTER_HEIGHT};",
        f"pub const BLASTER_LEFT: i32 = {BLASTER_LEFT};",
        f"pub const BLASTER_TOP: i32 = {BLASTER_TOP};",
        f"pub const COOL_ATLAS_TOP: i32 = {COOL_ATLAS_TOP};",
        f"pub const WARM_ATLAS_TOP: i32 = {WARM_ATLAS_TOP};",
        f"const CHANNELS_PER_PIXEL: usize = {CHANNELS_PER_PIXEL};",
        "#[cfg(test)]",
        f"const SOURCE_RGBA_FNV1A: u32 = 0x{source_hash:08X};",
        f"const PALETTE: [[u8; CHANNELS_PER_PIXEL]; {len(palette)}] = [",
    ]
    lines.extend(
        f"    [{red}, {green}, {blue}, {alpha}],"
        for red, green, blue, alpha in palette
    )
    lines.extend(["];", f"const RUNS: [(u8, u8); {len(runs)}] = ["])
    for offset in range(0, len(runs), 8):
        chunk = ", ".join(
            f"({length}, {index})" for length, index in runs[offset : offset + 8]
        )
        lines.append(f"    {chunk},")
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
            "    fn mission_overlay_decodes_to_the_certified_atlas() {",
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
    parser.add_argument("cool_frame", type=Path)
    parser.add_argument("warm_frame", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    cool_frame = source_frame(args.cool_frame)
    warm_frame = source_frame(args.warm_frame)
    atlas = render_atlas(cool_frame, warm_frame)
    palette, runs = encode(atlas)
    source_hash = fnv1a(atlas)
    args.output.write_text(
        rust_source(
            args.cool_frame.name,
            args.warm_frame.name,
            palette,
            runs,
            source_hash,
        ),
        encoding="utf-8",
    )
    print(
        f"generated {args.output}: {len(palette)} colors, {len(runs)} runs, "
        f"FNV-1a {source_hash:08X}"
    )


if __name__ == "__main__":
    main()
