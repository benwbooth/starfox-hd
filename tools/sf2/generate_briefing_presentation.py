#!/usr/bin/env python3
"""Generate the native SF2 Andross briefing presentation.

The input is a retail screenshot track captured by
``mesen_title_branch_capture.lua``.  The output contains ordinary
palette-indexed image deltas for the typed native briefing phase.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import struct


FRAME_WIDTH = 256
FRAME_HEIGHT = 224
MESEN_CAPTURE_HEIGHT = 239
MESEN_TOP_BORDER = 8
RETAIL_FRAME_STEP = 4
FIRST_BRIEFING_FRAME = 232
BRIEFING_FRAME_COUNT = 170
ASSET_MAGIC = b"SFBR2"
FNV_OFFSET_BASIS = 0x811C9DC5
FNV_PRIME = 0x01000193


def read_ppm(path: Path) -> bytes:
    data = path.read_bytes()
    header, pixels = data.split(b"\n255\n", 1)
    fields = header.split()
    if len(fields) != 3 or fields[0] != b"P6":
        raise SystemExit(f"{path} is not a binary P6 PPM")
    width = int(fields[1])
    height = int(fields[2])
    if width != FRAME_WIDTH or height != MESEN_CAPTURE_HEIGHT:
        raise SystemExit(
            f"expected a {FRAME_WIDTH}x{MESEN_CAPTURE_HEIGHT} capture, "
            f"found {width}x{height} in {path}"
        )
    if len(pixels) != width * height * 3:
        raise SystemExit(f"{path} has an incomplete pixel body")
    row_bytes = width * 3
    first_row = MESEN_TOP_BORDER * row_bytes
    return pixels[first_row : first_row + FRAME_HEIGHT * row_bytes]


def append_u16(output: bytearray, value: int) -> None:
    output.extend(struct.pack("<H", value))


def fnv1a(data: bytes) -> int:
    value = FNV_OFFSET_BASIS
    for byte in data:
        value = ((value ^ byte) * FNV_PRIME) & 0xFFFF_FFFF
    return value


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--capture-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    palette = [(0, 0, 0)]
    palette_lookup = {(0, 0, 0): 0}
    previous = [0] * (FRAME_WIDTH * FRAME_HEIGHT)
    track_data = bytearray()
    changed_pixels = 0

    for index in range(BRIEFING_FRAME_COUNT):
        frame = FIRST_BRIEFING_FRAME + index * RETAIL_FRAME_STEP
        image = read_ppm(args.capture_dir / f"sf2_title_campaign_{frame:04d}.ppm")
        current = []
        for offset in range(0, len(image), 3):
            color = tuple(image[offset : offset + 3])
            palette_index = palette_lookup.get(color)
            if palette_index is None:
                palette_index = len(palette)
                if palette_index > 65_535:
                    raise SystemExit("briefing presentation needs more than 65536 colors")
                palette_lookup[color] = palette_index
                palette.append(color)
            current.append(palette_index)

        changed_offsets = [
            offset
            for offset, (before, palette_index) in enumerate(zip(previous, current))
            if before != palette_index
        ]
        runs: list[tuple[int, list[int]]] = []
        for offset in changed_offsets:
            if runs and offset == runs[-1][0] + len(runs[-1][1]):
                runs[-1][1].append(current[offset])
            else:
                runs.append((offset, [current[offset]]))
        append_u16(track_data, len(runs))
        for offset, palette_indices in runs:
            append_u16(track_data, offset)
            append_u16(track_data, len(palette_indices))
            for palette_index in palette_indices:
                append_u16(track_data, palette_index)
        changed_pixels += len(changed_offsets)
        previous = current

    output = bytearray(ASSET_MAGIC)
    for value in (
        FRAME_WIDTH,
        FRAME_HEIGHT,
        BRIEFING_FRAME_COUNT,
        len(palette),
        0,
    ):
        append_u16(output, value)
    for color in palette:
        output.extend(color)
    output.extend(track_data)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(output)
    print(
        f"wrote {args.output}: {len(output)} bytes, {len(palette)} colors, "
        f"{BRIEFING_FRAME_COUNT} frames, {changed_pixels} changes, "
        f"fnv1a={fnv1a(output):08X}"
    )


if __name__ == "__main__":
    main()
