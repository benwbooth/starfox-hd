#!/usr/bin/env python3
"""Extract the opaque SF2 mission-message panel from an oracle PPM frame."""

from __future__ import annotations

import argparse
from pathlib import Path


PANEL_LEFT = 84
PANEL_TOP = 173
PANEL_WIDTH = 111
PANEL_HEIGHT = 40
CHANNELS_PER_PIXEL = 4
MAX_RUN_LENGTH = 255
FNV_OFFSET_BASIS = 2_166_136_261
FNV_PRIME = 16_777_619


def read_ppm(path: Path) -> tuple[int, int, bytes]:
    contents = path.read_bytes()
    parts = contents.split(maxsplit=4)
    if len(parts) != 5 or parts[0] != b"P6" or parts[3] != b"255":
        raise SystemExit("expected a binary RGB PPM with maximum value 255")
    width = int(parts[1])
    height = int(parts[2])
    pixels = parts[4]
    if len(pixels) != width * height * 3:
        raise SystemExit("PPM pixel payload does not match its dimensions")
    return width, height, pixels


def crop_rgba(width: int, height: int, pixels: bytes) -> bytes:
    if width < PANEL_LEFT + PANEL_WIDTH or height < PANEL_TOP + PANEL_HEIGHT:
        raise SystemExit("oracle frame does not contain the mission-message panel")
    output = bytearray()
    for y in range(PANEL_TOP, PANEL_TOP + PANEL_HEIGHT):
        row = (y * width + PANEL_LEFT) * 3
        for x in range(PANEL_WIDTH):
            offset = row + x * 3
            output.extend((*pixels[offset : offset + 3], 255))
    return bytes(output)


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
    lines = [
        "//! Generated native SF2 mission-message panel.",
        "//!",
        f"//! Source: oracle screen frame `{source_name}`.",
        "//! Regenerate with `tools/sf2/generate_mission_message_panel.py`.",
        "",
        f"pub const LEFT: i32 = {PANEL_LEFT};",
        f"pub const TOP: i32 = {PANEL_TOP};",
        f"pub const WIDTH: usize = {PANEL_WIDTH};",
        f"pub const HEIGHT: usize = {PANEL_HEIGHT};",
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
            "    fn mission_message_panel_decodes_to_the_oracle_crop() {",
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
    parser.add_argument("oracle_frame", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    width, height, pixels = read_ppm(args.oracle_frame)
    panel = crop_rgba(width, height, pixels)
    palette, runs = encode(panel)
    args.output.write_text(
        rust_source(args.oracle_frame.name, palette, runs, fnv1a(panel)),
        encoding="utf-8",
    )
    print(
        f"generated {args.output}: {len(palette)} colors, {len(runs)} runs, "
        f"FNV-1a {fnv1a(panel):08X}"
    )


if __name__ == "__main__":
    main()
