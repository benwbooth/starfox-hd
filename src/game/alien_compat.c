// SNES alien offset compatibility layer.
// See alien_compat.h for details.

#include "alien_compat.h"

static bool alien_base_byte_ptr(Alien *al, uint16 offset, uint8 **out) {
    switch (offset) {
    case 4:  *out = ((uint8 *)&al->shape) + 0; return true;
    case 5:  *out = ((uint8 *)&al->shape) + 1; return true;
    case 6:  *out = ((uint8 *)&al->ptr) + 0; return true;
    case 7:  *out = ((uint8 *)&al->ptr) + 1; return true;
    case 8:  *out = &al->flags; return true;
    case 9:  *out = &al->type; return true;
    case 10: *out = &al->count; return true;
    case 11: *out = &al->count1; return true;
    case 12: *out = ((uint8 *)&al->worldx) + 0; return true;
    case 13: *out = ((uint8 *)&al->worldx) + 1; return true;
    case 14: *out = ((uint8 *)&al->worldy) + 0; return true;
    case 15: *out = ((uint8 *)&al->worldy) + 1; return true;
    case 16: *out = ((uint8 *)&al->worldz) + 0; return true;
    case 17: *out = ((uint8 *)&al->worldz) + 1; return true;
    case 18: *out = &al->rotx; return true;
    case 19: *out = &al->roty; return true;
    case 20: *out = &al->rotz; return true;
    case 21: *out = &al->vel; return true;
    // al_stratptr (22..24 on SNES) is a 24-bit function pointer.
    // Flat-memory C function pointers are not byte-compatible; deny direct writes.
    case 25: *out = ((uint8 *)&al->immuneptr) + 0; return true;
    case 26: *out = ((uint8 *)&al->immuneptr) + 1; return true;
    case 27: *out = ((uint8 *)&al->collobjptr) + 0; return true;
    case 28: *out = ((uint8 *)&al->collobjptr) + 1; return true;
    case 29: *out = &al->sflags; return true;
    case 30: *out = &al->sflags2; return true;
    case 31: *out = &al->sflags3; return true;
    case 32: *out = &al->sflags4; return true;
    case 33: *out = &al->skidy; return true;
    case 34: *out = &al->sbyte1; return true;
    case 35: *out = &al->sbyte2; return true;
    case 36: *out = &al->sbyte3; return true;
    case 37: *out = &al->sbyte4; return true;
    case 38: *out = ((uint8 *)&al->sword1) + 0; return true;
    case 39: *out = ((uint8 *)&al->sword1) + 1; return true;
    case 40: *out = ((uint8 *)&al->sword2) + 0; return true;
    case 41: *out = ((uint8 *)&al->sword2) + 1; return true;
    case 42: *out = &al->HP; return true;
    case 43: *out = &al->AP; return true;
    default:
        return false;
    }
}

static bool alien_alx_byte_ptr(Alien *al, uint16 offset, uint8 **out) {
    switch (offset) {
    case 0:  *out = ((uint8 *)&al->sWPx1) + 0; return true;
    case 1:  *out = ((uint8 *)&al->sWPx1) + 1; return true;
    case 2:  *out = ((uint8 *)&al->sWPy1) + 0; return true;
    case 3:  *out = ((uint8 *)&al->sWPy1) + 1; return true;
    case 4:  *out = ((uint8 *)&al->sWPz1) + 0; return true;
    case 5:  *out = ((uint8 *)&al->sWPz1) + 1; return true;
    // alx strategy pointers (6..17 on SNES) are 24-bit function pointers.
    // Flat-memory C function pointers are not byte-compatible; deny direct access.
    case 18: *out = &al->stratstate; return true;
    case 19: *out = ((uint8 *)&al->fireobjptr) + 0; return true;
    case 20: *out = ((uint8 *)&al->fireobjptr) + 1; return true;
    case 21: *out = ((uint8 *)&al->depthoffset) + 0; return true;
    case 22: *out = ((uint8 *)&al->depthoffset) + 1; return true;
    case 23: *out = &al->relposx; return true;
    case 24: *out = &al->relposy; return true;
    case 25: *out = &al->relposz; return true;
    case 26: *out = ((uint8 *)&al->debrisshape) + 0; return true;
    case 27: *out = ((uint8 *)&al->debrisshape) + 1; return true;
    case 28: *out = &al->colframe; return true;
    case 29: *out = &al->animframe; return true;
    case 30: *out = &al->snd1; return true;
    case 31: *out = &al->snd2; return true;
    case 32: *out = ((uint8 *)&al->coltab) + 0; return true;
    case 33: *out = ((uint8 *)&al->coltab) + 1; return true;
    case 34: *out = &al->childx; return true;
    case 35: *out = &al->childy; return true;
    case 36: *out = &al->childz; return true;
    case 37: *out = &al->childrotx; return true;
    case 38: *out = &al->childroty; return true;
    case 39: *out = &al->childrotz; return true;
    case 40: *out = ((uint8 *)&al->childrotobj) + 0; return true;
    case 41: *out = ((uint8 *)&al->childrotobj) + 1; return true;
    case 42: *out = &al->tx; return true;
    case 43: *out = &al->ty; return true;
    case 44: *out = ((uint8 *)&al->memptr) + 0; return true;
    case 45: *out = ((uint8 *)&al->memptr) + 1; return true;
    case 46: *out = ((uint8 *)&al->stackptr) + 0; return true;
    case 47: *out = ((uint8 *)&al->stackptr) + 1; return true;
    case 48: *out = ((uint8 *)&al->stratmem) + 0; return true;
    case 49: *out = ((uint8 *)&al->stratmem) + 1; return true;
    case 50: *out = &al->pbyte1; return true;
    case 51: *out = &al->pbyte2; return true;
    case 52: *out = ((uint8 *)&al->pword1) + 0; return true;
    case 53: *out = ((uint8 *)&al->pword1) + 1; return true;
    default:
        return false;
    }
}

static bool alien_byte_ptr(Alien *al, uint16 offset, bool is_alx, uint8 **out) {
    return is_alx ? alien_alx_byte_ptr(al, offset, out)
                  : alien_base_byte_ptr(al, offset, out);
}

bool AlienCompat_Read8(const Alien *al, uint16 offset, bool is_alx, uint8 *out) {
    if (!al || !out) return false;
    uint8 *ptr = NULL;
    if (!alien_byte_ptr((Alien *)al, offset, is_alx, &ptr) || !ptr) return false;
    *out = *ptr;
    return true;
}

bool AlienCompat_Read16(const Alien *al, uint16 offset, bool is_alx, uint16 *out) {
    if (!al || !out) return false;
    uint8 lo = 0;
    uint8 hi = 0;
    if (!AlienCompat_Read8(al, offset, is_alx, &lo)) return false;
    if (!AlienCompat_Read8(al, (uint16)(offset + 1), is_alx, &hi)) return false;
    *out = (uint16)lo | ((uint16)hi << 8);
    return true;
}

bool AlienCompat_Write8(Alien *al, uint16 offset, bool is_alx, uint8 value) {
    if (!al) return false;
    uint8 *ptr = NULL;
    if (!alien_byte_ptr(al, offset, is_alx, &ptr) || !ptr) return false;
    *ptr = value;
    return true;
}

bool AlienCompat_Write16(Alien *al, uint16 offset, bool is_alx, uint16 value) {
    if (!al) return false;
    if (!AlienCompat_Write8(al, offset, is_alx, (uint8)(value & 0xFFu))) return false;
    if (!AlienCompat_Write8(al, (uint16)(offset + 1), is_alx, (uint8)(value >> 8))) return false;
    return true;
}

bool AlienCompat_Add8(Alien *al, uint16 offset, bool is_alx, int8 delta) {
    uint8 cur = 0;
    if (!AlienCompat_Read8(al, offset, is_alx, &cur)) return false;
    cur = (uint8)(cur + delta);
    return AlienCompat_Write8(al, offset, is_alx, cur);
}

bool AlienCompat_Add16(Alien *al, uint16 offset, bool is_alx, int16 delta) {
    uint16 cur = 0;
    if (!AlienCompat_Read16(al, offset, is_alx, &cur)) return false;
    cur = (uint16)((int16)cur + delta);
    return AlienCompat_Write16(al, offset, is_alx, cur);
}
