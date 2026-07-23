#!/usr/bin/env python3
"""Generate the native SF2 staff roll and end-screen presentation.

The primary input is the four-retail-frame cadence captured from the exact
Astropolis handoff by ``mesen_post_sortie_trace.lua``. The response input is
the independently captured eight-frame fade after accepting Start. The output
contains ordinary palette-indexed image deltas selected by typed native ending
state; it contains no source-machine state.
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
FIRST_RETAIL_FRAME = 214_119
FRAME_COUNT = 3_001
START_RESPONSE_FIRST_RETAIL_FRAME = 222_103
START_RESPONSE_FRAME_COUNT = 8
ASSET_MAGIC = b"SFEN2"
FNV_OFFSET_BASIS = 2_166_136_261
FNV_PRIME = 16_777_619


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
        value = ((value ^ byte) * FNV_PRIME) & 4_294_967_295
    return value


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--capture-dir", type=Path, required=True)
    parser.add_argument("--start-response-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    palette = [(0, 0, 0)]
    palette_lookup = {(0, 0, 0): 0}
    track_data = bytearray()
    changed_pixels = 0

    def append_track(
        directory: Path,
        first_frame: int,
        frame_count: int,
    ) -> None:
        nonlocal changed_pixels
        previous = [0] * (FRAME_WIDTH * FRAME_HEIGHT)
        for index in range(frame_count):
            frame = first_frame + index * RETAIL_FRAME_STEP
            image = read_ppm(directory / f"sf2_post_sortie_{frame:06d}.ppm")
            current = []
            for offset in range(0, len(image), 3):
                color = tuple(image[offset : offset + 3])
                palette_index = palette_lookup.get(color)
                if palette_index is None:
                    palette_index = len(palette)
                    if palette_index > 65_535:
                        raise SystemExit(
                            "ending presentation needs more than 65536 colors"
                        )
                    palette_lookup[color] = palette_index
                    palette.append(color)
                current.append(palette_index)

            changed_offsets = [
                offset
                for offset, (before, palette_index) in enumerate(
                    zip(previous, current)
                )
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

    append_track(args.capture_dir, FIRST_RETAIL_FRAME, FRAME_COUNT)
    append_track(
        args.start_response_dir,
        START_RESPONSE_FIRST_RETAIL_FRAME,
        START_RESPONSE_FRAME_COUNT,
    )

    output = bytearray(ASSET_MAGIC)
    for value in (
        FRAME_WIDTH,
        FRAME_HEIGHT,
        FRAME_COUNT,
        START_RESPONSE_FRAME_COUNT,
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
        f"{FRAME_COUNT + START_RESPONSE_FRAME_COUNT} frames, "
        f"{changed_pixels} changes, fnv1a={fnv1a(output):08X}"
    )


if __name__ == "__main__":
    main()
