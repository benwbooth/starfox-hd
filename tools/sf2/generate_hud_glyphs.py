#!/usr/bin/env python3
"""Generate reusable native SF2 HUD glyphs from an oracle PPU snapshot.

The generated Rust atlas contains ordinary RGBA art. Reading VRAM/CGRAM and
interpreting sprite tiles stays confined to this verification/codegen tool;
the shipping renderer addresses semantic glyphs such as digits and shield
pips.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path

from decode_oam import COLORS_PER_PALETTE, OBJECT_PALETTE_BASE, color
from decode_ppu_background import tile_pixel
from generate_backdrop import FNV_OFFSET_BASIS, FNV_PRIME, MAX_RUN_LENGTH, fnv1a


TILE_SIZE = 8
DIGIT_COUNT = 10
ITEM_DIGIT_COUNT = 4
CHANNELS_PER_PIXEL = 4
ATLAS_WIDTH = 224
ATLAS_HEIGHT = 16

SCORE_DIGITS_LEFT = 0
CLOCK_DIGITS_LEFT = SCORE_DIGITS_LEFT + DIGIT_COUNT * TILE_SIZE
CLOCK_SEPARATOR_LEFT = CLOCK_DIGITS_LEFT + DIGIT_COUNT * TILE_SIZE
SHIELD_PIP_LEFT = CLOCK_SEPARATOR_LEFT + TILE_SIZE
RADAR_PLAYER_LEFT = SHIELD_PIP_LEFT + TILE_SIZE
RADAR_ENEMY_LEFT = RADAR_PLAYER_LEFT + TILE_SIZE
ITEM_DIGITS_LEFT = RADAR_ENEMY_LEFT + TILE_SIZE


@dataclass(frozen=True)
class TileSpec:
    tile: int
    palette: int
    horizontal_flip: bool = False
    vertical_flip: bool = False


def render_tile(
    pixels: bytearray,
    vram: bytes,
    cgram: bytes,
    character_base: int,
    left: int,
    top: int,
    spec: TileSpec,
) -> None:
    for local_y in range(TILE_SIZE):
        source_y = TILE_SIZE - 1 - local_y if spec.vertical_flip else local_y
        for local_x in range(TILE_SIZE):
            source_x = TILE_SIZE - 1 - local_x if spec.horizontal_flip else local_x
            palette_index = tile_pixel(
                vram,
                character_base + spec.tile * 16,
                0,
                source_x,
                source_y,
            )
            if palette_index == 0:
                continue
            red, green, blue = color(
                cgram,
                OBJECT_PALETTE_BASE
                + spec.palette * COLORS_PER_PALETTE
                + palette_index,
            )
            output = (
                (top + local_y) * ATLAS_WIDTH + left + local_x
            ) * CHANNELS_PER_PIXEL
            pixels[output : output + CHANNELS_PER_PIXEL] = bytes(
                (red, green, blue, 255)
            )


def render_atlas(vram: bytes, cgram: bytes, character_base: int) -> bytes:
    pixels = bytearray(ATLAS_WIDTH * ATLAS_HEIGHT * CHANNELS_PER_PIXEL)

    for digit in range(DIGIT_COUNT):
        tile = 112 + digit
        render_tile(
            pixels,
            vram,
            cgram,
            character_base,
            SCORE_DIGITS_LEFT + digit * TILE_SIZE,
            0,
            TileSpec(tile, 1),
        )
        render_tile(
            pixels,
            vram,
            cgram,
            character_base,
            CLOCK_DIGITS_LEFT + digit * TILE_SIZE,
            0,
            TileSpec(tile, 0),
        )

    render_tile(
        pixels,
        vram,
        cgram,
        character_base,
        CLOCK_SEPARATOR_LEFT,
        0,
        TileSpec(126, 0),
    )
    render_tile(
        pixels,
        vram,
        cgram,
        character_base,
        SHIELD_PIP_LEFT,
        0,
        TileSpec(69, 3),
    )
    render_tile(
        pixels,
        vram,
        cgram,
        character_base,
        RADAR_PLAYER_LEFT,
        0,
        TileSpec(66, 0, horizontal_flip=True),
    )
    render_tile(
        pixels,
        vram,
        cgram,
        character_base,
        RADAR_ENEMY_LEFT,
        0,
        TileSpec(109, 3),
    )

    # The item counter uses vertically mirrored half-glyphs. Counts zero and
    # one have dedicated upper halves; the same curved half forms two and
    # three by selecting whether the lower half is mirrored horizontally.
    item_halves = (85, 86, 87, 87)
    for digit, tile in enumerate(item_halves):
        left = ITEM_DIGITS_LEFT + digit * TILE_SIZE
        render_tile(
            pixels,
            vram,
            cgram,
            character_base,
            left,
            0,
            TileSpec(tile, 1),
        )
        render_tile(
            pixels,
            vram,
            cgram,
            character_base,
            left,
            TILE_SIZE,
            TileSpec(
                tile,
                1,
                horizontal_flip=digit == 2,
                vertical_flip=True,
            ),
        )

    return bytes(pixels)


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
                raise SystemExit("HUD glyph atlas needs more than 256 colors")
            palette_indices[rgba] = index
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
        "//! Generated native SF2 HUD glyph atlas.",
        "//!",
        f"//! Source: oracle PPU snapshot `{source_name}`.",
        "//! Regenerate with `tools/sf2/generate_hud_glyphs.py`.",
        "",
        f"pub const WIDTH: usize = {ATLAS_WIDTH};",
        f"pub const HEIGHT: usize = {ATLAS_HEIGHT};",
        f"pub const GLYPH_SIZE: i32 = {TILE_SIZE};",
        f"pub const ITEM_DIGIT_HEIGHT: i32 = {TILE_SIZE * 2};",
        f"pub const SCORE_DIGITS_LEFT: i32 = {SCORE_DIGITS_LEFT};",
        f"pub const CLOCK_DIGITS_LEFT: i32 = {CLOCK_DIGITS_LEFT};",
        f"pub const CLOCK_SEPARATOR_LEFT: i32 = {CLOCK_SEPARATOR_LEFT};",
        f"pub const SHIELD_PIP_LEFT: i32 = {SHIELD_PIP_LEFT};",
        f"pub const RADAR_PLAYER_LEFT: i32 = {RADAR_PLAYER_LEFT};",
        f"pub const RADAR_ENEMY_LEFT: i32 = {RADAR_ENEMY_LEFT};",
        f"pub const ITEM_DIGITS_LEFT: i32 = {ITEM_DIGITS_LEFT};",
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
            "    fn hud_glyphs_decode_to_the_certified_atlas() {",
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
    parser.add_argument("--characters", type=lambda value: int(value, 0), required=True)
    args = parser.parse_args()

    vram = args.vram.read_bytes()
    cgram = args.cgram.read_bytes()
    if len(vram) != 65_536 or len(cgram) != 512:
        raise SystemExit("expected a complete 64 KiB VRAM and 512-byte CGRAM snapshot")
    pixels = render_atlas(vram, cgram, args.characters)
    palette, runs = encode(pixels)
    source_hash = fnv1a(pixels)
    args.output.write_text(
        rust_source(args.vram.name, palette, runs, source_hash), encoding="utf-8"
    )
    print(
        f"generated {args.output}: {len(palette)} colors, {len(runs)} runs, "
        f"FNV-1a {source_hash:08X}"
    )


if __name__ == "__main__":
    main()
