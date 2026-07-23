#!/usr/bin/env python3
"""Generate an object-free strategic-map backdrop from a retail capture.

The input capture still contains the retail object layer.  This oracle-only
tool removes each non-transparent object pixel using the already certified
opening backdrop.  Shipping Rust receives only an ordinary RGBA texture and
renders typed strategic actors separately.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path

from decode_oam import sprite_pixel, sprites
from generate_backdrop import (
    EXPECTED_HEIGHT,
    EXPECTED_WIDTH,
    MESEN_CAPTURE_HEIGHT,
    MESEN_TOP_BORDER,
    encode,
    fnv1a,
    read_ppm,
    rust_source,
)


CHANNELS_PER_PIXEL = 4
RGB_CHANNELS_PER_PIXEL = 3
OBJECT_VERTICAL_ADJUSTMENT = -1


def generated_backdrop_rgb(path: Path) -> bytes:
    source = path.read_text(encoding="utf-8")
    palette_source = (
        source.split("const PALETTE:", 1)[1].split("= [", 1)[1].split("];", 1)[0]
    )
    run_source = (
        source.split("const RUNS:", 1)[1].split("= [", 1)[1].split("];", 1)[0]
    )
    palette = [
        tuple(map(int, values))
        for values in re.findall(
            r"\[(\d+),\s*(\d+),\s*(\d+),\s*(\d+)\]", palette_source
        )
    ]
    runs = [
        tuple(map(int, values))
        for values in re.findall(r"\((\d+),\s*(\d+)\)", run_source)
    ]
    rgba = bytearray()
    for length, palette_index in runs:
        rgba.extend(bytes(palette[palette_index]) * length)
    expected = EXPECTED_WIDTH * EXPECTED_HEIGHT * CHANNELS_PER_PIXEL
    if len(rgba) != expected:
        raise SystemExit(f"{path} decoded to {len(rgba)} bytes, expected {expected}")
    return bytes(
        channel
        for offset in range(0, len(rgba), CHANNELS_PER_PIXEL)
        for channel in rgba[offset : offset + RGB_CHANNELS_PER_PIXEL]
    )


def crop_capture(path: Path) -> bytes:
    width, height, pixels = read_ppm(path)
    if width != EXPECTED_WIDTH or height != MESEN_CAPTURE_HEIGHT:
        raise SystemExit(
            f"expected a {EXPECTED_WIDTH}x{MESEN_CAPTURE_HEIGHT} capture, "
            f"found {width}x{height}"
        )
    row_bytes = width * RGB_CHANNELS_PER_PIXEL
    top = MESEN_TOP_BORDER * row_bytes
    return pixels[top : top + EXPECTED_HEIGHT * row_bytes]


def remove_object_layer(
    retail: bytes,
    opening: bytes,
    vram: bytes,
    oam: bytes,
    character_base: int,
) -> bytes:
    result = bytearray(retail)
    for sprite in sprites(oam):
        for local_y in range(sprite.size):
            raw_y = (sprite.y + local_y) & 255
            screen_y = raw_y + OBJECT_VERTICAL_ADJUSTMENT
            if not 0 <= screen_y < EXPECTED_HEIGHT:
                continue
            for local_x in range(sprite.size):
                screen_x = sprite.x + local_x
                if not 0 <= screen_x < EXPECTED_WIDTH:
                    continue
                if sprite_pixel(vram, character_base, sprite, local_x, local_y) == 0:
                    continue
                offset = (
                    screen_y * EXPECTED_WIDTH + screen_x
                ) * RGB_CHANNELS_PER_PIXEL
                result[offset : offset + RGB_CHANNELS_PER_PIXEL] = opening[
                    offset : offset + RGB_CHANNELS_PER_PIXEL
                ]
    return bytes(result)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("capture", type=Path)
    parser.add_argument("vram", type=Path)
    parser.add_argument("oam", type=Path)
    parser.add_argument("opening_backdrop", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--characters", type=lambda value: int(value, 0), required=True)
    parser.add_argument(
        "--variant-name",
        default="escalated",
        help="semantic name used in the generated module documentation",
    )
    parser.add_argument(
        "--test-name",
        default="escalated_map_decodes_to_the_certified_frame",
        help="semantic generated Rust test name",
    )
    args = parser.parse_args()

    vram = args.vram.read_bytes()
    oam = args.oam.read_bytes()
    if len(vram) != 65_536 or len(oam) != 544:
        raise SystemExit("expected complete 64 KiB VRAM and 544-byte OAM snapshots")
    retail = crop_capture(args.capture)
    opening = generated_backdrop_rgb(args.opening_backdrop)
    pixels = remove_object_layer(retail, opening, vram, oam, args.characters)
    palette, runs = encode(pixels)
    rgba = bytearray()
    for length, palette_index in runs:
        rgba.extend(bytes((*palette[palette_index], 255)) * length)
    args.output.write_text(
        rust_source(
            args.capture.name,
            EXPECTED_WIDTH,
            EXPECTED_HEIGHT,
            palette,
            runs,
            fnv1a(bytes(rgba)),
            f"Generated native SF2 {args.variant_name} strategic-map backdrop.",
            "retail capture with its object layer removed by oracle tooling",
            "tools/sf2/generate_strategic_map_variant.py",
            args.test_name,
        ),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
