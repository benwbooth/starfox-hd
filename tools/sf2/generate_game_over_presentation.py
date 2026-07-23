#!/usr/bin/env python3
"""Generate the native SF2 Game Over presentation from retail screenshots.

The input screenshots are verification artifacts captured by the Mesen oracle.
The output contains ordinary palette-indexed image deltas and portrait crops;
shipping Rust never consumes source-machine memory or PPU state.
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
TAUNT_FIRST_FRAME = 8_687
TAUNT_LAST_FRAME = 9_083
PROMPT_FIRST_FRAME = 9_087
PROMPT_REVEAL_LAST_FRAME = 9_159
PROMPT_LOOP_FIRST_FRAME = 9_163
PROMPT_LOOP_LAST_FRAME = 9_735
PORTRAIT_LEFT = 56
PORTRAIT_TOP = 160
PORTRAIT_WIDTH = 40
PORTRAIT_HEIGHT = 48
PILOT_NAMES = ("fox", "falco", "peppy", "slippy", "miyu", "fay")
PORTRAIT_VARIANT_COUNT = len(PILOT_NAMES) + 1
EMPTY_PORTRAIT_SOURCE_FRAME = 9_071
ASSET_MAGIC = b"SFGO1"
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


def crop(image: Image, left: int, top: int, width: int, height: int) -> Image:
    rows = []
    for y in range(top, top + height):
        first = (y * image.width + left) * 3
        rows.append(image.pixels[first : first + width * 3])
    return Image(width, height, b"".join(rows))


def frame_path(directory: Path, frame: int) -> Path:
    return directory / f"sf2_post_sortie_{frame:05d}.ppm"


def inclusive_frames(first: int, last: int) -> range:
    return range(first, last + 1, RETAIL_FRAME_STEP)


def prompt_frames() -> list[int]:
    return [
        *inclusive_frames(PROMPT_FIRST_FRAME, PROMPT_REVEAL_LAST_FRAME),
        *inclusive_frames(PROMPT_LOOP_FIRST_FRAME, PROMPT_LOOP_LAST_FRAME),
    ]


def parse_pilot_capture(value: str) -> tuple[str, Path]:
    name, separator, path = value.partition("=")
    if not separator or name not in PILOT_NAMES:
        raise argparse.ArgumentTypeError(
            "pilot captures must use name=/path/file.ppm for "
            + ", ".join(PILOT_NAMES)
        )
    return name, Path(path)


def palette_and_indices(
    tracks: list[list[Image]], portraits: list[Image]
) -> tuple[list[tuple[int, int, int]], list[list[list[int]]], list[list[int]]]:
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

    indexed_tracks = [[indices(image) for image in track] for track in tracks]
    indexed_portraits = [indices(image) for image in portraits]
    return palette, indexed_tracks, indexed_portraits


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
            raise SystemExit("one frame contains too many changed pixels")
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
    parser.add_argument("--taunt-dir", type=Path, required=True)
    parser.add_argument("--prompt-yes-dir", type=Path, required=True)
    parser.add_argument("--prompt-no-dir", type=Path, required=True)
    parser.add_argument(
        "--pilot",
        type=parse_pilot_capture,
        action="append",
        required=True,
        help="repeat as name=/path/to/prompt.ppm for all six pilots",
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    pilot_paths = dict(args.pilot)
    missing = [name for name in PILOT_NAMES if name not in pilot_paths]
    if missing:
        raise SystemExit("missing pilot captures: " + ", ".join(missing))

    taunt = [
        read_ppm(frame_path(args.taunt_dir, frame))
        for frame in inclusive_frames(TAUNT_FIRST_FRAME, TAUNT_LAST_FRAME)
    ]
    requested_prompt_frames = prompt_frames()
    prompt_yes = [
        read_ppm(frame_path(args.prompt_yes_dir, frame))
        for frame in requested_prompt_frames
    ]
    prompt_no = [
        read_ppm(frame_path(args.prompt_no_dir, frame))
        for frame in requested_prompt_frames
    ]
    portraits = [
        crop(
            read_ppm(pilot_paths[name]),
            PORTRAIT_LEFT,
            PORTRAIT_TOP,
            PORTRAIT_WIDTH,
            PORTRAIT_HEIGHT,
        )
        for name in PILOT_NAMES
    ]
    portraits.append(
        crop(
            read_ppm(frame_path(args.taunt_dir, EMPTY_PORTRAIT_SOURCE_FRAME)),
            PORTRAIT_LEFT,
            PORTRAIT_TOP,
            PORTRAIT_WIDTH,
            PORTRAIT_HEIGHT,
        )
    )

    palette, tracks, indexed_portraits = palette_and_indices(
        [taunt, prompt_yes, prompt_no], portraits
    )
    black_index = palette.index((0, 0, 0))

    output = bytearray(ASSET_MAGIC)
    for value in (
        FRAME_WIDTH,
        FRAME_HEIGHT,
        len(taunt),
        len(prompt_yes),
        len(inclusive_frames(PROMPT_FIRST_FRAME, PROMPT_REVEAL_LAST_FRAME)),
        PORTRAIT_VARIANT_COUNT,
        PORTRAIT_LEFT,
        PORTRAIT_TOP,
        PORTRAIT_WIDTH,
        PORTRAIT_HEIGHT,
        len(palette),
        black_index,
    ):
        append_u16(output, value)
    for red, green, blue in palette:
        output.extend((red, green, blue))

    changed_pixels = sum(encode_track(output, track, black_index) for track in tracks)
    for portrait in indexed_portraits:
        for palette_index in portrait:
            append_u16(output, palette_index)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(output)
    print(
        f"generated {args.output}: {len(output)} bytes, {len(palette)} colors, "
        f"{len(taunt)} taunt frames, {len(prompt_yes)} prompt frames per choice, "
        f"{changed_pixels} changed pixels, FNV-1a {fnv1a(output):08X}"
    )


if __name__ == "__main__":
    main()
