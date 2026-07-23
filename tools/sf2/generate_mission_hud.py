#!/usr/bin/env python3
"""Generate the static native SF2 mission HUD from an oracle OAM snapshot.

Only invariant chrome is retained: labels, icons, meter frames, and the radar
grid. Score/time digits, shield pips, item count, radar contacts, and the aim
sight remain independently rendered from live typed game state.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from decode_oam import (
    FRAME_HEIGHT,
    FRAME_WIDTH,
    OBJECT_PALETTE_BASE,
    COLORS_PER_PALETTE,
    Sprite,
    color,
    sprite_pixel,
    sprites,
)
from generate_backdrop import FNV_OFFSET_BASIS, FNV_PRIME, MAX_RUN_LENGTH, fnv1a


STATIC_SPRITE_INDICES = frozenset(
    (
        *range(4, 12),
        *range(13, 25),
        *range(30, 66),
        74,
        83,
        84,
        85,
    )
)
CHANNELS_PER_PIXEL = 4
TRANSPARENT = (0, 0, 0, 0)


def render_static(vram: bytes, cgram: bytes, oam: bytes, base_word: int) -> bytes:
    pixels = bytearray(FRAME_WIDTH * FRAME_HEIGHT * CHANNELS_PER_PIXEL)
    selected: list[Sprite] = [
        sprite for sprite in sprites(oam) if sprite.index in STATIC_SPRITE_INDICES
    ]
    for sprite in reversed(selected):
        for local_y in range(sprite.size):
            screen_y = (sprite.y + local_y) & 255
            if screen_y >= FRAME_HEIGHT:
                continue
            for local_x in range(sprite.size):
                screen_x = sprite.x + local_x
                if not 0 <= screen_x < FRAME_WIDTH:
                    continue
                palette_index = sprite_pixel(vram, base_word, sprite, local_x, local_y)
                if palette_index == 0:
                    continue
                red, green, blue = color(
                    cgram,
                    OBJECT_PALETTE_BASE
                    + sprite.palette * COLORS_PER_PALETTE
                    + palette_index,
                )
                output = (screen_y * FRAME_WIDTH + screen_x) * CHANNELS_PER_PIXEL
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
        index = palette_indices.get(rgba)
        if index is None:
            index = len(palette)
            if index > 255:
                raise SystemExit("mission HUD needs more than 256 colors")
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
        "//! Generated native SF2 mission HUD chrome.",
        "//!",
        f"//! Source: oracle OAM snapshot `{source_name}`.",
        "//! Regenerate with `tools/sf2/generate_mission_hud.py`.",
        "",
        f"pub const WIDTH: usize = {FRAME_WIDTH};",
        f"pub const HEIGHT: usize = {FRAME_HEIGHT};",
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
            "    fn mission_hud_decodes_to_the_source_frame() {",
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
    parser.add_argument("vram", type=Path)
    parser.add_argument("cgram", type=Path)
    parser.add_argument("oam", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--characters", type=lambda value: int(value, 0), required=True)
    args = parser.parse_args()

    vram = args.vram.read_bytes()
    cgram = args.cgram.read_bytes()
    oam = args.oam.read_bytes()
    if len(vram) != 65_536 or len(cgram) != 512 or len(oam) != 544:
        raise SystemExit("expected a complete 64 KiB VRAM, 512-byte CGRAM, and OAM snapshot")
    pixels = render_static(vram, cgram, oam, args.characters)
    palette, runs = encode(pixels)
    source_hash = fnv1a(pixels)
    args.output.write_text(
        rust_source(args.oam.name, palette, runs, source_hash), encoding="utf-8"
    )
    print(
        f"generated {args.output}: {len(palette)} colors, {len(runs)} runs, "
        f"FNV-1a {source_hash:08X}"
    )


if __name__ == "__main__":
    main()
