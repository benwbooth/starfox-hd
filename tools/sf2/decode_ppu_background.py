#!/usr/bin/env python3
"""Decode a retail SF2 tiled background snapshot into a portable PPM.

This is an oracle-side extraction tool. It accepts the raw VRAM and CGRAM
captures emitted by the Mesen traces and reconstructs one ordinary SNES
background layer. Shipping Rust consumes generated image data; it does not
reproduce this address-based PPU view.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path


FRAME_WIDTH = 256
FRAME_HEIGHT = 224
VRAM_BYTE_MASK = 65_535
TILE_INDEX_MASK = 1_023
TILE_FLIP_HORIZONTAL = 16_384
TILE_FLIP_VERTICAL = 32_768
TILE_PALETTE_SHIFT = 10
TILE_PALETTE_MASK = 7
COLORS_PER_4BPP_PALETTE = 16
BYTES_PER_4BPP_TILE = 32


@dataclass(frozen=True)
class Layer:
    tilemap_word_address: int
    character_word_address: int
    horizontal_scroll: int
    vertical_scroll: int
    tiles_wide: int
    tiles_high: int
    large_tiles: bool


@dataclass(frozen=True)
class OffsetMap:
    tilemap_word_address: int
    horizontal_scroll: int
    vertical_scroll: int
    tiles_wide: int
    tiles_high: int


def vram_word(vram: bytes, word_address: int) -> int:
    byte_address = word_address * 2 & VRAM_BYTE_MASK
    return vram[byte_address] | (vram[(byte_address + 1) & VRAM_BYTE_MASK] << 8)


def tilemap_word(vram: bytes, layer: Layer, tile_x: int, tile_y: int) -> int:
    x = tile_x % layer.tiles_wide
    y = tile_y % layer.tiles_high
    screen_x = x // 32
    screen_y = y // 32
    screens_per_row = 2 if layer.tiles_wide == 64 else 1
    screen = screen_y * screens_per_row + screen_x
    entry_word = screen * 1_024 + (y % 32) * 32 + (x % 32)
    return vram_word(vram, layer.tilemap_word_address + entry_word)


def tile_pixel(vram: bytes, character_base: int, tile: int, x: int, y: int) -> int:
    start = (character_base * 2 + tile * BYTES_PER_4BPP_TILE) & VRAM_BYTE_MASK
    bit = 7 - (x & 7)
    value = 0
    for plane in range(4):
        pair = plane // 2
        plane_byte = plane & 1
        address = (start + pair * 16 + (y & 7) * 2 + plane_byte) & VRAM_BYTE_MASK
        value |= ((vram[address] >> bit) & 1) << plane
    return value


def color(cgram: bytes, index: int) -> tuple[int, int, int]:
    offset = (index & 255) * 2
    raw = cgram[offset] | (cgram[offset + 1] << 8)

    def expand(component: int) -> int:
        five_bit = component & 31
        return (five_bit << 3) | (five_bit >> 2)

    return expand(raw), expand(raw >> 5), expand(raw >> 10)


def decode(vram: bytes, cgram: bytes, layer: Layer) -> bytes:
    pixels = bytearray(FRAME_WIDTH * FRAME_HEIGHT * 3)
    tile_size = 16 if layer.large_tiles else 8
    for y in range(FRAME_HEIGHT):
        for x in range(FRAME_WIDTH):
            source_x = (x + layer.horizontal_scroll) & 1_023
            source_y = (y + layer.vertical_scroll) & 1_023
            entry = tilemap_word(vram, layer, source_x // tile_size, source_y // tile_size)
            pixel_x = source_x % tile_size
            pixel_y = source_y % tile_size
            if entry & TILE_FLIP_HORIZONTAL:
                pixel_x = tile_size - 1 - pixel_x
            if entry & TILE_FLIP_VERTICAL:
                pixel_y = tile_size - 1 - pixel_y
            tile = entry & TILE_INDEX_MASK
            if layer.large_tiles:
                tile += pixel_x // 8 + (pixel_y // 8) * 16
            palette_index = tile_pixel(
                vram,
                layer.character_word_address,
                tile,
                pixel_x,
                pixel_y,
            )
            palette = (entry >> TILE_PALETTE_SHIFT) & TILE_PALETTE_MASK
            rgb = color(cgram, palette * COLORS_PER_4BPP_PALETTE + palette_index)
            output = (y * FRAME_WIDTH + x) * 3
            pixels[output : output + 3] = bytes(rgb)
    return bytes(pixels)


def offset_word(
    vram: bytes,
    offsets: OffsetMap,
    fetch_column: int,
    vertical: bool,
) -> int:
    column_mask = offsets.tiles_wide - 1
    row_mask = offsets.tiles_high - 1
    column = (
        (fetch_column * 8 + (offsets.horizontal_scroll & ~7)) >> 3
    ) & column_mask
    row = (offsets.vertical_scroll >> 3) & row_mask
    entry = column + row * 32
    if vertical:
        entry = (entry + 32) & (2_047 if offsets.tiles_high == 64 else 1_023)
    return vram_word(vram, offsets.tilemap_word_address + entry)


def decode_mode2_offset_layer(
    vram: bytes,
    cgram: bytes,
    layer: Layer,
    offsets: OffsetMap,
) -> bytes:
    """Decode one Mode-2 layer with the retail offset-per-tile fetch cadence.

    The first fetched 8-pixel column uses the layer's ordinary scroll. Each
    following column uses the horizontal and vertical words fetched for the
    preceding column. This mirrors the PPU pipeline while keeping all address
    interpretation in this verification-only extractor.
    """

    pixels = bytearray(FRAME_WIDTH * FRAME_HEIGHT * 3)
    enable_bit = 8_192
    original_horizontal_scroll = layer.horizontal_scroll
    for y in range(FRAME_HEIGHT):
        for x in range(FRAME_WIDTH):
            fetch_column = (x + (original_horizontal_scroll & 7)) >> 3
            horizontal_scroll = original_horizontal_scroll
            vertical_scroll = layer.vertical_scroll
            if fetch_column > 0:
                horizontal_offset = offset_word(
                    vram, offsets, fetch_column - 1, vertical=False
                )
                vertical_offset = offset_word(
                    vram, offsets, fetch_column - 1, vertical=True
                )
                if horizontal_offset & enable_bit:
                    horizontal_scroll = (
                        original_horizontal_scroll & 7
                    ) | (horizontal_offset & 1_016)
                if vertical_offset & enable_bit:
                    vertical_scroll = vertical_offset & 1_023

            tile_size = 16 if layer.large_tiles else 8
            source_y = (y + vertical_scroll) & 1_023
            tile_column = fetch_column + (horizontal_scroll >> 3)
            if layer.large_tiles:
                tile_column >>= 1
            entry = tilemap_word(
                vram,
                layer,
                tile_column,
                source_y // tile_size,
            )

            tile = entry & TILE_INDEX_MASK
            within_x = (x + original_horizontal_scroll) & 7
            within_y = source_y & 7
            if layer.large_tiles:
                second_horizontal_tile = (
                    (fetch_column * 8 + original_horizontal_scroll) & 8
                ) != 0
                second_vertical_tile = (source_y & 8) != 0
                if entry & TILE_FLIP_HORIZONTAL:
                    tile += 0 if second_horizontal_tile else 1
                else:
                    tile += 1 if second_horizontal_tile else 0
                if entry & TILE_FLIP_VERTICAL:
                    tile += 0 if second_vertical_tile else 16
                else:
                    tile += 16 if second_vertical_tile else 0
            if entry & TILE_FLIP_HORIZONTAL:
                within_x = 7 - within_x
            if entry & TILE_FLIP_VERTICAL:
                within_y = 7 - within_y

            palette_index = tile_pixel(
                vram,
                layer.character_word_address,
                tile & TILE_INDEX_MASK,
                within_x,
                within_y,
            )
            palette = (entry >> TILE_PALETTE_SHIFT) & TILE_PALETTE_MASK
            rgb = color(cgram, palette * COLORS_PER_4BPP_PALETTE + palette_index)
            output = (y * FRAME_WIDTH + x) * 3
            pixels[output : output + 3] = bytes(rgb)
    return bytes(pixels)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("vram", type=Path)
    parser.add_argument("cgram", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--tilemap", type=lambda value: int(value, 0), required=True)
    parser.add_argument("--characters", type=lambda value: int(value, 0), required=True)
    parser.add_argument("--hscroll", type=int, default=0)
    parser.add_argument("--vscroll", type=int, default=0)
    parser.add_argument("--wide", action="store_true")
    parser.add_argument("--tall", action="store_true")
    parser.add_argument("--large-tiles", action="store_true")
    parser.add_argument(
        "--offset-tilemap",
        type=lambda value: int(value, 0),
        help="Mode-2 offset-map word address; enables offset-per-tile decoding",
    )
    parser.add_argument("--offset-hscroll", type=int, default=0)
    parser.add_argument("--offset-vscroll", type=int, default=0)
    parser.add_argument("--offset-wide", action="store_true")
    parser.add_argument("--offset-tall", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    vram = args.vram.read_bytes()
    cgram = args.cgram.read_bytes()
    if len(vram) != 65_536:
        raise SystemExit(f"expected 65536 VRAM bytes, found {len(vram)}")
    if len(cgram) != 512:
        raise SystemExit(f"expected 512 CGRAM bytes, found {len(cgram)}")
    layer = Layer(
        tilemap_word_address=args.tilemap,
        character_word_address=args.characters,
        horizontal_scroll=args.hscroll,
        vertical_scroll=args.vscroll,
        tiles_wide=64 if args.wide else 32,
        tiles_high=64 if args.tall else 32,
        large_tiles=args.large_tiles,
    )
    if args.offset_tilemap is None:
        pixels = decode(vram, cgram, layer)
    else:
        offsets = OffsetMap(
            tilemap_word_address=args.offset_tilemap,
            horizontal_scroll=args.offset_hscroll,
            vertical_scroll=args.offset_vscroll,
            tiles_wide=64 if args.offset_wide else 32,
            tiles_high=64 if args.offset_tall else 32,
        )
        pixels = decode_mode2_offset_layer(vram, cgram, layer, offsets)
    args.output.write_bytes(
        f"P6\n{FRAME_WIDTH} {FRAME_HEIGHT}\n255\n".encode("ascii") + pixels
    )


if __name__ == "__main__":
    main()
