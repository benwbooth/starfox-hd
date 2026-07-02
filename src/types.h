#ifndef STARFOX_TYPES_H
#define STARFOX_TYPES_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

// Basic types matching SNES register widths
typedef uint8_t  uint8;
typedef uint16_t uint16;
typedef uint32_t uint32;
typedef int8_t   int8;
typedef int16_t  int16;
typedef int32_t  int32;
typedef uint64_t uint64;
typedef int64_t  int64;

// SNES-style aliases
typedef uint8  BYTE;
typedef uint16 WORD;
typedef uint32 LONG;

// Fixed-point types
// 8.8 format: upper byte = integer, lower byte = fraction
typedef uint16 fixed8_8;
typedef int16  sfixed8_8;

// 16.16 format: upper word = integer, lower word = fraction
typedef uint32 fixed16_16;
typedef int32  sfixed16_16;

// Fixed-point conversion macros (8.8)
#define FP8_MAKE(i, f)     ((fixed8_8)(((uint16)(i) << 8) | ((uint8)(f))))
#define FP8_INT(x)         ((uint8)((x) >> 8))
#define FP8_FRAC(x)        ((uint8)((x) & 0xFF))
#define FP8_FROM_INT(i)    ((fixed8_8)((uint16)(i) << 8))
#define FP8_MUL(a, b)      ((fixed8_8)(((uint32)(a) * (uint32)(b)) >> 8))
#define FP8_DIV(a, b)      ((fixed8_8)(((uint32)(a) << 8) / (uint32)(b)))

// Fixed-point conversion macros (16.16)
#define FP16_MAKE(i, f)    ((fixed16_16)(((uint32)(i) << 16) | ((uint16)(f))))
#define FP16_INT(x)        ((uint16)((x) >> 16))
#define FP16_FRAC(x)       ((uint16)((x) & 0xFFFF))
#define FP16_FROM_INT(i)   ((fixed16_16)((uint32)(i) << 16))
#define FP16_MUL(a, b)     ((fixed16_16)(((int64_t)(a) * (int64_t)(b)) >> 16))
#define FP16_DIV(a, b)     ((fixed16_16)(((int64_t)(a) << 16) / (int64_t)(b)))

// Angle system: 0-255 maps to 0-360 degrees (SNES convention)
typedef uint8 angle8;
typedef uint16 angle16;

// Convert SNES angle (0-255) to radians
#define ANGLE8_TO_RAD(a)   ((float)(a) * (3.14159265f / 128.0f))
#define ANGLE16_TO_RAD(a)  ((float)(a) * (3.14159265f / 32768.0f))

// Star Fox PRNG (must match SNES exactly)
#define PRNG_NEXT(rnd) (((rnd) * 91 + 0x61D7) & 0xFFFF)

// Object system constants
#define MAX_OBJECTS      128
#define MAX_DRAW_LIST    128
#define MAX_SHAPE_VERTS  256

// SNES memory sizes
#define WRAM_SIZE        0x20000  // 128KB main RAM
#define SRAM_SIZE        0x10000  // 64KB (Super FX shared)
#define VRAM_SIZE        0x10000  // 64KB video RAM

// Clamp helper
#define CLAMP(x, lo, hi) ((x) < (lo) ? (lo) : ((x) > (hi) ? (hi) : (x)))
#define MIN(a, b)        ((a) < (b) ? (a) : (b))
#define MAX(a, b)        ((a) > (b) ? (a) : (b))
#define ARRAY_SIZE(a)    (sizeof(a) / sizeof((a)[0]))

// Draw list entry — the bridge between game logic and renderer
// Decompiled from STRUCTS.INC dl_ structure (30 bytes on SNES).
// On SNES this lived in shared SRAM at $700000 for the Super FX to read.
// We use wider types (int32 for positions) since the OpenGL renderer needs them.
typedef struct {
    int32  x, y, z;           // World position (fixed-point 16.16)
    int16  rx, ry, rz;        // Rotation angles (SNES 0-255 units, stored as int16)
    uint16 shape_id;           // Shape index (dl_shape)
    uint16 color_table;        // Color table pointer (dl_coltab)
    int16  sort_z;             // Sort depth — ground objects get 15000 (dl_sortz)
    uint8  sflags;             // Strategy flags copied from alien (dl_sflags)
    uint8  explosion_cnt;      // Explosion frame count, 0=normal (dl_expcnt)
    uint8  anim_frame;         // Animation frame 0-127 (dl_animframe)
    uint8  col_frame;          // Color animation frame 0-127 (dl_colframe)
    uint8  depth_offset;       // Depth layering offset (dl_depth)
    uint8  flags;              // Visibility/effect flags
    // Shadow/particle data (dl_shadx/y/z — used for particle objects)
    int16  shad_x, shad_y, shad_z;
    uint8  tscroll_x, tscroll_y; // Texture scroll coordinates (dl_tscrollx/y)
    uint16 obj_id;               // Stable source-object id (alien index + 1);
                                 // used to pair entries across frames for
                                 // render interpolation. 0 = no identity.
} DrawListEntry;

// Draw list flags
#define DL_FLAG_VISIBLE    0x01
#define DL_FLAG_SHADOW     0x02
#define DL_FLAG_HIGHLIGHT  0x04
#define DL_FLAG_WIREFRAME  0x08

#endif // STARFOX_TYPES_H
