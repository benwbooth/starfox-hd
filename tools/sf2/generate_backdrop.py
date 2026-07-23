#!/usr/bin/env python3
"""Generate a compact native Rust texture from an isolated SF2 backdrop."""

from __future__ import annotations

import argparse
from pathlib import Path


EXPECTED_WIDTH = 256
EXPECTED_HEIGHT = 224
MESEN_CAPTURE_HEIGHT = 239
MESEN_TOP_BORDER = 8
MAX_RUN_LENGTH = 255
FNV_OFFSET_BASIS = 0x811C9DC5
FNV_PRIME = 0x01000193


def read_ppm(path: Path) -> tuple[int, int, bytes]:
    data = path.read_bytes()
    header, pixels = data.split(b"\n255\n", 1)
    fields = header.split()
    if len(fields) != 3 or fields[0] != b"P6":
        raise SystemExit(f"{path} is not a binary P6 PPM")
    width = int(fields[1])
    height = int(fields[2])
    if len(pixels) != width * height * 3:
        raise SystemExit(f"{path} has an incomplete pixel body")
    return width, height, pixels


def encode(pixels: bytes) -> tuple[list[tuple[int, int, int]], list[tuple[int, int]]]:
    palette: list[tuple[int, int, int]] = []
    palette_indices: dict[tuple[int, int, int], int] = {}
    runs: list[tuple[int, int]] = []
    for offset in range(0, len(pixels), 3):
        color = tuple(pixels[offset : offset + 3])
        index = palette_indices.get(color)
        if index is None:
            index = len(palette)
            if index > 255:
                raise SystemExit("backdrop needs more than 256 palette entries")
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
        value = ((value ^ byte) * FNV_PRIME) & 0xFFFFFFFF
    return value


def rust_source(
    source_name: str,
    width: int,
    height: int,
    palette: list[tuple[int, int, int]],
    runs: list[tuple[int, int]],
    source_hash: int,
    rust_doc: str,
    source_description: str,
    regenerate_command: str,
    test_name: str,
) -> str:
    lines = [
        f"//! {rust_doc}",
        "//!",
        f"//! Source: {source_description} `{source_name}`.",
        f"//! Regenerate with `{regenerate_command}`.",
        "",
        f"pub const WIDTH: usize = {width};",
        f"pub const HEIGHT: usize = {height};",
        "const CHANNELS_PER_PIXEL: usize = 4;",
        "#[cfg(test)]",
        "const FNV_OFFSET_BASIS: u32 = 0x811C9DC5;",
        "#[cfg(test)]",
        "const FNV_PRIME: u32 = 0x01000193;",
        "#[cfg(test)]",
        f"const SOURCE_RGBA_FNV1A: u32 = 0x{source_hash:08X};",
        f"const PALETTE: [[u8; CHANNELS_PER_PIXEL]; {len(palette)}] = [",
    ]
    lines.extend(f"    [{red}, {green}, {blue}, 255]," for red, green, blue in palette)
    lines.extend(
        [
            "];",
            f"const RUNS: [(u8, u8); {len(runs)}] = [",
        ]
    )
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
            f"    fn {test_name}() {{",
            "        let rgba = decode_rgba();",
            "        assert_eq!(rgba.len(), WIDTH * HEIGHT * CHANNELS_PER_PIXEL);",
            "        let hash = rgba.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {",
            "            (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)",
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
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument(
        "--rust-doc",
        default="Generated native SF2 opening-sortie backdrop.",
    )
    parser.add_argument(
        "--source-description",
        default="oracle-isolated retail layer",
    )
    parser.add_argument(
        "--regenerate-command",
        default="tools/sf2/generate_backdrop.py",
    )
    parser.add_argument(
        "--test-name",
        default="backdrop_decodes_to_the_native_frame_size",
    )
    args = parser.parse_args()

    width, height, pixels = read_ppm(args.input)
    if width != EXPECTED_WIDTH:
        raise SystemExit(f"expected a 256-pixel-wide input, found {width}x{height}")
    if height == MESEN_CAPTURE_HEIGHT:
        row_bytes = width * 3
        first_row = MESEN_TOP_BORDER * row_bytes
        pixels = pixels[first_row : first_row + EXPECTED_HEIGHT * row_bytes]
        height = EXPECTED_HEIGHT
    elif height != EXPECTED_HEIGHT:
        raise SystemExit(f"expected a 256x224 or 256x239 input, found {width}x{height}")
    palette, runs = encode(pixels)
    rgba = bytearray()
    for length, palette_index in runs:
        rgba.extend(bytes((*palette[palette_index], 255)) * length)
    source_hash = fnv1a(bytes(rgba))
    args.output.write_text(
        rust_source(
            args.input.name,
            width,
            height,
            palette,
            runs,
            source_hash,
            args.rust_doc,
            args.source_description,
            args.regenerate_command,
            args.test_name,
        ),
        encoding="utf-8",
    )
    print(
        f"generated {args.output}: {len(palette)} colors, "
        f"{len(runs)} runs, {width}x{height} pixels, FNV-1a {source_hash:08X}"
    )


if __name__ == "__main__":
    main()
