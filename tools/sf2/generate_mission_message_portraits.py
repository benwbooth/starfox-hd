#!/usr/bin/env python3
"""Generate the exact SF2 mission-radio iris and Slippy portrait atlas."""

from __future__ import annotations

import argparse
from pathlib import Path


PORTRAIT_LEFT = 49
PORTRAIT_TOP = 173
PORTRAIT_WIDTH = 32
PORTRAIT_HEIGHT = 40
CHANNELS_PER_PIXEL = 4
MAX_RUN_LENGTH = 255
FNV_OFFSET_BASIS = 2_166_136_261
FNV_PRIME = 16_777_619

FRAME_SOURCES = (
    ("THIN_LINE_FRAME", 510),
    ("EMPTY_PANEL_FRAME", 514),
    ("SPARSE_INTERFERENCE_FRAME", 518),
    ("DENSE_INTERFERENCE_FRAME", 519),
    ("FULL_INTERFERENCE_FRAME", 520),
    ("SLIPPY_TALKING_FRAME", 522),
    ("SLIPPY_STILL_FRAME", 524),
)

# Exact colors written by the mission-radio artwork. Dark blues and the one
# bright flight-scene pixel around its rounded corners remain transparent.
PORTRAIT_COLORS = frozenset(
    (
        (24, 41, 24),
        (214, 231, 214),
        (247, 255, 247),
        (165, 181, 165),
        (57, 74, 57),
        (107, 123, 107),
        (74, 74, 222),
        (255, 222, 107),
        (123, 165, 255),
        (239, 173, 66),
        (206, 82, 41),
        (49, 214, 49),
        (24, 24, 156),
        (123, 24, 24),
        (181, 222, 255),
    )
)


def read_ppm(path: Path) -> tuple[int, int, bytes]:
    parts = path.read_bytes().split(maxsplit=4)
    if len(parts) != 5 or parts[0] != b"P6" or parts[3] != b"255":
        raise SystemExit(f"{path}: expected a binary RGB PPM with maximum value 255")
    width = int(parts[1])
    height = int(parts[2])
    pixels = parts[4]
    if len(pixels) != width * height * 3:
        raise SystemExit(f"{path}: pixel payload does not match its dimensions")
    return width, height, pixels


def portrait_frame(path: Path) -> bytes:
    width, height, pixels = read_ppm(path)
    if width < PORTRAIT_LEFT + PORTRAIT_WIDTH or height < PORTRAIT_TOP + PORTRAIT_HEIGHT:
        raise SystemExit(f"{path}: frame does not contain the mission-radio portrait")
    output = bytearray()
    for y in range(PORTRAIT_TOP, PORTRAIT_TOP + PORTRAIT_HEIGHT):
        row = (y * width + PORTRAIT_LEFT) * 3
        for x in range(PORTRAIT_WIDTH):
            offset = row + x * 3
            rgb = tuple(pixels[offset : offset + 3])
            output.extend((*rgb, 255 if rgb in PORTRAIT_COLORS else 0))
    return bytes(output)


def compose_atlas(directory: Path) -> bytes:
    frames = [
        portrait_frame(directory / f"sf2_audio_{frame:04}.ppm")
        for _, frame in FRAME_SOURCES
    ]
    atlas = bytearray(
        len(frames) * PORTRAIT_WIDTH * PORTRAIT_HEIGHT * CHANNELS_PER_PIXEL
    )
    atlas_width = len(frames) * PORTRAIT_WIDTH
    for frame_index, frame in enumerate(frames):
        for y in range(PORTRAIT_HEIGHT):
            source = y * PORTRAIT_WIDTH * CHANNELS_PER_PIXEL
            destination = (
                y * atlas_width + frame_index * PORTRAIT_WIDTH
            ) * CHANNELS_PER_PIXEL
            atlas[destination : destination + PORTRAIT_WIDTH * CHANNELS_PER_PIXEL] = (
                frame[source : source + PORTRAIT_WIDTH * CHANNELS_PER_PIXEL]
            )
    return bytes(atlas)


def encode(pixels: bytes) -> tuple[list[tuple[int, ...]], list[tuple[int, int]]]:
    palette: list[tuple[int, ...]] = []
    palette_indices: dict[tuple[int, ...], int] = {}
    runs: list[tuple[int, int]] = []
    for offset in range(0, len(pixels), CHANNELS_PER_PIXEL):
        color = tuple(pixels[offset : offset + CHANNELS_PER_PIXEL])
        index = palette_indices.get(color)
        if index is None:
            index = len(palette)
            palette_indices[color] = index
            palette.append(color)
        if runs and runs[-1][1] == index and runs[-1][0] < MAX_RUN_LENGTH:
            length, _ = runs[-1]
            runs[-1] = (length + 1, index)
        else:
            runs.append((1, index))
    return palette, runs


def fnv1a(data: bytes) -> int:
    value = FNV_OFFSET_BASIS
    for byte in data:
        value = ((value ^ byte) * FNV_PRIME) & 0xFFFF_FFFF
    return value


def rust_source(
    source_name: str,
    palette: list[tuple[int, ...]],
    runs: list[tuple[int, int]],
    source_hash: int,
) -> str:
    frame_count = len(FRAME_SOURCES)
    lines = [
        "//! Generated native SF2 mission-radio portrait atlas.",
        "//!",
        f"//! Source: oracle screen sequence `{source_name}`.",
        "//! Regenerate with `tools/sf2/generate_mission_message_portraits.py`.",
        "",
        f"pub const LEFT: i32 = {PORTRAIT_LEFT};",
        f"pub const TOP: i32 = {PORTRAIT_TOP};",
        f"pub const FRAME_WIDTH: usize = {PORTRAIT_WIDTH};",
        f"pub const HEIGHT: usize = {PORTRAIT_HEIGHT};",
        f"pub const FRAME_COUNT: usize = {frame_count};",
        "pub const WIDTH: usize = FRAME_WIDTH * FRAME_COUNT;",
    ]
    lines.extend(
        f"pub const {name}: usize = {index};"
        for index, (name, _) in enumerate(FRAME_SOURCES)
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
    lines.extend([
        "];",
        f"const RUNS: [(u8, u8); {len(runs)}] = [",
    ])
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
            "    fn mission_message_portraits_decode_to_the_oracle_sequence() {",
            "        let rgba = decode_rgba();",
            "        let hash = rgba.into_iter().fold(2_166_136_261, |value, byte| {",
            "            (value ^ u32::from(byte)).wrapping_mul(16_777_619)",
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
    parser.add_argument("oracle_directory", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    atlas = compose_atlas(args.oracle_directory)
    palette, runs = encode(atlas)
    args.output.write_text(
        rust_source(args.oracle_directory.name, palette, runs, fnv1a(atlas)),
        encoding="utf-8",
    )
    print(
        f"generated {args.output}: {len(palette)} colors, {len(runs)} runs, "
        f"FNV-1a {fnv1a(atlas):08X}"
    )


if __name__ == "__main__":
    main()
