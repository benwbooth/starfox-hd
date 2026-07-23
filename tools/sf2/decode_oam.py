#!/usr/bin/env python3
"""Decode an oracle SNES OAM/VRAM/CGRAM snapshot into a portable PPM.

This tool is verification-only. It turns Mesen's raw PPU memories into a
normal image and a semantic sprite listing; the shipping Rust port consumes
neither OAM nor PPU address state.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path

from decode_ppu_background import color, tile_pixel


FRAME_WIDTH = 256
FRAME_HEIGHT = 224
SPRITE_COUNT = 128
SPRITE_RECORD_BYTES = 4
HIGH_TABLE_OFFSET = SPRITE_COUNT * SPRITE_RECORD_BYTES
SPRITES_PER_HIGH_BYTE = 4
BITS_PER_HIGH_ENTRY = 2
X_HIGH_MASK = 1
LARGE_SIZE_MASK = 2
ATTRIBUTE_NAME_TABLE_MASK = 1
ATTRIBUTE_PALETTE_SHIFT = 1
ATTRIBUTE_PALETTE_MASK = 7
ATTRIBUTE_PRIORITY_SHIFT = 4
ATTRIBUTE_PRIORITY_MASK = 3
ATTRIBUTE_HORIZONTAL_FLIP = 64
ATTRIBUTE_VERTICAL_FLIP = 128
OBJECT_PALETTE_BASE = 128
COLORS_PER_PALETTE = 16
SMALL_SPRITE_SIZE = 8
LARGE_SPRITE_SIZE = 16
TILE_BYTES = 32
TILES_PER_CHARACTER_ROW = 16
VRAM_WORDS = 32_768


@dataclass(frozen=True)
class Sprite:
    index: int
    x: int
    y: int
    tile: int
    palette: int
    priority: int
    horizontal_flip: bool
    vertical_flip: bool
    size: int


def sprites(oam: bytes) -> list[Sprite]:
    result = []
    for index in range(SPRITE_COUNT):
        offset = index * SPRITE_RECORD_BYTES
        high = oam[HIGH_TABLE_OFFSET + index // SPRITES_PER_HIGH_BYTE]
        high >>= (index % SPRITES_PER_HIGH_BYTE) * BITS_PER_HIGH_ENTRY
        x = oam[offset] | ((high & X_HIGH_MASK) << 8)
        if x >= FRAME_WIDTH:
            x -= 512
        attribute = oam[offset + 3]
        result.append(
            Sprite(
                index=index,
                x=x,
                y=oam[offset + 1],
                tile=oam[offset + 2]
                + (256 if attribute & ATTRIBUTE_NAME_TABLE_MASK else 0),
                palette=(attribute >> ATTRIBUTE_PALETTE_SHIFT) & ATTRIBUTE_PALETTE_MASK,
                priority=(attribute >> ATTRIBUTE_PRIORITY_SHIFT) & ATTRIBUTE_PRIORITY_MASK,
                horizontal_flip=bool(attribute & ATTRIBUTE_HORIZONTAL_FLIP),
                vertical_flip=bool(attribute & ATTRIBUTE_VERTICAL_FLIP),
                size=LARGE_SPRITE_SIZE if high & LARGE_SIZE_MASK else SMALL_SPRITE_SIZE,
            )
        )
    return result


def sprite_pixel(vram: bytes, base_word: int, sprite: Sprite, x: int, y: int) -> int:
    source_x = sprite.size - 1 - x if sprite.horizontal_flip else x
    source_y = sprite.size - 1 - y if sprite.vertical_flip else y
    tile = sprite.tile + source_x // 8 + (source_y // 8) * TILES_PER_CHARACTER_ROW
    tile_word = (base_word + tile * (TILE_BYTES // 2)) % VRAM_WORDS
    return tile_pixel(vram, tile_word, 0, source_x, source_y)


def decode(vram: bytes, cgram: bytes, oam: bytes, base_word: int) -> bytes:
    pixels = bytearray(FRAME_WIDTH * FRAME_HEIGHT * 3)
    # With OAM priority rotation disabled, lower-numbered entries win. Paint
    # backwards so a lower entry naturally replaces a higher one.
    for sprite in reversed(sprites(oam)):
        for local_y in range(sprite.size):
            screen_y = (sprite.y + local_y) & 255
            if screen_y >= FRAME_HEIGHT:
                continue
            for local_x in range(sprite.size):
                screen_x = sprite.x + local_x
                if not 0 <= screen_x < FRAME_WIDTH:
                    continue
                palette_index = sprite_pixel(vram, base_word, sprite, local_x, local_y)
                if palette_index == 0:
                    continue
                rgb = color(
                    cgram,
                    OBJECT_PALETTE_BASE
                    + sprite.palette * COLORS_PER_PALETTE
                    + palette_index,
                )
                output = (screen_y * FRAME_WIDTH + screen_x) * 3
                pixels[output : output + 3] = bytes(rgb)
    return bytes(pixels)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("vram", type=Path)
    parser.add_argument("cgram", type=Path)
    parser.add_argument("oam", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--characters", type=lambda value: int(value, 0), required=True)
    parser.add_argument("--list", action="store_true", dest="list_sprites")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    vram = args.vram.read_bytes()
    cgram = args.cgram.read_bytes()
    oam = args.oam.read_bytes()
    if len(vram) != 65_536:
        raise SystemExit(f"expected 65536 VRAM bytes, found {len(vram)}")
    if len(cgram) != 512:
        raise SystemExit(f"expected 512 CGRAM bytes, found {len(cgram)}")
    if len(oam) != 544:
        raise SystemExit(f"expected 544 OAM bytes, found {len(oam)}")
    if args.list_sprites:
        for sprite in sprites(oam):
            if sprite.y < FRAME_HEIGHT or sprite.y + sprite.size > 255:
                print(sprite)
    pixels = decode(vram, cgram, oam, args.characters)
    args.output.write_bytes(
        f"P6\n{FRAME_WIDTH} {FRAME_HEIGHT}\n255\n".encode("ascii") + pixels
    )


if __name__ == "__main__":
    main()
