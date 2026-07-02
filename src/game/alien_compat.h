// SNES alien offset compatibility layer.
// Maps 16-bit-era al_/alx_ byte offsets used by map/path bytecode
// onto the flat C Alien struct fields.

#ifndef STARFOX_GAME_ALIEN_COMPAT_H
#define STARFOX_GAME_ALIEN_COMPAT_H

#include "../types.h"
#include "obj.h"

// Generic SNES offset access.
// When is_alx is false, offset is relative to al_start (includes 4-byte list header).
// When is_alx is true, offset is relative to alx_start.
bool AlienCompat_Read8(const Alien *al, uint16 offset, bool is_alx, uint8 *out);
bool AlienCompat_Read16(const Alien *al, uint16 offset, bool is_alx, uint16 *out);
bool AlienCompat_Write8(Alien *al, uint16 offset, bool is_alx, uint8 value);
bool AlienCompat_Write16(Alien *al, uint16 offset, bool is_alx, uint16 value);
bool AlienCompat_Add8(Alien *al, uint16 offset, bool is_alx, int8 delta);
bool AlienCompat_Add16(Alien *al, uint16 offset, bool is_alx, int16 delta);

// Path bytecode variable offsets use PALVAROFFSET encoding:
// low 7 bits = offset, bit7 set = alx offset.
static inline bool AlienCompat_PathRead8(const Alien *al, uint8 encoded_offset, uint8 *out) {
    return AlienCompat_Read8(al,
                             (uint16)(encoded_offset & 0x7Fu),
                             (encoded_offset & 0x80u) != 0,
                             out);
}

static inline bool AlienCompat_PathRead16(const Alien *al, uint8 encoded_offset, uint16 *out) {
    return AlienCompat_Read16(al,
                              (uint16)(encoded_offset & 0x7Fu),
                              (encoded_offset & 0x80u) != 0,
                              out);
}

static inline bool AlienCompat_PathWrite8(Alien *al, uint8 encoded_offset, uint8 value) {
    return AlienCompat_Write8(al,
                              (uint16)(encoded_offset & 0x7Fu),
                              (encoded_offset & 0x80u) != 0,
                              value);
}

static inline bool AlienCompat_PathWrite16(Alien *al, uint8 encoded_offset, uint16 value) {
    return AlienCompat_Write16(al,
                               (uint16)(encoded_offset & 0x7Fu),
                               (encoded_offset & 0x80u) != 0,
                               value);
}

static inline bool AlienCompat_PathAdd8(Alien *al, uint8 encoded_offset, int8 delta) {
    return AlienCompat_Add8(al,
                            (uint16)(encoded_offset & 0x7Fu),
                            (encoded_offset & 0x80u) != 0,
                            delta);
}

static inline bool AlienCompat_PathAdd16(Alien *al, uint8 encoded_offset, int16 delta) {
    return AlienCompat_Add16(al,
                             (uint16)(encoded_offset & 0x7Fu),
                             (encoded_offset & 0x80u) != 0,
                             delta);
}

#endif // STARFOX_GAME_ALIEN_COMPAT_H
