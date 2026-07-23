#!/usr/bin/env python3
"""Generate semantic SF2 strategic-map sprites from an oracle PPU snapshot."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path

from decode_oam import COLORS_PER_PALETTE, OBJECT_PALETTE_BASE, color
from decode_ppu_background import tile_pixel
from generate_backdrop import (
    FNV_OFFSET_BASIS,
    FNV_PRIME,
    MAX_RUN_LENGTH,
    MESEN_CAPTURE_HEIGHT,
    MESEN_TOP_BORDER,
    read_ppm,
    fnv1a,
)
from generate_strategic_map_variant import generated_backdrop_rgb


TILE_SIZE = 8
LARGE_SIZE = 16
PILOT_COUNT = 6
CHANNELS_PER_PIXEL = 4
ACTOR_CELL_SIZE = 40
POST_PIGMA_ACTOR_CELL_SIZE = 48
POST_ELADARD_ACTOR_CELL_SIZE = 48
ACTOR_TOP = 16
ATLAS_WIDTH = 2_248
ATLAS_HEIGHT = ACTOR_TOP + POST_PIGMA_ACTOR_CELL_SIZE
PILOTS_LEFT = 0
SHIELD_FULL_LEFT = 96
SHIELD_EMPTY_LEFT = 104
ITEM_ICON_LEFT = 112
GAUGE_BAR_LEFT = 128
CRAFT_BODY_LEFT = 136
CRAFT_ACCENT_LEFT = 152
CRAFT_CURSOR_LEFT = 160
GAUGE_BAR_FLIPPED_LEFT = 168
CRAFT_ACCENT_HORIZONTAL_LEFT = 176
CRAFT_ACCENT_VERTICAL_LEFT = 184
NORTH_INSTALLATION_OPENING_LEFT = 192
NORTH_INSTALLATION_ESCALATED_LEFT = 232
SOUTH_INSTALLATION_OPENING_LEFT = 272
SOUTH_INSTALLATION_ESCALATED_LEFT = 312
ENEMY_CARRIER_OPENING_LEFT = 352
ENEMY_CARRIER_ESCALATED_LEFT = 392
ENEMY_FORMATION_OPENING_LEFT = 432
ENEMY_FORMATION_ESCALATED_LEFT = 472
EAST_INTERCEPTOR_OPENING_LEFT = 512
EAST_INTERCEPTOR_ESCALATED_LEFT = 552
PATROL_SHIP_LEFT = 592
MISSILE_TRAIL_OPENING_LEFT = 632
MISSILE_OPENING_LEFT = 672
MISSILE_ESCALATED_LEFT = 712
CRAFT_CURSOR_POST_INTERCEPTION_LEFT = 752
NORTH_INSTALLATION_POST_INTERCEPTION_LEFT = 760
SOUTH_INSTALLATION_POST_INTERCEPTION_LEFT = 800
ENEMY_CARRIER_POST_INTERCEPTION_LEFT = 840
ENEMY_FORMATION_POST_INTERCEPTION_LEFT = 880
EAST_INTERCEPTOR_POST_INTERCEPTION_LEFT = 920
ATTACKING_FIGHTER_POST_INTERCEPTION_LEFT = 960
CRAFT_CURSOR_POST_FIGHTER_INTERCEPT_LEFT = 1_000
NORTH_INSTALLATION_POST_FIGHTER_INTERCEPT_LEFT = 1_008
SOUTH_INSTALLATION_POST_FIGHTER_INTERCEPT_LEFT = 1_048
ENEMY_CARRIER_POST_FIGHTER_INTERCEPT_LEFT = 1_088
ENEMY_FORMATION_POST_FIGHTER_INTERCEPT_LEFT = 1_128
EAST_INTERCEPTOR_POST_FIGHTER_INTERCEPT_LEFT = 1_168
ATTACKING_FIGHTER_POST_FIGHTER_INTERCEPT_LEFT = 1_208
FIGHTER_PROJECTILE_POST_FIGHTER_INTERCEPT_LEFT = 1_248
CRAFT_CURSOR_POST_PIGMA_LEFT = 1_288
NORTH_INSTALLATION_POST_PIGMA_LEFT = 1_296
SOUTH_INSTALLATION_POST_PIGMA_LEFT = 1_344
ENEMY_CARRIER_POST_PIGMA_LEFT = 1_392
ENEMY_FORMATION_POST_PIGMA_LEFT = 1_440
EAST_INTERCEPTOR_POST_PIGMA_LEFT = 1_488
PATROL_SHIP_POST_PIGMA_LEFT = 1_536
RIVAL_FIGHTER_POST_PIGMA_LEFT = 1_584
FIGHTER_PROJECTILE_POST_PIGMA_LEFT = 1_632
CRAFT_CURSOR_POST_ELADARD_LEFT = 1_680
NORTH_INSTALLATION_POST_ELADARD_LEFT = 1_688
SOUTH_INSTALLATION_POST_ELADARD_LEFT = 1_736
ENEMY_CARRIER_POST_ELADARD_LEFT = 1_784
ENEMY_FORMATION_POST_ELADARD_LEFT = 1_832
EAST_INTERCEPTOR_POST_ELADARD_LEFT = 1_880
PATROL_SHIP_POST_ELADARD_LEFT = 1_928
ATTACKING_FIGHTER_POST_ELADARD_LEFT = 1_976
UNKNOWN_SIGNAL_POST_ELADARD_LEFT = 2_024
FIGHTER_PROJECTILE_POST_ELADARD_LEFT = 2_072
PILOTS_POST_ELADARD_LEFT = 2_120
SHIELD_FULL_POST_ELADARD_LEFT = 2_216
SHIELD_EMPTY_POST_ELADARD_LEFT = 2_224
ITEM_ICON_POST_ELADARD_LEFT = 2_232


@dataclass(frozen=True)
class SpriteSpec:
    tile: int
    palette: int
    size: int
    horizontal_flip: bool = False
    vertical_flip: bool = False


def render_sprite(
    pixels: bytearray,
    vram: bytes,
    cgram: bytes,
    character_base: int,
    left: int,
    top: int,
    spec: SpriteSpec,
) -> None:
    for y in range(spec.size):
        source_y = spec.size - 1 - y if spec.vertical_flip else y
        for x in range(spec.size):
            source_x = spec.size - 1 - x if spec.horizontal_flip else x
            tile = (
                spec.tile
                + source_x // TILE_SIZE
                + (source_y // TILE_SIZE) * 16
            )
            palette_index = tile_pixel(
                vram,
                character_base,
                tile,
                source_x % TILE_SIZE,
                source_y % TILE_SIZE,
            )
            if palette_index == 0:
                continue
            red, green, blue = color(
                cgram,
                OBJECT_PALETTE_BASE
                + spec.palette * COLORS_PER_PALETTE
                + palette_index,
            )
            output = ((top + y) * ATLAS_WIDTH + left + x) * CHANNELS_PER_PIXEL
            pixels[output : output + CHANNELS_PER_PIXEL] = bytes(
                (red, green, blue, 255)
            )


def precompose_color_math(
    pixels: bytearray,
    capture: bytes,
    atlas_left: int,
    screen_left: int,
    screen_top: int,
    cell_size: int = ACTOR_CELL_SIZE,
) -> None:
    """Bake the retail visible color for non-transparent object pixels.

    The map uses PPU color math for several object palettes.  This oracle-side
    step resolves that presentation detail into ordinary RGBA colors while the
    shipping renderer continues to draw a semantic actor texture.
    """

    for local_y in range(cell_size):
        for local_x in range(cell_size):
            atlas_offset = (
                (ACTOR_TOP + local_y) * ATLAS_WIDTH + atlas_left + local_x
            ) * CHANNELS_PER_PIXEL
            if pixels[atlas_offset + 3] == 0:
                continue
            screen_x = screen_left + local_x
            screen_y = screen_top + local_y
            if not 0 <= screen_x < 256 or not 0 <= screen_y < 224:
                continue
            capture_offset = (screen_y * 256 + screen_x) * 3
            pixels[atlas_offset : atlas_offset + 3] = capture[
                capture_offset : capture_offset + 3
            ]


def precompose_small_sprite(
    pixels: bytearray,
    capture: bytes,
    atlas_left: int,
    screen_left: int,
    screen_top: int,
    size: int = TILE_SIZE,
) -> None:
    for local_y in range(size):
        for local_x in range(size):
            atlas_offset = (local_y * ATLAS_WIDTH + atlas_left + local_x) * CHANNELS_PER_PIXEL
            if pixels[atlas_offset + 3] == 0:
                continue
            screen_offset = (
                (screen_top + local_y) * 256 + screen_left + local_x
            ) * 3
            pixels[atlas_offset : atlas_offset + 3] = capture[
                screen_offset : screen_offset + 3
            ]


def extract_capture_sprite(
    pixels: bytearray,
    capture: bytes,
    backdrop: bytes,
    atlas_left: int,
    screen_left: int,
    screen_top: int,
    size: int,
) -> None:
    """Copy one semantic object by subtracting the object-free backdrop."""

    for local_y in range(size):
        for local_x in range(size):
            atlas_offset = (local_y * ATLAS_WIDTH + atlas_left + local_x) * CHANNELS_PER_PIXEL
            screen_offset = (
                (screen_top + local_y) * 256 + screen_left + local_x
            ) * 3
            pixels[atlas_offset : atlas_offset + CHANNELS_PER_PIXEL] = bytes((0, 0, 0, 0))
            visible = capture[screen_offset : screen_offset + 3]
            if visible != backdrop[screen_offset : screen_offset + 3]:
                pixels[atlas_offset : atlas_offset + CHANNELS_PER_PIXEL] = bytes((*visible, 255))


def read_capture(path: Path) -> bytes:
    width, height, pixels = read_ppm(path)
    if width != 256 or height != MESEN_CAPTURE_HEIGHT:
        raise SystemExit(f"expected a 256x{MESEN_CAPTURE_HEIGHT} capture, found {width}x{height}")
    row_bytes = width * 3
    top = MESEN_TOP_BORDER * row_bytes
    return pixels[top : top + 224 * row_bytes]


def render_atlas(
    vram: bytes,
    cgram: bytes,
    opening_vram: bytes,
    opening_cgram: bytes,
    character_base: int,
    opening_capture: bytes,
    escalated_capture: bytes,
    post_interception_vram: bytes,
    post_interception_cgram: bytes,
    post_interception_capture: bytes,
    post_fighter_intercept_vram: bytes,
    post_fighter_intercept_cgram: bytes,
    post_fighter_intercept_capture: bytes,
    post_pigma_vram: bytes,
    post_pigma_cgram: bytes,
    post_pigma_capture: bytes,
    post_eladard_vram: bytes,
    post_eladard_cgram: bytes,
    post_eladard_capture: bytes,
    post_eladard_backdrop: bytes,
) -> bytes:
    pixels = bytearray(ATLAS_WIDTH * ATLAS_HEIGHT * CHANNELS_PER_PIXEL)
    for pilot in range(PILOT_COUNT):
        render_sprite(
            pixels,
            vram,
            cgram,
            character_base,
            PILOTS_LEFT + pilot * LARGE_SIZE,
            0,
            SpriteSpec(32 + pilot * 2, 0, LARGE_SIZE),
        )
    for left, spec in (
        (SHIELD_FULL_LEFT, SpriteSpec(69, 3, TILE_SIZE)),
        (SHIELD_EMPTY_LEFT, SpriteSpec(71, 3, TILE_SIZE)),
        (ITEM_ICON_LEFT, SpriteSpec(98, 0, LARGE_SIZE)),
        (GAUGE_BAR_LEFT, SpriteSpec(367, 0, TILE_SIZE)),
        (CRAFT_BODY_LEFT, SpriteSpec(198, 0, LARGE_SIZE)),
        (CRAFT_ACCENT_LEFT, SpriteSpec(195, 0, TILE_SIZE)),
        (CRAFT_CURSOR_LEFT, SpriteSpec(463, 1, TILE_SIZE)),
        (GAUGE_BAR_FLIPPED_LEFT, SpriteSpec(367, 0, TILE_SIZE, vertical_flip=True)),
        (CRAFT_ACCENT_HORIZONTAL_LEFT, SpriteSpec(195, 0, TILE_SIZE, horizontal_flip=True)),
        (CRAFT_ACCENT_VERTICAL_LEFT, SpriteSpec(195, 0, TILE_SIZE, vertical_flip=True)),
    ):
        render_sprite(pixels, vram, cgram, character_base, left, 0, spec)

    for pilot in range(PILOT_COUNT):
        render_sprite(
            pixels,
            post_eladard_vram,
            post_eladard_cgram,
            character_base,
            PILOTS_POST_ELADARD_LEFT + pilot * LARGE_SIZE,
            0,
            SpriteSpec(32 + pilot * 2, 0, LARGE_SIZE),
        )
    for left, spec in (
        (SHIELD_FULL_POST_ELADARD_LEFT, SpriteSpec(69, 3, TILE_SIZE)),
        (SHIELD_EMPTY_POST_ELADARD_LEFT, SpriteSpec(71, 3, TILE_SIZE)),
        (ITEM_ICON_POST_ELADARD_LEFT, SpriteSpec(98, 0, LARGE_SIZE)),
    ):
        render_sprite(
            pixels,
            post_eladard_vram,
            post_eladard_cgram,
            character_base,
            left,
            0,
            spec,
        )

    def composite(
        left: int,
        sprites: tuple[tuple[int, int, SpriteSpec], ...],
        source_vram: bytes = vram,
        source_cgram: bytes = cgram,
    ) -> None:
        for local_left, local_top, spec in sprites:
            render_sprite(
                pixels,
                source_vram,
                source_cgram,
                character_base,
                left + local_left,
                ACTOR_TOP + local_top,
                spec,
            )

    north_base = (
        (16, 16, SpriteSpec(202, 7, LARGE_SIZE, horizontal_flip=True)),
        (0, 16, SpriteSpec(202, 7, LARGE_SIZE)),
        (16, 0, SpriteSpec(200, 7, LARGE_SIZE, horizontal_flip=True)),
        (0, 0, SpriteSpec(200, 7, LARGE_SIZE)),
    )
    south_base = (
        (16, 16, SpriteSpec(234, 7, LARGE_SIZE)),
        (0, 16, SpriteSpec(206, 7, LARGE_SIZE)),
        (16, 0, SpriteSpec(204, 7, LARGE_SIZE, horizontal_flip=True)),
        (0, 0, SpriteSpec(204, 7, LARGE_SIZE)),
    )
    composite(
        NORTH_INSTALLATION_OPENING_LEFT,
        north_base + ((16, 8, SpriteSpec(163, 0, TILE_SIZE)),),
        opening_vram,
        opening_cgram,
    )
    composite(
        NORTH_INSTALLATION_ESCALATED_LEFT,
        north_base
        + (
            (12, 20, SpriteSpec(224, 0, TILE_SIZE)),
            (12, 12, SpriteSpec(176, 0, TILE_SIZE)),
            (16, 8, SpriteSpec(164, 0, TILE_SIZE)),
        ),
    )
    composite(
        SOUTH_INSTALLATION_OPENING_LEFT,
        south_base + ((16, 8, SpriteSpec(163, 0, TILE_SIZE)),),
        opening_vram,
        opening_cgram,
    )
    composite(
        SOUTH_INSTALLATION_ESCALATED_LEFT,
        south_base + ((16, 8, SpriteSpec(164, 0, TILE_SIZE)),),
    )
    carrier_base = (
        (16, 16, SpriteSpec(6, 0, LARGE_SIZE, horizontal_flip=True)),
        (0, 16, SpriteSpec(6, 0, LARGE_SIZE)),
        (16, 0, SpriteSpec(4, 0, LARGE_SIZE, horizontal_flip=True)),
        (0, 0, SpriteSpec(4, 0, LARGE_SIZE)),
    )
    composite(
        ENEMY_CARRIER_OPENING_LEFT,
        carrier_base
        + ((10, 8, SpriteSpec(10, 0, LARGE_SIZE, horizontal_flip=True, vertical_flip=True)),),
        opening_vram,
        opening_cgram,
    )
    composite(
        ENEMY_CARRIER_ESCALATED_LEFT,
        carrier_base + ((6, 8, SpriteSpec(10, 0, LARGE_SIZE)),),
    )
    composite(
        ENEMY_FORMATION_OPENING_LEFT,
        (
            (3, 0, SpriteSpec(302, 0, LARGE_SIZE)),
            (3, 0, SpriteSpec(8, 0, LARGE_SIZE, horizontal_flip=True)),
            (3, 26, SpriteSpec(431, 4, TILE_SIZE)),
            (0, 23, SpriteSpec(179, 0, TILE_SIZE)),
            (7, 23, SpriteSpec(0, 0, LARGE_SIZE, horizontal_flip=True)),
        ),
        opening_vram,
        opening_cgram,
    )
    composite(
        ENEMY_FORMATION_ESCALATED_LEFT,
        (
            (17, 0, SpriteSpec(302, 0, LARGE_SIZE)),
            (17, 0, SpriteSpec(8, 0, LARGE_SIZE, vertical_flip=True)),
            (3, 9, SpriteSpec(447, 2, TILE_SIZE)),
            (0, 6, SpriteSpec(179, 0, TILE_SIZE)),
            (7, 12, SpriteSpec(0, 0, LARGE_SIZE, horizontal_flip=True)),
            (7, 12, SpriteSpec(72, 1, LARGE_SIZE)),
        ),
    )
    composite(
        EAST_INTERCEPTOR_OPENING_LEFT,
        (
            (0, 0, SpriteSpec(302, 0, LARGE_SIZE)),
            (0, 0, SpriteSpec(8, 0, LARGE_SIZE, horizontal_flip=True, vertical_flip=True)),
        ),
        opening_vram,
        opening_cgram,
    )
    composite(
        EAST_INTERCEPTOR_ESCALATED_LEFT,
        (
            (0, 0, SpriteSpec(302, 0, LARGE_SIZE)),
            (0, 0, SpriteSpec(8, 0, LARGE_SIZE)),
        ),
    )
    composite(PATROL_SHIP_LEFT, ((0, 0, SpriteSpec(300, 0, LARGE_SIZE)),))
    composite(
        MISSILE_TRAIL_OPENING_LEFT,
        (
            (4, 0, SpriteSpec(431, 4, TILE_SIZE)),
            (0, 0, SpriteSpec(178, 0, TILE_SIZE)),
        ),
        opening_vram,
        opening_cgram,
    )
    composite(
        MISSILE_OPENING_LEFT,
        (
            (4, 0, SpriteSpec(431, 4, TILE_SIZE)),
            (0, 0, SpriteSpec(179, 0, TILE_SIZE)),
        ),
        opening_vram,
        opening_cgram,
    )
    composite(
        MISSILE_ESCALATED_LEFT,
        (
            (4, 0, SpriteSpec(447, 2, TILE_SIZE)),
            (0, 0, SpriteSpec(179, 0, TILE_SIZE)),
        ),
    )
    render_sprite(
        pixels,
        post_interception_vram,
        post_interception_cgram,
        character_base,
        CRAFT_CURSOR_POST_INTERCEPTION_LEFT,
        0,
        SpriteSpec(192, 0, TILE_SIZE, vertical_flip=True),
    )
    composite(
        NORTH_INSTALLATION_POST_INTERCEPTION_LEFT,
        north_base + ((16, 8, SpriteSpec(165, 0, TILE_SIZE)),),
        post_interception_vram,
        post_interception_cgram,
    )
    composite(
        SOUTH_INSTALLATION_POST_INTERCEPTION_LEFT,
        south_base + ((16, 8, SpriteSpec(165, 0, TILE_SIZE)),),
        post_interception_vram,
        post_interception_cgram,
    )
    composite(
        ENEMY_CARRIER_POST_INTERCEPTION_LEFT,
        carrier_base + ((6, 8, SpriteSpec(10, 0, LARGE_SIZE)),),
        post_interception_vram,
        post_interception_cgram,
    )
    composite(
        ENEMY_FORMATION_POST_INTERCEPTION_LEFT,
        (
            (0, 0, SpriteSpec(302, 0, LARGE_SIZE)),
            (0, 0, SpriteSpec(8, 0, LARGE_SIZE, vertical_flip=True)),
        ),
        post_interception_vram,
        post_interception_cgram,
    )
    composite(
        EAST_INTERCEPTOR_POST_INTERCEPTION_LEFT,
        (
            (0, 0, SpriteSpec(302, 0, LARGE_SIZE)),
            (0, 0, SpriteSpec(8, 0, LARGE_SIZE)),
        ),
        post_interception_vram,
        post_interception_cgram,
    )
    composite(
        ATTACKING_FIGHTER_POST_INTERCEPTION_LEFT,
        (
            (3, 0, SpriteSpec(0, 0, LARGE_SIZE, horizontal_flip=True)),
            (2, 5, SpriteSpec(178, 0, TILE_SIZE)),
            (18, 4, SpriteSpec(179, 0, TILE_SIZE)),
        ),
        post_interception_vram,
        post_interception_cgram,
    )
    render_sprite(
        pixels,
        post_fighter_intercept_vram,
        post_fighter_intercept_cgram,
        character_base,
        CRAFT_CURSOR_POST_FIGHTER_INTERCEPT_LEFT,
        0,
        SpriteSpec(208, 0, TILE_SIZE, horizontal_flip=True),
    )
    composite(
        NORTH_INSTALLATION_POST_FIGHTER_INTERCEPT_LEFT,
        north_base,
        post_fighter_intercept_vram,
        post_fighter_intercept_cgram,
    )
    composite(
        SOUTH_INSTALLATION_POST_FIGHTER_INTERCEPT_LEFT,
        south_base + ((16, 8, SpriteSpec(164, 0, TILE_SIZE)),),
        post_fighter_intercept_vram,
        post_fighter_intercept_cgram,
    )
    composite(
        ENEMY_CARRIER_POST_FIGHTER_INTERCEPT_LEFT,
        carrier_base + ((6, 8, SpriteSpec(10, 0, LARGE_SIZE)),),
        post_fighter_intercept_vram,
        post_fighter_intercept_cgram,
    )
    composite(
        ENEMY_FORMATION_POST_FIGHTER_INTERCEPT_LEFT,
        (
            (26, 0, SpriteSpec(164, 0, TILE_SIZE)),
            (0, 4, SpriteSpec(302, 0, LARGE_SIZE)),
            (0, 4, SpriteSpec(8, 0, LARGE_SIZE, horizontal_flip=True)),
        ),
        post_fighter_intercept_vram,
        post_fighter_intercept_cgram,
    )
    composite(
        EAST_INTERCEPTOR_POST_FIGHTER_INTERCEPT_LEFT,
        (
            (0, 0, SpriteSpec(302, 0, LARGE_SIZE)),
            (0, 0, SpriteSpec(8, 0, LARGE_SIZE, horizontal_flip=True)),
        ),
        post_fighter_intercept_vram,
        post_fighter_intercept_cgram,
    )
    composite(
        ATTACKING_FIGHTER_POST_FIGHTER_INTERCEPT_LEFT,
        (
            (7, 1, SpriteSpec(0, 0, LARGE_SIZE, horizontal_flip=True)),
            (0, 8, SpriteSpec(179, 0, TILE_SIZE)),
            (4, 8, SpriteSpec(447, 2, TILE_SIZE)),
        ),
        post_fighter_intercept_vram,
        post_fighter_intercept_cgram,
    )
    composite(
        FIGHTER_PROJECTILE_POST_FIGHTER_INTERCEPT_LEFT,
        (
            (0, 0, SpriteSpec(178, 0, TILE_SIZE)),
            (4, 0, SpriteSpec(447, 2, TILE_SIZE)),
        ),
        post_fighter_intercept_vram,
        post_fighter_intercept_cgram,
    )
    render_sprite(
        pixels,
        post_pigma_vram,
        post_pigma_cgram,
        character_base,
        CRAFT_CURSOR_POST_PIGMA_LEFT,
        0,
        SpriteSpec(193, 0, TILE_SIZE),
    )
    composite(
        NORTH_INSTALLATION_POST_PIGMA_LEFT,
        north_base
        + (
            (12, 12, SpriteSpec(176, 0, TILE_SIZE)),
            (12, 20, SpriteSpec(229, 0, TILE_SIZE)),
        ),
        post_pigma_vram,
        post_pigma_cgram,
    )
    composite(
        SOUTH_INSTALLATION_POST_PIGMA_LEFT,
        south_base,
        post_pigma_vram,
        post_pigma_cgram,
    )
    composite(
        ENEMY_CARRIER_POST_PIGMA_LEFT,
        carrier_base + ((6, 8, SpriteSpec(10, 0, LARGE_SIZE)),),
        post_pigma_vram,
        post_pigma_cgram,
    )
    composite(
        ENEMY_FORMATION_POST_PIGMA_LEFT,
        (
            (0, 0, SpriteSpec(302, 0, LARGE_SIZE)),
            (0, 0, SpriteSpec(8, 0, LARGE_SIZE, vertical_flip=True)),
            (13, 8, SpriteSpec(179, 0, TILE_SIZE)),
        ),
        post_pigma_vram,
        post_pigma_cgram,
    )
    composite(
        EAST_INTERCEPTOR_POST_PIGMA_LEFT,
        (
            (0, 3, SpriteSpec(164, 0, TILE_SIZE)),
            (26, 0, SpriteSpec(302, 0, LARGE_SIZE)),
            (26, 0, SpriteSpec(8, 0, LARGE_SIZE)),
        ),
        post_pigma_vram,
        post_pigma_cgram,
    )
    composite(
        PATROL_SHIP_POST_PIGMA_LEFT,
        (
            (0, 5, SpriteSpec(300, 0, LARGE_SIZE)),
            (34, 0, SpriteSpec(178, 0, TILE_SIZE)),
        ),
        post_pigma_vram,
        post_pigma_cgram,
    )
    composite(
        RIVAL_FIGHTER_POST_PIGMA_LEFT,
        ((0, 0, SpriteSpec(0, 0, LARGE_SIZE, horizontal_flip=True)),),
        post_pigma_vram,
        post_pigma_cgram,
    )
    composite(
        FIGHTER_PROJECTILE_POST_PIGMA_LEFT,
        ((0, 0, SpriteSpec(179, 0, TILE_SIZE)),),
        post_pigma_vram,
        post_pigma_cgram,
    )
    render_sprite(
        pixels,
        post_eladard_vram,
        post_eladard_cgram,
        character_base,
        CRAFT_CURSOR_POST_ELADARD_LEFT,
        0,
        SpriteSpec(192, 0, TILE_SIZE, vertical_flip=True),
    )
    composite(
        NORTH_INSTALLATION_POST_ELADARD_LEFT,
        (
            (0, 2, SpriteSpec(200, 7, LARGE_SIZE)),
            (16, 2, SpriteSpec(200, 7, LARGE_SIZE, horizontal_flip=True)),
            (0, 18, SpriteSpec(202, 7, LARGE_SIZE)),
            (16, 18, SpriteSpec(202, 7, LARGE_SIZE, horizontal_flip=True)),
            (3, 0, SpriteSpec(177, 0, TILE_SIZE, vertical_flip=True)),
        ),
        post_eladard_vram,
        post_eladard_cgram,
    )
    composite(
        SOUTH_INSTALLATION_POST_ELADARD_LEFT,
        south_base + ((11, 12, SpriteSpec(163, 0, TILE_SIZE)),),
        post_eladard_vram,
        post_eladard_cgram,
    )
    composite(
        ENEMY_CARRIER_POST_ELADARD_LEFT,
        carrier_base
        + ((10, 8, SpriteSpec(10, 0, LARGE_SIZE, horizontal_flip=True, vertical_flip=True)),),
        post_eladard_vram,
        post_eladard_cgram,
    )
    composite(
        ENEMY_FORMATION_POST_ELADARD_LEFT,
        (
            (0, 0, SpriteSpec(8, 0, LARGE_SIZE, horizontal_flip=True)),
            (0, 0, SpriteSpec(302, 0, LARGE_SIZE)),
        ),
        post_eladard_vram,
        post_eladard_cgram,
    )
    composite(
        EAST_INTERCEPTOR_POST_ELADARD_LEFT,
        (
            (
                0,
                0,
                SpriteSpec(
                    8,
                    0,
                    LARGE_SIZE,
                    horizontal_flip=True,
                    vertical_flip=True,
                ),
            ),
            (0, 0, SpriteSpec(302, 0, LARGE_SIZE)),
        ),
        post_eladard_vram,
        post_eladard_cgram,
    )
    composite(
        PATROL_SHIP_POST_ELADARD_LEFT,
        ((0, 0, SpriteSpec(300, 0, LARGE_SIZE)),),
        post_eladard_vram,
        post_eladard_cgram,
    )
    composite(
        ATTACKING_FIGHTER_POST_ELADARD_LEFT,
        ((0, 0, SpriteSpec(0, 1, LARGE_SIZE)),),
        post_eladard_vram,
        post_eladard_cgram,
    )
    composite(
        UNKNOWN_SIGNAL_POST_ELADARD_LEFT,
        (
            (0, 4, SpriteSpec(320, 1, LARGE_SIZE)),
            (4, 0, SpriteSpec(179, 0, TILE_SIZE)),
        ),
        post_eladard_vram,
        post_eladard_cgram,
    )
    composite(
        FIGHTER_PROJECTILE_POST_ELADARD_LEFT,
        ((0, 0, SpriteSpec(179, 0, TILE_SIZE)),),
        post_eladard_vram,
        post_eladard_cgram,
    )
    for capture, atlas_left, screen_left, screen_top in (
        (opening_capture, NORTH_INSTALLATION_OPENING_LEFT, 16, 14),
        (opening_capture, SOUTH_INSTALLATION_OPENING_LEFT, 208, 110),
        (opening_capture, ENEMY_CARRIER_OPENING_LEFT, 220, 7),
        (opening_capture, ENEMY_FORMATION_OPENING_LEFT, 62, 40),
        (opening_capture, EAST_INTERCEPTOR_OPENING_LEFT, 203, 88),
        (opening_capture, MISSILE_TRAIL_OPENING_LEFT, 100, 132),
        (opening_capture, MISSILE_OPENING_LEFT, 180, 117),
        (escalated_capture, NORTH_INSTALLATION_ESCALATED_LEFT, 16, 14),
        (escalated_capture, SOUTH_INSTALLATION_ESCALATED_LEFT, 208, 110),
        (escalated_capture, ENEMY_CARRIER_ESCALATED_LEFT, 220, 7),
        (escalated_capture, ENEMY_FORMATION_ESCALATED_LEFT, 45, 45),
        (escalated_capture, EAST_INTERCEPTOR_ESCALATED_LEFT, 198, 89),
        (escalated_capture, MISSILE_ESCALATED_LEFT, 147, 125),
        (post_interception_capture, NORTH_INSTALLATION_POST_INTERCEPTION_LEFT, 16, 14),
        (post_interception_capture, SOUTH_INSTALLATION_POST_INTERCEPTION_LEFT, 208, 110),
        (post_interception_capture, ENEMY_CARRIER_POST_INTERCEPTION_LEFT, 220, 7),
        (post_interception_capture, ENEMY_FORMATION_POST_INTERCEPTION_LEFT, 47, 66),
        (post_interception_capture, EAST_INTERCEPTOR_POST_INTERCEPTION_LEFT, 172, 94),
        (post_interception_capture, ATTACKING_FIGHTER_POST_INTERCEPTION_LEFT, 132, 119),
        (
            post_fighter_intercept_capture,
            NORTH_INSTALLATION_POST_FIGHTER_INTERCEPT_LEFT,
            16,
            14,
        ),
        (
            post_fighter_intercept_capture,
            SOUTH_INSTALLATION_POST_FIGHTER_INTERCEPT_LEFT,
            208,
            110,
        ),
        (
            post_fighter_intercept_capture,
            ENEMY_CARRIER_POST_FIGHTER_INTERCEPT_LEFT,
            220,
            7,
        ),
        (
            post_fighter_intercept_capture,
            ENEMY_FORMATION_POST_FIGHTER_INTERCEPT_LEFT,
            46,
            64,
        ),
        (
            post_fighter_intercept_capture,
            EAST_INTERCEPTOR_POST_FIGHTER_INTERCEPT_LEFT,
            170,
            95,
        ),
        (
            post_fighter_intercept_capture,
            ATTACKING_FIGHTER_POST_FIGHTER_INTERCEPT_LEFT,
            135,
            119,
        ),
        (
            post_fighter_intercept_capture,
            FIGHTER_PROJECTILE_POST_FIGHTER_INTERCEPT_LEFT,
            86,
            136,
        ),
        (post_pigma_capture, NORTH_INSTALLATION_POST_PIGMA_LEFT, 16, 14),
        (post_pigma_capture, SOUTH_INSTALLATION_POST_PIGMA_LEFT, 208, 110),
        (post_pigma_capture, ENEMY_CARRIER_POST_PIGMA_LEFT, 220, 7),
        (post_pigma_capture, ENEMY_FORMATION_POST_PIGMA_LEFT, 44, 71),
        (post_pigma_capture, EAST_INTERCEPTOR_POST_PIGMA_LEFT, 140, 95),
        (post_pigma_capture, PATROL_SHIP_POST_PIGMA_LEFT, 12, 145),
        (post_pigma_capture, RIVAL_FIGHTER_POST_PIGMA_LEFT, 211, 120),
        (post_pigma_capture, FIGHTER_PROJECTILE_POST_PIGMA_LEFT, 115, 132),
    ):
        precompose_color_math(
            pixels,
            capture,
            atlas_left,
            screen_left,
            screen_top,
        )
    for atlas_left, screen_left, screen_top in (
        (NORTH_INSTALLATION_POST_PIGMA_LEFT, 16, 14),
        (SOUTH_INSTALLATION_POST_PIGMA_LEFT, 208, 110),
        (ENEMY_CARRIER_POST_PIGMA_LEFT, 220, 7),
        (ENEMY_FORMATION_POST_PIGMA_LEFT, 44, 71),
        (EAST_INTERCEPTOR_POST_PIGMA_LEFT, 140, 95),
        (PATROL_SHIP_POST_PIGMA_LEFT, 12, 145),
        (RIVAL_FIGHTER_POST_PIGMA_LEFT, 211, 120),
        (FIGHTER_PROJECTILE_POST_PIGMA_LEFT, 115, 132),
    ):
        precompose_color_math(
            pixels,
            post_pigma_capture,
            atlas_left,
            screen_left,
            screen_top,
            POST_PIGMA_ACTOR_CELL_SIZE,
        )
    for atlas_left, screen_left, screen_top in (
        (NORTH_INSTALLATION_POST_ELADARD_LEFT, 16, 12),
        (SOUTH_INSTALLATION_POST_ELADARD_LEFT, 208, 110),
        (ENEMY_CARRIER_POST_ELADARD_LEFT, 220, 7),
        (ENEMY_FORMATION_POST_ELADARD_LEFT, 41, 75),
        (EAST_INTERCEPTOR_POST_ELADARD_LEFT, 161, 96),
        (PATROL_SHIP_POST_ELADARD_LEFT, 12, 150),
        (ATTACKING_FIGHTER_POST_ELADARD_LEFT, 192, 122),
        (UNKNOWN_SIGNAL_POST_ELADARD_LEFT, 45, 101),
        (FIGHTER_PROJECTILE_POST_ELADARD_LEFT, 86, 139),
    ):
        precompose_color_math(
            pixels,
            post_eladard_capture,
            atlas_left,
            screen_left,
            screen_top,
            POST_ELADARD_ACTOR_CELL_SIZE,
        )
    precompose_small_sprite(
        pixels,
        escalated_capture,
        CRAFT_CURSOR_LEFT,
        68,
        108,
    )
    precompose_small_sprite(
        pixels,
        post_interception_capture,
        CRAFT_CURSOR_POST_INTERCEPTION_LEFT,
        76,
        108,
    )
    precompose_small_sprite(
        pixels,
        post_fighter_intercept_capture,
        CRAFT_CURSOR_POST_FIGHTER_INTERCEPT_LEFT,
        80,
        104,
    )
    precompose_small_sprite(
        pixels,
        post_pigma_capture,
        CRAFT_CURSOR_POST_PIGMA_LEFT,
        73,
        100,
    )
    precompose_small_sprite(
        pixels,
        post_eladard_capture,
        CRAFT_CURSOR_POST_ELADARD_LEFT,
        76,
        108,
    )
    for atlas_left, screen_left, screen_top, size in (
        (PILOTS_POST_ELADARD_LEFT, 74, 196, LARGE_SIZE),
        (PILOTS_POST_ELADARD_LEFT + 3 * LARGE_SIZE, 93, 196, LARGE_SIZE),
        (SHIELD_FULL_POST_ELADARD_LEFT, 114, 197, TILE_SIZE),
        (ITEM_ICON_POST_ELADARD_LEFT, 157, 196, LARGE_SIZE),
    ):
        extract_capture_sprite(
            pixels,
            post_eladard_capture,
            post_eladard_backdrop,
            atlas_left,
            screen_left,
            screen_top,
            size,
        )
    return bytes(pixels)


def encode(pixels: bytes) -> tuple[list[tuple[int, ...]], list[tuple[int, int]]]:
    palette: list[tuple[int, ...]] = []
    palette_indices: dict[tuple[int, ...], int] = {}
    runs: list[tuple[int, int]] = []
    for offset in range(0, len(pixels), CHANNELS_PER_PIXEL):
        rgba = tuple(pixels[offset : offset + CHANNELS_PER_PIXEL])
        index = palette_indices.setdefault(rgba, len(palette))
        if index == len(palette):
            palette.append(rgba)
        if runs and runs[-1][1] == index and runs[-1][0] < MAX_RUN_LENGTH:
            length, _ = runs[-1]
            runs[-1] = (length + 1, index)
        else:
            runs.append((1, index))
    return palette, runs


def rust_source(
    source_name: str,
    palette: list[tuple[int, ...]],
    runs: list[tuple[int, int]],
    source_hash: int,
) -> str:
    constants = {
        "WIDTH": ATLAS_WIDTH,
        "HEIGHT": ATLAS_HEIGHT,
        "PILOT_SIZE": LARGE_SIZE,
        "PILOTS_LEFT": PILOTS_LEFT,
        "SHIELD_FULL_LEFT": SHIELD_FULL_LEFT,
        "SHIELD_EMPTY_LEFT": SHIELD_EMPTY_LEFT,
        "ITEM_ICON_LEFT": ITEM_ICON_LEFT,
        "GAUGE_BAR_LEFT": GAUGE_BAR_LEFT,
        "CRAFT_BODY_LEFT": CRAFT_BODY_LEFT,
        "CRAFT_ACCENT_LEFT": CRAFT_ACCENT_LEFT,
        "CRAFT_CURSOR_LEFT": CRAFT_CURSOR_LEFT,
        "GAUGE_BAR_FLIPPED_LEFT": GAUGE_BAR_FLIPPED_LEFT,
        "CRAFT_ACCENT_HORIZONTAL_LEFT": CRAFT_ACCENT_HORIZONTAL_LEFT,
        "CRAFT_ACCENT_VERTICAL_LEFT": CRAFT_ACCENT_VERTICAL_LEFT,
        "ACTOR_CELL_SIZE": ACTOR_CELL_SIZE,
        "POST_PIGMA_ACTOR_CELL_SIZE": POST_PIGMA_ACTOR_CELL_SIZE,
        "POST_ELADARD_ACTOR_CELL_SIZE": POST_ELADARD_ACTOR_CELL_SIZE,
        "ACTOR_TOP": ACTOR_TOP,
        "NORTH_INSTALLATION_OPENING_LEFT": NORTH_INSTALLATION_OPENING_LEFT,
        "NORTH_INSTALLATION_ESCALATED_LEFT": NORTH_INSTALLATION_ESCALATED_LEFT,
        "SOUTH_INSTALLATION_OPENING_LEFT": SOUTH_INSTALLATION_OPENING_LEFT,
        "SOUTH_INSTALLATION_ESCALATED_LEFT": SOUTH_INSTALLATION_ESCALATED_LEFT,
        "ENEMY_CARRIER_OPENING_LEFT": ENEMY_CARRIER_OPENING_LEFT,
        "ENEMY_CARRIER_ESCALATED_LEFT": ENEMY_CARRIER_ESCALATED_LEFT,
        "ENEMY_FORMATION_OPENING_LEFT": ENEMY_FORMATION_OPENING_LEFT,
        "ENEMY_FORMATION_ESCALATED_LEFT": ENEMY_FORMATION_ESCALATED_LEFT,
        "EAST_INTERCEPTOR_OPENING_LEFT": EAST_INTERCEPTOR_OPENING_LEFT,
        "EAST_INTERCEPTOR_ESCALATED_LEFT": EAST_INTERCEPTOR_ESCALATED_LEFT,
        "PATROL_SHIP_LEFT": PATROL_SHIP_LEFT,
        "MISSILE_TRAIL_OPENING_LEFT": MISSILE_TRAIL_OPENING_LEFT,
        "MISSILE_OPENING_LEFT": MISSILE_OPENING_LEFT,
        "MISSILE_ESCALATED_LEFT": MISSILE_ESCALATED_LEFT,
        "CRAFT_CURSOR_POST_INTERCEPTION_LEFT": CRAFT_CURSOR_POST_INTERCEPTION_LEFT,
        "NORTH_INSTALLATION_POST_INTERCEPTION_LEFT": NORTH_INSTALLATION_POST_INTERCEPTION_LEFT,
        "SOUTH_INSTALLATION_POST_INTERCEPTION_LEFT": SOUTH_INSTALLATION_POST_INTERCEPTION_LEFT,
        "ENEMY_CARRIER_POST_INTERCEPTION_LEFT": ENEMY_CARRIER_POST_INTERCEPTION_LEFT,
        "ENEMY_FORMATION_POST_INTERCEPTION_LEFT": ENEMY_FORMATION_POST_INTERCEPTION_LEFT,
        "EAST_INTERCEPTOR_POST_INTERCEPTION_LEFT": EAST_INTERCEPTOR_POST_INTERCEPTION_LEFT,
        "ATTACKING_FIGHTER_POST_INTERCEPTION_LEFT": ATTACKING_FIGHTER_POST_INTERCEPTION_LEFT,
        "CRAFT_CURSOR_POST_FIGHTER_INTERCEPT_LEFT": CRAFT_CURSOR_POST_FIGHTER_INTERCEPT_LEFT,
        "NORTH_INSTALLATION_POST_FIGHTER_INTERCEPT_LEFT": NORTH_INSTALLATION_POST_FIGHTER_INTERCEPT_LEFT,
        "SOUTH_INSTALLATION_POST_FIGHTER_INTERCEPT_LEFT": SOUTH_INSTALLATION_POST_FIGHTER_INTERCEPT_LEFT,
        "ENEMY_CARRIER_POST_FIGHTER_INTERCEPT_LEFT": ENEMY_CARRIER_POST_FIGHTER_INTERCEPT_LEFT,
        "ENEMY_FORMATION_POST_FIGHTER_INTERCEPT_LEFT": ENEMY_FORMATION_POST_FIGHTER_INTERCEPT_LEFT,
        "EAST_INTERCEPTOR_POST_FIGHTER_INTERCEPT_LEFT": EAST_INTERCEPTOR_POST_FIGHTER_INTERCEPT_LEFT,
        "ATTACKING_FIGHTER_POST_FIGHTER_INTERCEPT_LEFT": ATTACKING_FIGHTER_POST_FIGHTER_INTERCEPT_LEFT,
        "FIGHTER_PROJECTILE_POST_FIGHTER_INTERCEPT_LEFT": FIGHTER_PROJECTILE_POST_FIGHTER_INTERCEPT_LEFT,
        "CRAFT_CURSOR_POST_PIGMA_LEFT": CRAFT_CURSOR_POST_PIGMA_LEFT,
        "NORTH_INSTALLATION_POST_PIGMA_LEFT": NORTH_INSTALLATION_POST_PIGMA_LEFT,
        "SOUTH_INSTALLATION_POST_PIGMA_LEFT": SOUTH_INSTALLATION_POST_PIGMA_LEFT,
        "ENEMY_CARRIER_POST_PIGMA_LEFT": ENEMY_CARRIER_POST_PIGMA_LEFT,
        "ENEMY_FORMATION_POST_PIGMA_LEFT": ENEMY_FORMATION_POST_PIGMA_LEFT,
        "EAST_INTERCEPTOR_POST_PIGMA_LEFT": EAST_INTERCEPTOR_POST_PIGMA_LEFT,
        "PATROL_SHIP_POST_PIGMA_LEFT": PATROL_SHIP_POST_PIGMA_LEFT,
        "RIVAL_FIGHTER_POST_PIGMA_LEFT": RIVAL_FIGHTER_POST_PIGMA_LEFT,
        "FIGHTER_PROJECTILE_POST_PIGMA_LEFT": FIGHTER_PROJECTILE_POST_PIGMA_LEFT,
        "CRAFT_CURSOR_POST_ELADARD_LEFT": CRAFT_CURSOR_POST_ELADARD_LEFT,
        "NORTH_INSTALLATION_POST_ELADARD_LEFT": NORTH_INSTALLATION_POST_ELADARD_LEFT,
        "SOUTH_INSTALLATION_POST_ELADARD_LEFT": SOUTH_INSTALLATION_POST_ELADARD_LEFT,
        "ENEMY_CARRIER_POST_ELADARD_LEFT": ENEMY_CARRIER_POST_ELADARD_LEFT,
        "ENEMY_FORMATION_POST_ELADARD_LEFT": ENEMY_FORMATION_POST_ELADARD_LEFT,
        "EAST_INTERCEPTOR_POST_ELADARD_LEFT": EAST_INTERCEPTOR_POST_ELADARD_LEFT,
        "PATROL_SHIP_POST_ELADARD_LEFT": PATROL_SHIP_POST_ELADARD_LEFT,
        "ATTACKING_FIGHTER_POST_ELADARD_LEFT": ATTACKING_FIGHTER_POST_ELADARD_LEFT,
        "UNKNOWN_SIGNAL_POST_ELADARD_LEFT": UNKNOWN_SIGNAL_POST_ELADARD_LEFT,
        "FIGHTER_PROJECTILE_POST_ELADARD_LEFT": FIGHTER_PROJECTILE_POST_ELADARD_LEFT,
        "PILOTS_POST_ELADARD_LEFT": PILOTS_POST_ELADARD_LEFT,
        "SHIELD_FULL_POST_ELADARD_LEFT": SHIELD_FULL_POST_ELADARD_LEFT,
        "SHIELD_EMPTY_POST_ELADARD_LEFT": SHIELD_EMPTY_POST_ELADARD_LEFT,
        "ITEM_ICON_POST_ELADARD_LEFT": ITEM_ICON_POST_ELADARD_LEFT,
    }
    lines = [
        "//! Generated native SF2 strategic-map sprite atlas.",
        "//!",
        f"//! Source: oracle PPU snapshot `{source_name}`.",
        "//! Regenerate with `tools/sf2/generate_map_sprites.py`.",
        "",
    ]
    for name, value in constants.items():
        rust_type = "usize" if name in {"WIDTH", "HEIGHT"} else "i32"
        lines.append(f"pub const {name}: {rust_type} = {value};")
    lines.extend(
        [
            f"const CHANNELS_PER_PIXEL: usize = {CHANNELS_PER_PIXEL};",
            "#[cfg(test)]",
            f"const SOURCE_RGBA_FNV1A: u32 = 0x{source_hash:08X};",
            f"const PALETTE: [[u8; CHANNELS_PER_PIXEL]; {len(palette)}] = [",
        ]
    )
    lines.extend(
        f"    [{red}, {green}, {blue}, {alpha}],"
        for red, green, blue, alpha in palette
    )
    lines.extend([
        "];",
        f"const RUNS: [(u8, u16); {len(runs)}] = [",
    ])
    for offset in range(0, len(runs), 8):
        lines.append(
            "    "
            + ", ".join(
                f"({length}, {palette_index})"
                for length, palette_index in runs[offset : offset + 8]
            )
            + ","
        )
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
            "    fn map_sprites_decode_to_the_certified_atlas() {",
            "        let rgba = decode_rgba();",
            f"        let hash = rgba.into_iter().fold({FNV_OFFSET_BASIS}, |value, byte| {{",
            f"            (value ^ u32::from(byte)).wrapping_mul({FNV_PRIME})",
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
    parser.add_argument("vram", type=Path)
    parser.add_argument("cgram", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--characters", type=lambda value: int(value, 0), required=True)
    parser.add_argument("--opening-capture", type=Path, required=True)
    parser.add_argument("--escalated-capture", type=Path, required=True)
    parser.add_argument("--opening-vram", type=Path, required=True)
    parser.add_argument("--opening-cgram", type=Path, required=True)
    parser.add_argument("--post-interception-capture", type=Path, required=True)
    parser.add_argument("--post-interception-vram", type=Path, required=True)
    parser.add_argument("--post-interception-cgram", type=Path, required=True)
    parser.add_argument("--post-fighter-intercept-capture", type=Path, required=True)
    parser.add_argument("--post-fighter-intercept-vram", type=Path, required=True)
    parser.add_argument("--post-fighter-intercept-cgram", type=Path, required=True)
    parser.add_argument("--post-pigma-capture", type=Path, required=True)
    parser.add_argument("--post-pigma-vram", type=Path, required=True)
    parser.add_argument("--post-pigma-cgram", type=Path, required=True)
    parser.add_argument("--post-eladard-capture", type=Path, required=True)
    parser.add_argument("--post-eladard-vram", type=Path, required=True)
    parser.add_argument("--post-eladard-cgram", type=Path, required=True)
    parser.add_argument("--post-eladard-backdrop", type=Path, required=True)
    args = parser.parse_args()
    vram = args.vram.read_bytes()
    cgram = args.cgram.read_bytes()
    opening_vram = args.opening_vram.read_bytes()
    opening_cgram = args.opening_cgram.read_bytes()
    post_interception_vram = args.post_interception_vram.read_bytes()
    post_interception_cgram = args.post_interception_cgram.read_bytes()
    post_fighter_intercept_vram = args.post_fighter_intercept_vram.read_bytes()
    post_fighter_intercept_cgram = args.post_fighter_intercept_cgram.read_bytes()
    post_pigma_vram = args.post_pigma_vram.read_bytes()
    post_pigma_cgram = args.post_pigma_cgram.read_bytes()
    post_eladard_vram = args.post_eladard_vram.read_bytes()
    post_eladard_cgram = args.post_eladard_cgram.read_bytes()
    if (
        len(vram) != 65_536
        or len(cgram) != 512
        or len(opening_vram) != 65_536
        or len(opening_cgram) != 512
        or len(post_interception_vram) != 65_536
        or len(post_interception_cgram) != 512
        or len(post_fighter_intercept_vram) != 65_536
        or len(post_fighter_intercept_cgram) != 512
        or len(post_pigma_vram) != 65_536
        or len(post_pigma_cgram) != 512
        or len(post_eladard_vram) != 65_536
        or len(post_eladard_cgram) != 512
    ):
        raise SystemExit("expected a complete 64 KiB VRAM and 512-byte CGRAM snapshot")
    pixels = render_atlas(
        vram,
        cgram,
        opening_vram,
        opening_cgram,
        args.characters,
        read_capture(args.opening_capture),
        read_capture(args.escalated_capture),
        post_interception_vram,
        post_interception_cgram,
        read_capture(args.post_interception_capture),
        post_fighter_intercept_vram,
        post_fighter_intercept_cgram,
        read_capture(args.post_fighter_intercept_capture),
        post_pigma_vram,
        post_pigma_cgram,
        read_capture(args.post_pigma_capture),
        post_eladard_vram,
        post_eladard_cgram,
        read_capture(args.post_eladard_capture),
        generated_backdrop_rgb(args.post_eladard_backdrop),
    )
    palette, runs = encode(pixels)
    args.output.write_text(
        rust_source(args.vram.name, palette, runs, fnv1a(pixels)),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
