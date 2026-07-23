#!/usr/bin/env python3
"""Generate the native SF2 title, menu, and records presentation.

Inputs are retail screenshots captured by ``mesen_title_branch_capture.lua``.
The output stores ordinary palette-indexed image deltas.  Shipping Rust picks
tracks using typed title-page, menu-item, difficulty, and audio-output values.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import struct
from typing import Iterable


FRAME_WIDTH = 256
FRAME_HEIGHT = 224
MESEN_CAPTURE_HEIGHT = 239
MESEN_TOP_BORDER = 8
RETAIL_FRAME_STEP = 4
TITLE_FRAME_COUNT = 150
RECORDS_FRAME_COUNT = 200
TRACK_COUNT = 8
MAIN_FIRST_FRAME = 4
FIRST_SELECTION_SETTLED_FRAME = 24
SECOND_SELECTION_SETTLED_FRAME = 204
THIRD_SELECTION_SETTLED_FRAME = 384
ASSET_MAGIC = b"SFTL2"
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


def frame_path(directory: Path, scenario: str, frame: int) -> Path:
    return directory / f"sf2_title_{scenario}_{frame:04d}.ppm"


def frame_range(
    directory: Path,
    scenario: str,
    first: int,
    count: int,
) -> list[bytes]:
    return [
        read_ppm(frame_path(directory, scenario, first + index * RETAIL_FRAME_STEP))
        for index in range(count)
    ]


def append_u16(output: bytearray, value: int) -> None:
    output.extend(struct.pack("<H", value))


def fnv1a(data: bytes) -> int:
    value = FNV_OFFSET_BASIS
    for byte in data:
        value = ((value ^ byte) * FNV_PRIME) & 0xFFFF_FFFF
    return value


class Encoder:
    def __init__(self) -> None:
        self.palette = [(0, 0, 0)]
        self.lookup = {(0, 0, 0): 0}
        self.track_data = bytearray()
        self.changed_pixels = 0

    def indices(self, image: bytes) -> list[int]:
        result = []
        for offset in range(0, len(image), 3):
            color = tuple(image[offset : offset + 3])
            index = self.lookup.get(color)
            if index is None:
                index = len(self.palette)
                if index > 65_535:
                    raise SystemExit("title presentation needs more than 65536 colors")
                self.lookup[color] = index
                self.palette.append(color)
            result.append(index)
        return result

    def track(self, images: Iterable[bytes]) -> None:
        previous = [0] * (FRAME_WIDTH * FRAME_HEIGHT)
        for image in images:
            current = self.indices(image)
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
            append_u16(self.track_data, len(runs))
            for offset, palette_indices in runs:
                append_u16(self.track_data, offset)
                append_u16(self.track_data, len(palette_indices))
                for palette_index in palette_indices:
                    append_u16(self.track_data, palette_index)
            self.changed_pixels += len(changed_offsets)
            previous = current


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--main-dir", type=Path, required=True)
    parser.add_argument("--record-dir", type=Path, required=True)
    parser.add_argument("--stereo-dir", type=Path, required=True)
    parser.add_argument("--sound-dir", type=Path, required=True)
    parser.add_argument("--normal-dir", type=Path, required=True)
    parser.add_argument("--hard-dir", type=Path, required=True)
    parser.add_argument("--expert-dir", type=Path, required=True)
    parser.add_argument("--records-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    tracks = [
        frame_range(args.main_dir, "main", MAIN_FIRST_FRAME, TITLE_FRAME_COUNT),
        frame_range(
            args.record_dir,
            "record",
            FIRST_SELECTION_SETTLED_FRAME,
            TITLE_FRAME_COUNT,
        ),
        frame_range(
            args.stereo_dir,
            "stereo",
            SECOND_SELECTION_SETTLED_FRAME,
            TITLE_FRAME_COUNT,
        ),
        frame_range(
            args.sound_dir,
            "sound",
            THIRD_SELECTION_SETTLED_FRAME,
            TITLE_FRAME_COUNT,
        ),
        frame_range(
            args.normal_dir,
            "difficulty_normal",
            FIRST_SELECTION_SETTLED_FRAME,
            TITLE_FRAME_COUNT,
        ),
        frame_range(
            args.hard_dir,
            "difficulty_hard",
            SECOND_SELECTION_SETTLED_FRAME,
            TITLE_FRAME_COUNT,
        ),
        frame_range(
            args.expert_dir,
            "difficulty",
            THIRD_SELECTION_SETTLED_FRAME,
            TITLE_FRAME_COUNT,
        ),
        frame_range(
            args.records_dir,
            "records",
            SECOND_SELECTION_SETTLED_FRAME,
            RECORDS_FRAME_COUNT,
        ),
    ]

    encoder = Encoder()
    for track in tracks:
        encoder.track(track)

    output = bytearray(ASSET_MAGIC)
    for value in (
        FRAME_WIDTH,
        FRAME_HEIGHT,
        TITLE_FRAME_COUNT,
        RECORDS_FRAME_COUNT,
        TRACK_COUNT,
        len(encoder.palette),
        0,
    ):
        append_u16(output, value)
    for color in encoder.palette:
        output.extend(color)
    output.extend(encoder.track_data)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(output)
    print(
        f"wrote {args.output}: {len(output)} bytes, {len(encoder.palette)} colors, "
        f"{TRACK_COUNT} tracks, {encoder.changed_pixels} changes, "
        f"fnv1a={fnv1a(output):08X}"
    )


if __name__ == "__main__":
    main()
