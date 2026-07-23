#!/usr/bin/env python3
"""Generate the native SF2 campaign-loss Results presentation.

The input screenshots are verification artifacts captured by the Mesen oracle.
The output contains ordinary palette-indexed image deltas; shipping Rust never
consumes source-machine memory or display state.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import struct


FRAME_WIDTH = 256
FRAME_HEIGHT = 224
MESEN_CAPTURE_HEIGHT = 239
MESEN_TOP_BORDER = 8
RETAIL_FRAME_STEP = 4
RESULTS_FIRST_FRAME = 9_170
RESULTS_CAPTURE_LAST_FRAME = 13_050
RESULTS_LOOP_FIRST_FRAME = 12_542
EARLY_CAPTURE_LAST_FRAME = 9_826
MIDDLE_CAPTURE_LAST_FRAME = 11_002
OPENING_FIRST_FRAME = 10_002
OPENING_LAST_FRAME = 10_018
CHOICE_ENTRY_FRAME = 10_022
RETRY_LOOP_FIRST_FRAME = 10_026
RETRY_LOOP_LAST_FRAME = 10_054
TITLE_LOOP_FIRST_FRAME = 10_218
TITLE_LOOP_LAST_FRAME = 10_246
LEAVING_SELECTED_FRAME = 10_502
LEAVING_PLAIN_FRAME = 10_506
ASSET_MAGIC = b"SFRS1"
TRACK_COUNT = 6
FNV_OFFSET_BASIS = 0x811C9DC5
FNV_PRIME = 0x01000193


@dataclass(frozen=True)
class Image:
    width: int
    height: int
    pixels: bytes


def read_ppm(path: Path) -> Image:
    data = path.read_bytes()
    header, pixels = data.split(b"\n255\n", 1)
    fields = header.split()
    if len(fields) != 3 or fields[0] != b"P6":
        raise SystemExit(f"{path} is not a binary P6 PPM")
    width = int(fields[1])
    height = int(fields[2])
    if len(pixels) != width * height * 3:
        raise SystemExit(f"{path} has an incomplete pixel body")
    if width != FRAME_WIDTH or height != MESEN_CAPTURE_HEIGHT:
        raise SystemExit(
            f"expected a {FRAME_WIDTH}x{MESEN_CAPTURE_HEIGHT} capture, "
            f"found {width}x{height} in {path}"
        )
    row_bytes = width * 3
    first_row = MESEN_TOP_BORDER * row_bytes
    pixels = pixels[first_row : first_row + FRAME_HEIGHT * row_bytes]
    return Image(width, FRAME_HEIGHT, pixels)


def frame_path(directory: Path, frame: int) -> Path:
    return directory / f"sf2_post_sortie_{frame:05d}.ppm"


def inclusive_frames(first: int, last: int) -> range:
    return range(first, last + 1, RETAIL_FRAME_STEP)


def palette_and_indices(
    tracks: list[list[Image]],
) -> tuple[list[tuple[int, int, int]], list[list[list[int]]]]:
    palette: list[tuple[int, int, int]] = []
    lookup: dict[tuple[int, int, int], int] = {}

    def indices(image: Image) -> list[int]:
        result = []
        for offset in range(0, len(image.pixels), 3):
            color = tuple(image.pixels[offset : offset + 3])
            index = lookup.get(color)
            if index is None:
                index = len(palette)
                if index > 65_535:
                    raise SystemExit("presentation needs more than 65536 colors")
                lookup[color] = index
                palette.append(color)
            result.append(index)
        return result

    return palette, [[indices(image) for image in track] for track in tracks]


def append_u16(output: bytearray, value: int) -> None:
    output.extend(struct.pack("<H", value))


def encode_track(output: bytearray, frames: list[list[int]], black_index: int) -> int:
    previous = [black_index] * (FRAME_WIDTH * FRAME_HEIGHT)
    changed_pixels = 0
    for frame in frames:
        changes = [
            (offset, palette_index)
            for offset, (before, palette_index) in enumerate(zip(previous, frame))
            if before != palette_index
        ]
        if len(changes) > 65_535:
            raise SystemExit("one frame contains more than 65535 changed pixels")
        append_u16(output, len(changes))
        for offset, palette_index in changes:
            append_u16(output, offset)
            append_u16(output, palette_index)
        previous = frame
        changed_pixels += len(changes)
    return changed_pixels


def fnv1a(data: bytes) -> int:
    value = FNV_OFFSET_BASIS
    for byte in data:
        value = ((value ^ byte) * FNV_PRIME) & 0xFFFF_FFFF
    return value


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--entry-dir", type=Path, required=True)
    parser.add_argument("--idle-dir", type=Path, required=True)
    parser.add_argument("--idle-tail-dir", type=Path, required=True)
    parser.add_argument("--retry-menu-dir", type=Path, required=True)
    parser.add_argument("--title-menu-dir", type=Path, required=True)
    parser.add_argument("--retry-exit-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    reveal = []
    for frame in inclusive_frames(RESULTS_FIRST_FRAME, RESULTS_CAPTURE_LAST_FRAME):
        if frame <= EARLY_CAPTURE_LAST_FRAME:
            directory = args.entry_dir
        elif frame <= MIDDLE_CAPTURE_LAST_FRAME:
            directory = args.idle_dir
        else:
            directory = args.idle_tail_dir
        reveal.append(read_ppm(frame_path(directory, frame)))

    opening = [
        read_ppm(frame_path(args.retry_menu_dir, frame))
        for frame in inclusive_frames(OPENING_FIRST_FRAME, OPENING_LAST_FRAME)
    ]
    retry_choice = [
        read_ppm(frame_path(args.retry_menu_dir, CHOICE_ENTRY_FRAME)),
        *[
            read_ppm(frame_path(args.retry_menu_dir, frame))
            for frame in inclusive_frames(RETRY_LOOP_FIRST_FRAME, RETRY_LOOP_LAST_FRAME)
        ],
    ]
    title_choice = [
        read_ppm(frame_path(args.retry_menu_dir, CHOICE_ENTRY_FRAME)),
        *[
            read_ppm(frame_path(args.title_menu_dir, frame))
            for frame in inclusive_frames(TITLE_LOOP_FIRST_FRAME, TITLE_LOOP_LAST_FRAME)
        ],
    ]
    retry_leaving = [
        read_ppm(frame_path(args.retry_exit_dir, LEAVING_SELECTED_FRAME)),
        read_ppm(frame_path(args.retry_exit_dir, LEAVING_PLAIN_FRAME)),
    ]
    title_leaving = [
        read_ppm(frame_path(args.title_menu_dir, LEAVING_SELECTED_FRAME)),
        read_ppm(frame_path(args.title_menu_dir, LEAVING_PLAIN_FRAME)),
    ]
    image_tracks = [
        reveal,
        opening,
        retry_choice,
        title_choice,
        retry_leaving,
        title_leaving,
    ]
    palette, indexed_tracks = palette_and_indices(image_tracks)
    black_index = palette.index((0, 0, 0))

    output = bytearray(ASSET_MAGIC)
    for value in (
        FRAME_WIDTH,
        FRAME_HEIGHT,
        len(reveal),
        (RESULTS_LOOP_FIRST_FRAME - RESULTS_FIRST_FRAME) // RETAIL_FRAME_STEP,
        len(opening),
        len(retry_choice),
        len(retry_leaving),
        TRACK_COUNT,
        len(palette),
        black_index,
    ):
        append_u16(output, value)
    for color in palette:
        output.extend(color)

    change_counts = [
        encode_track(output, track, black_index) for track in indexed_tracks
    ]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(output)
    print(
        f"wrote {args.output}: {len(output)} bytes, {len(palette)} colors, "
        f"changes={change_counts}, fnv1a={fnv1a(output):08X}"
    )


if __name__ == "__main__":
    main()
