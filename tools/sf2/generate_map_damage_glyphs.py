#!/usr/bin/env python3
"""Generate the native strategic-map damage digits from an oracle PPU snapshot.

All source-machine tile and palette interpretation stays in this codegen tool.
The generated Rust module contains only a semantic RGBA digit atlas.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from decode_ppu_background import color, tile_pixel
from generate_backdrop import FNV_OFFSET_BASIS, FNV_PRIME, MAX_RUN_LENGTH, fnv1a


DIGIT_WIDTH = 16
DIGIT_HEIGHT = 14
DIGIT_COUNT = 10
CHANNELS_PER_PIXEL = 4
ATLAS_WIDTH = DIGIT_WIDTH * DIGIT_COUNT
ATLAS_HEIGHT = DIGIT_HEIGHT
BACKGROUND_PALETTE_INDEX = 9
BACKGROUND_PALETTE = 4
BOTTOM_TILE_OFFSET = 16
DIGIT_TOP_TILES = (96, 98, 100, 102, 104, 106, 108, 110, 128, 130)


def render_atlas(vram: bytes, cgram: bytes, character_base: int) -> bytes:
    pixels = bytearray(ATLAS_WIDTH * ATLAS_HEIGHT * CHANNELS_PER_PIXEL)
    for digit, top_tile in enumerate(DIGIT_TOP_TILES):
        for y in range(DIGIT_HEIGHT):
            for x in range(DIGIT_WIDTH):
                tile = top_tile + x // 8
                if y >= 8:
                    tile += BOTTOM_TILE_OFFSET
                palette_index = tile_pixel(
                    vram,
                    character_base,
                    tile,
                    x % 8,
                    y % 8,
                )
                if palette_index == BACKGROUND_PALETTE_INDEX:
                    continue
                red, green, blue = color(
                    cgram,
                    BACKGROUND_PALETTE * 16 + palette_index,
                )
                output = (
                    y * ATLAS_WIDTH + digit * DIGIT_WIDTH + x
                ) * CHANNELS_PER_PIXEL
                pixels[output : output + CHANNELS_PER_PIXEL] = bytes(
                    (red, green, blue, 255)
                )
    return bytes(pixels)


def encode(pixels: bytes) -> tuple[list[tuple[int, ...]], list[tuple[int, int]]]:
    palette: list[tuple[int, ...]] = []
    palette_indices: dict[tuple[int, ...], int] = {}
    runs: list[tuple[int, int]] = []
    for offset in range(0, len(pixels), CHANNELS_PER_PIXEL):
        rgba = tuple(pixels[offset : offset + CHANNELS_PER_PIXEL])
        index = palette_indices.setdefault(rgba, len(palette))
        if index == len(palette):
            palette.append(rgba)
        if runs and runs[-1][1] == index and runs[-1][0] < MAX_RUN_LENGTH:
            length, _ = runs[-1]
            runs[-1] = (length + 1, index)
        else:
            runs.append((1, index))
    return palette, runs


def rust_source(
    source_name: str,
    palette: list[tuple[int, ...]],
    runs: list[tuple[int, int]],
    source_hash: int,
) -> str:
    lines = [
        "//! Generated native SF2 strategic-map damage digit atlas.",
        "//!",
        f"//! Source: oracle PPU snapshot `{source_name}`.",
        "//! Regenerate with `tools/sf2/generate_map_damage_glyphs.py`.",
        "",
        f"pub const WIDTH: usize = {ATLAS_WIDTH};",
        f"pub const HEIGHT: usize = {ATLAS_HEIGHT};",
        f"pub const DIGIT_WIDTH: i32 = {DIGIT_WIDTH};",
        f"pub const DIGIT_HEIGHT: i32 = {DIGIT_HEIGHT};",
        f"const CHANNELS_PER_PIXEL: usize = {CHANNELS_PER_PIXEL};",
        "#[cfg(test)]",
        f"const SOURCE_RGBA_FNV1A: u32 = 0x{source_hash:08X};",
        f"const PALETTE: [[u8; CHANNELS_PER_PIXEL]; {len(palette)}] = [",
    ]
    lines.extend(
        f"    [{red}, {green}, {blue}, {alpha}],"
        for red, green, blue, alpha in palette
    )
    lines.extend([
        "];",
        f"const RUNS: [(u8, u8); {len(runs)}] = [",
    ])
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
            "    fn damage_digits_decode_to_the_certified_atlas() {",
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
    parser.add_argument("vram", type=Path)
    parser.add_argument("cgram", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--characters", type=lambda value: int(value, 0), default=0)
    args = parser.parse_args()

    vram = args.vram.read_bytes()
    cgram = args.cgram.read_bytes()
    if len(vram) != 65_536 or len(cgram) != 512:
        raise SystemExit("expected a complete 64 KiB VRAM and 512-byte CGRAM snapshot")
    pixels = render_atlas(vram, cgram, args.characters)
    palette, runs = encode(pixels)
    args.output.write_text(
        rust_source(args.vram.name, palette, runs, fnv1a(pixels)),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
