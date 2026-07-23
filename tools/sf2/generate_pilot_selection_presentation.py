#!/usr/bin/env python3
"""Generate the native SF2 pilot-selection presentation.

The inputs are retail screenshots captured by the Mesen oracle.  The output
contains ordinary palette-indexed image deltas.  Shipping Rust selects tracks
with typed pilot, menu, control-style, and presentation-phase values.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import struct
from typing import Iterable


FRAME_WIDTH = 256
FRAME_HEIGHT = 224
MESEN_CAPTURE_HEIGHT = 239
MESEN_TOP_BORDER = 8
RETAIL_FRAME_STEP = 4
PILOT_COUNT = 6
PRIMARY_VARIANT_COUNT = 8
REVEAL_FRAME_COUNT = 24
PRIMARY_FRAME_COUNT = 20
WING_FRAME_COUNT = 20
READY_FRAME_COUNT = 43
LAUNCH_FRAME_COUNT = 57
PAIR_TRACK_COUNT = PILOT_COUNT * PILOT_COUNT
TRACK_COUNT = 1 + PRIMARY_VARIANT_COUNT + PAIR_TRACK_COUNT * 3
TOP_PANEL = (8, 0, 244, 116)
BOTTOM_PANEL = (8, 152, 244, 72)
ASSET_MAGIC = b"SFPS2"
FNV_OFFSET_BASIS = 0x811C9DC5
FNV_PRIME = 0x01000193

FOX = 0
FALCO = 1
PEPPY = 2
SLIPPY = 3
MIYU = 4
FAY = 5


@dataclass(frozen=True)
class Image:
    pixels: bytes


@dataclass(frozen=True)
class PairSource:
    directory: Path
    selection_frame: int


def read_ppm(path: Path) -> Image:
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
    return Image(pixels[first_row : first_row + FRAME_HEIGHT * row_bytes])


def frame_path(directory: Path, frame: int) -> Path:
    return directory / f"sf2_post_sortie_{frame:05d}.ppm"


def frame(directory: Path, number: int) -> Image:
    return read_ppm(frame_path(directory, number))


def frame_range(directory: Path, first: int, count: int) -> list[Image]:
    return [frame(directory, first + index * RETAIL_FRAME_STEP) for index in range(count)]


def paste_rectangle(destination: Image, source: Image, rectangle: tuple[int, int, int, int]) -> Image:
    left, top, width, height = rectangle
    pixels = bytearray(destination.pixels)
    row_bytes = FRAME_WIDTH * 3
    copied_bytes = width * 3
    for y in range(top, top + height):
        first = y * row_bytes + left * 3
        pixels[first : first + copied_bytes] = source.pixels[first : first + copied_bytes]
    return Image(bytes(pixels))


def pair_frame(source: PairSource, local_frame: int, launch: bool) -> Image:
    phase_offset = 173 if launch else 1
    return frame(
        source.directory,
        source.selection_frame + phase_offset + local_frame * RETAIL_FRAME_STEP,
    )


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

    def indices(self, image: Image) -> list[int]:
        result = []
        for offset in range(0, len(image.pixels), 3):
            color = tuple(image.pixels[offset : offset + 3])
            index = self.lookup.get(color)
            if index is None:
                index = len(self.palette)
                if index > 65_535:
                    raise SystemExit("presentation needs more than 65536 colors")
                self.lookup[color] = index
                self.palette.append(color)
            result.append(index)
        return result

    def track(self, images: Iterable[Image]) -> None:
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
                if any(index > 255 for index in palette_indices):
                    raise SystemExit("presentation run needs a palette wider than 256 colors")
                self.track_data.extend(palette_indices)
            self.changed_pixels += len(changed_offsets)
            previous = current


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--reveal-dir", type=Path, required=True)
    parser.add_argument("--primary-dir", type=Path, required=True)
    parser.add_argument("--control-dir", type=Path, required=True)
    parser.add_argument("--control-b-dir", type=Path, required=True)
    parser.add_argument("--wing-dir", type=Path, required=True)
    parser.add_argument("--fox-slippy-dir", type=Path, required=True)
    parser.add_argument("--slippy-miyu-dir", type=Path, required=True)
    parser.add_argument("--falco-miyu-dir", type=Path, required=True)
    parser.add_argument("--miyu-falco-dir", type=Path, required=True)
    parser.add_argument("--peppy-fay-dir", type=Path, required=True)
    parser.add_argument("--fay-peppy-dir", type=Path, required=True)
    parser.add_argument("--miyu-fox-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    primary_tracks = [
        frame_range(args.reveal_dir, 3_545, PRIMARY_FRAME_COUNT),
        frame_range(args.primary_dir, 3_601, PRIMARY_FRAME_COUNT),
        frame_range(args.primary_dir, 3_681, PRIMARY_FRAME_COUNT),
        frame_range(args.primary_dir, 3_761, PRIMARY_FRAME_COUNT),
        frame_range(args.primary_dir, 3_841, PRIMARY_FRAME_COUNT),
        frame_range(args.primary_dir, 3_921, PRIMARY_FRAME_COUNT),
        frame_range(args.control_dir, 4_001, PRIMARY_FRAME_COUNT),
        frame_range(args.control_b_dir, 4_085, PRIMARY_FRAME_COUNT),
    ]
    wing_tracks = [
        frame_range(args.miyu_falco_dir, 4_101, WING_FRAME_COUNT),
        frame_range(args.wing_dir, 3_861, WING_FRAME_COUNT),
        frame_range(args.wing_dir, 3_941, WING_FRAME_COUNT),
        frame_range(args.fox_slippy_dir, 3_601, WING_FRAME_COUNT),
        frame_range(args.wing_dir, 3_701, WING_FRAME_COUNT),
        frame_range(args.wing_dir, 3_781, WING_FRAME_COUNT),
    ]

    fox_slippy = PairSource(args.fox_slippy_dir, 3_700)
    slippy_miyu = PairSource(args.slippy_miyu_dir, 3_940)
    falco_miyu = PairSource(args.falco_miyu_dir, 3_860)
    miyu_falco = PairSource(args.miyu_falco_dir, 4_260)
    peppy_fay = PairSource(args.peppy_fay_dir, 4_020)
    fay_peppy = PairSource(args.fay_peppy_dir, 4_420)
    miyu_fox = PairSource(args.miyu_fox_dir, 4_180)

    exact_pairs = {
        (FOX, SLIPPY): fox_slippy,
        (FALCO, MIYU): falco_miyu,
        (PEPPY, FAY): peppy_fay,
        (SLIPPY, MIYU): slippy_miyu,
        (MIYU, FALCO): miyu_falco,
        (MIYU, FOX): miyu_fox,
        (FAY, PEPPY): fay_peppy,
    }
    top_sources = [
        fox_slippy,
        falco_miyu,
        peppy_fay,
        slippy_miyu,
        miyu_falco,
        fay_peppy,
    ]
    bottom_sources = [
        miyu_fox,
        miyu_falco,
        fay_peppy,
        fox_slippy,
        falco_miyu,
        peppy_fay,
    ]

    encoder = Encoder()
    encoder.track(frame_range(args.reveal_dir, 3_449, REVEAL_FRAME_COUNT))
    for track in primary_tracks:
        encoder.track(track)

    stable_primary_panels = [track[-1] for track in primary_tracks[:PILOT_COUNT]]
    for primary in range(PILOT_COUNT):
        for wingmate in range(PILOT_COUNT):
            if primary == FOX and wingmate != FOX:
                encoder.track(wing_tracks[wingmate])
            else:
                encoder.track(
                    paste_rectangle(base, stable_primary_panels[primary], TOP_PANEL)
                    for base in wing_tracks[wingmate]
                )

    for primary in range(PILOT_COUNT):
        for wingmate in range(PILOT_COUNT):
            exact = exact_pairs.get((primary, wingmate))
            images = []
            for local_frame in range(READY_FRAME_COUNT):
                if exact is not None:
                    images.append(pair_frame(exact, local_frame, False))
                    continue
                composed = pair_frame(fox_slippy, local_frame, False)
                composed = paste_rectangle(
                    composed,
                    pair_frame(top_sources[primary], local_frame, False),
                    TOP_PANEL,
                )
                composed = paste_rectangle(
                    composed,
                    pair_frame(bottom_sources[wingmate], local_frame, False),
                    BOTTOM_PANEL,
                )
                images.append(composed)
            encoder.track(images)

    for primary in range(PILOT_COUNT):
        for wingmate in range(PILOT_COUNT):
            exact = exact_pairs.get((primary, wingmate))
            images = []
            for local_frame in range(LAUNCH_FRAME_COUNT):
                if exact is not None:
                    images.append(pair_frame(exact, local_frame, True))
                    continue
                composed = pair_frame(fox_slippy, local_frame, True)
                composed = paste_rectangle(
                    composed,
                    pair_frame(top_sources[primary], local_frame, True),
                    TOP_PANEL,
                )
                composed = paste_rectangle(
                    composed,
                    pair_frame(bottom_sources[wingmate], local_frame, True),
                    BOTTOM_PANEL,
                )
                images.append(composed)
            encoder.track(images)

    output = bytearray(ASSET_MAGIC)
    for value in (
        FRAME_WIDTH,
        FRAME_HEIGHT,
        REVEAL_FRAME_COUNT,
        PRIMARY_FRAME_COUNT,
        WING_FRAME_COUNT,
        READY_FRAME_COUNT,
        LAUNCH_FRAME_COUNT,
        PILOT_COUNT,
        PRIMARY_VARIANT_COUNT,
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
