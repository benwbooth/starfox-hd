#!/usr/bin/env python3
"""Generate the native SF2 aim-sight texture from independent oracle layers."""

from __future__ import annotations

import argparse
from pathlib import Path

from generate_backdrop import (
    EXPECTED_HEIGHT,
    EXPECTED_WIDTH,
    FNV_OFFSET_BASIS,
    FNV_PRIME,
    MAX_RUN_LENGTH,
    MESEN_CAPTURE_HEIGHT,
    MESEN_TOP_BORDER,
    fnv1a,
    read_ppm,
)


SIGHT_LEFT = 63
SIGHT_TOP = 14
SIGHT_RIGHT = 193
SIGHT_BOTTOM = 174
SIGHT_WIDTH = SIGHT_RIGHT - SIGHT_LEFT + 1
SIGHT_HEIGHT = SIGHT_BOTTOM - SIGHT_TOP + 1
BRACKET_COLOR = (49, 214, 49)
RETICLE_LEFT = 120
RETICLE_TOP = 104
RETICLE_RIGHT = 135
RETICLE_BOTTOM = 119
CLOCK_ICON_LEFT = 147
CLOCK_ICON_TOP = 14
CLOCK_ICON_RIGHT = 162
CLOCK_ICON_BOTTOM = 29
TRANSPARENT = (0, 0, 0, 0)


def source_frame(path: Path) -> bytes:
    width, height, pixels = read_ppm(path)
    if width != EXPECTED_WIDTH:
        raise SystemExit(f"expected a 256-pixel-wide input, found {width}x{height}")
    if height == MESEN_CAPTURE_HEIGHT:
        row_bytes = width * 3
        first_row = MESEN_TOP_BORDER * row_bytes
        pixels = pixels[first_row : first_row + EXPECTED_HEIGHT * row_bytes]
    elif height != EXPECTED_HEIGHT:
        raise SystemExit(f"expected a 256x224 or 256x239 input, found {width}x{height}")
    return pixels


def rgb_at(pixels: bytes, x: int, y: int) -> tuple[int, int, int]:
    offset = (y * EXPECTED_WIDTH + x) * 3
    return tuple(pixels[offset : offset + 3])


def aim_pixels(layer: bytes, oam: bytes) -> list[tuple[int, int, int, int]]:
    output = []
    for y in range(SIGHT_TOP, SIGHT_BOTTOM + 1):
        for x in range(SIGHT_LEFT, SIGHT_RIGHT + 1):
            layer_color = rgb_at(layer, x, y)
            oam_color = rgb_at(oam, x, y)
            if layer_color == BRACKET_COLOR:
                output.append((*layer_color, 255))
            elif (
                (
                    RETICLE_LEFT <= x <= RETICLE_RIGHT
                    and RETICLE_TOP <= y <= RETICLE_BOTTOM
                )
                or (
                    CLOCK_ICON_LEFT <= x <= CLOCK_ICON_RIGHT
                    and CLOCK_ICON_TOP <= y <= CLOCK_ICON_BOTTOM
                )
            ) and oam_color != (0, 0, 0):
                output.append((*oam_color, 255))
            else:
                output.append(TRANSPARENT)
    return output


def encode(
    pixels: list[tuple[int, int, int, int]],
) -> tuple[list[tuple[int, int, int, int]], list[tuple[int, int]]]:
    palette = []
    indices = {}
    runs = []
    for color in pixels:
        index = indices.get(color)
        if index is None:
            index = len(palette)
            indices[color] = index
            palette.append(color)
        if runs and runs[-1][1] == index and runs[-1][0] < MAX_RUN_LENGTH:
            length, _ = runs[-1]
            runs[-1] = (length + 1, index)
        else:
            runs.append((1, index))
    return palette, runs


def rust_source(
    layer_name: str,
    oam_name: str,
    palette: list[tuple[int, int, int, int]],
    runs: list[tuple[int, int]],
    source_hash: int,
) -> str:
    lines = [
        "//! Generated native SF2 aim sight.",
        "//!",
        f"//! Sources: oracle-isolated `{layer_name}` and decoded `{oam_name}`.",
        "//! Regenerate with `tools/sf2/generate_aim_sight.py`.",
        "",
        f"pub const LEFT: i32 = {SIGHT_LEFT};",
        f"pub const TOP: i32 = {SIGHT_TOP};",
        f"pub const WIDTH: usize = {SIGHT_WIDTH};",
        f"pub const HEIGHT: usize = {SIGHT_HEIGHT};",
        "const CHANNELS_PER_PIXEL: usize = 4;",
        "#[cfg(test)]",
        f"const SOURCE_RGBA_FNV1A: u32 = 0x{source_hash:08X};",
        f"const PALETTE: [[u8; CHANNELS_PER_PIXEL]; {len(palette)}] = [",
    ]
    lines.extend(f"    [{r}, {g}, {b}, {a}]," for r, g, b, a in palette)
    lines.extend(["];"] + [f"const RUNS: [(u8, u8); {len(runs)}] = ["])
    for offset in range(0, len(runs), 8):
        chunk = ", ".join(f"({length}, {index})" for length, index in runs[offset : offset + 8])
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
            "    fn aim_sight_decodes_to_its_source_bounds() {",
            "        let rgba = decode_rgba();",
            "        assert_eq!(rgba.len(), WIDTH * HEIGHT * CHANNELS_PER_PIXEL);",
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
    parser.add_argument("isolated_layer", type=Path)
    parser.add_argument("decoded_oam", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    layer = source_frame(args.isolated_layer)
    oam = source_frame(args.decoded_oam)
    pixels = aim_pixels(layer, oam)
    palette, runs = encode(pixels)
    rgba = bytes(channel for pixel in pixels for channel in pixel)
    source_hash = fnv1a(rgba)
    args.output.write_text(
        rust_source(args.isolated_layer.name, args.decoded_oam.name, palette, runs, source_hash),
        encoding="utf-8",
    )
    print(
        f"generated {args.output}: {len(palette)} colors, {len(runs)} runs, "
        f"{SIGHT_WIDTH}x{SIGHT_HEIGHT} pixels, FNV-1a {source_hash:08X}"
    )


if __name__ == "__main__":
    main()
