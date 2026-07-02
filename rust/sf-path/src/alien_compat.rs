//! SNES alien offset compatibility layer.
//!
//! C origin: `src/game/alien_compat.c` / `alien_compat.h`. Maps 16-bit-era
//! `al_`/`alx_` byte offsets used by map/path bytecode onto the flat [`Alien`]
//! struct fields.
//!
//! When `is_alx` is false the offset is relative to `al_start` (includes the
//! 4-byte list header, so real fields start at offset 4). When `is_alx` is
//! true the offset is relative to `alx_start`.
//!
//! The C code hands out raw byte pointers into the struct
//! (`alien_base_byte_ptr` / `alien_alx_byte_ptr`); here each byte slot is a
//! get/set pair over the typed fields, preserving little-endian lo/hi
//! semantics for the 16-bit fields. Offsets covering 24-bit SNES function
//! pointers are denied exactly like the C layer (`al_stratptr` 22..24,
//! alx 6..17).

use crate::alien::Alien;

/// Read one byte of an `al_start`-relative slot (C `alien_base_byte_ptr` read).
fn base_read(al: &Alien, offset: u16) -> Option<u8> {
    Some(match offset {
        4 => al.shape.to_le_bytes()[0],
        5 => al.shape.to_le_bytes()[1],
        6 => al.ptr.to_le_bytes()[0],
        7 => al.ptr.to_le_bytes()[1],
        8 => al.flags,
        9 => al.type_,
        10 => al.count,
        11 => al.count1,
        12 => al.worldx.to_le_bytes()[0],
        13 => al.worldx.to_le_bytes()[1],
        14 => al.worldy.to_le_bytes()[0],
        15 => al.worldy.to_le_bytes()[1],
        16 => al.worldz.to_le_bytes()[0],
        17 => al.worldz.to_le_bytes()[1],
        18 => al.rotx,
        19 => al.roty,
        20 => al.rotz,
        21 => al.vel,
        // al_stratptr (22..24 on SNES) is a 24-bit function pointer; denied.
        25 => al.immuneptr.to_le_bytes()[0],
        26 => al.immuneptr.to_le_bytes()[1],
        27 => al.collobjptr.to_le_bytes()[0],
        28 => al.collobjptr.to_le_bytes()[1],
        29 => al.sflags,
        30 => al.sflags2,
        31 => al.sflags3,
        32 => al.sflags4,
        33 => al.skidy,
        34 => al.sbyte1,
        35 => al.sbyte2,
        36 => al.sbyte3,
        37 => al.sbyte4,
        38 => al.sword1.to_le_bytes()[0],
        39 => al.sword1.to_le_bytes()[1],
        40 => al.sword2.to_le_bytes()[0],
        41 => al.sword2.to_le_bytes()[1],
        42 => al.hp,
        43 => al.ap,
        _ => return None,
    })
}

/// Write one byte of an `al_start`-relative slot (C `alien_base_byte_ptr` write).
fn base_write(al: &mut Alien, offset: u16, value: u8) -> bool {
    fn set_lo(word: &mut u16, v: u8) {
        *word = (*word & 0xFF00) | v as u16;
    }
    fn set_hi(word: &mut u16, v: u8) {
        *word = (*word & 0x00FF) | ((v as u16) << 8);
    }
    fn set_lo_i(word: &mut i16, v: u8) {
        *word = (*word as u16 & 0xFF00 | v as u16) as i16;
    }
    fn set_hi_i(word: &mut i16, v: u8) {
        *word = (*word as u16 & 0x00FF | ((v as u16) << 8)) as i16;
    }
    match offset {
        4 => set_lo(&mut al.shape, value),
        5 => set_hi(&mut al.shape, value),
        6 => set_lo(&mut al.ptr, value),
        7 => set_hi(&mut al.ptr, value),
        8 => al.flags = value,
        9 => al.type_ = value,
        10 => al.count = value,
        11 => al.count1 = value,
        12 => set_lo_i(&mut al.worldx, value),
        13 => set_hi_i(&mut al.worldx, value),
        14 => set_lo_i(&mut al.worldy, value),
        15 => set_hi_i(&mut al.worldy, value),
        16 => set_lo_i(&mut al.worldz, value),
        17 => set_hi_i(&mut al.worldz, value),
        18 => al.rotx = value,
        19 => al.roty = value,
        20 => al.rotz = value,
        21 => al.vel = value,
        // al_stratptr (22..24) denied — see module docs.
        25 => set_lo(&mut al.immuneptr, value),
        26 => set_hi(&mut al.immuneptr, value),
        27 => set_lo(&mut al.collobjptr, value),
        28 => set_hi(&mut al.collobjptr, value),
        29 => al.sflags = value,
        30 => al.sflags2 = value,
        31 => al.sflags3 = value,
        32 => al.sflags4 = value,
        33 => al.skidy = value,
        34 => al.sbyte1 = value,
        35 => al.sbyte2 = value,
        36 => al.sbyte3 = value,
        37 => al.sbyte4 = value,
        38 => set_lo_i(&mut al.sword1, value),
        39 => set_hi_i(&mut al.sword1, value),
        40 => set_lo_i(&mut al.sword2, value),
        41 => set_hi_i(&mut al.sword2, value),
        42 => al.hp = value,
        43 => al.ap = value,
        _ => return false,
    }
    true
}

/// Read one byte of an `alx_start`-relative slot (C `alien_alx_byte_ptr` read).
fn alx_read(al: &Alien, offset: u16) -> Option<u8> {
    Some(match offset {
        0 => al.swpx1.to_le_bytes()[0],
        1 => al.swpx1.to_le_bytes()[1],
        2 => al.swpy1.to_le_bytes()[0],
        3 => al.swpy1.to_le_bytes()[1],
        4 => al.swpz1.to_le_bytes()[0],
        5 => al.swpz1.to_le_bytes()[1],
        // alx strategy pointers (6..17 on SNES) are 24-bit function pointers; denied.
        18 => al.stratstate,
        19 => al.fireobjptr.to_le_bytes()[0],
        20 => al.fireobjptr.to_le_bytes()[1],
        21 => al.depthoffset.to_le_bytes()[0],
        22 => al.depthoffset.to_le_bytes()[1],
        23 => al.relposx,
        24 => al.relposy,
        25 => al.relposz,
        26 => al.debrisshape.to_le_bytes()[0],
        27 => al.debrisshape.to_le_bytes()[1],
        28 => al.colframe,
        29 => al.animframe,
        30 => al.snd1,
        31 => al.snd2,
        32 => al.coltab.to_le_bytes()[0],
        33 => al.coltab.to_le_bytes()[1],
        34 => al.childx,
        35 => al.childy,
        36 => al.childz,
        37 => al.childrotx,
        38 => al.childroty,
        39 => al.childrotz,
        40 => al.childrotobj.to_le_bytes()[0],
        41 => al.childrotobj.to_le_bytes()[1],
        42 => al.tx,
        43 => al.ty,
        44 => al.memptr.to_le_bytes()[0],
        45 => al.memptr.to_le_bytes()[1],
        46 => al.stackptr.to_le_bytes()[0],
        47 => al.stackptr.to_le_bytes()[1],
        48 => al.stratmem.to_le_bytes()[0],
        49 => al.stratmem.to_le_bytes()[1],
        50 => al.pbyte1,
        51 => al.pbyte2,
        52 => al.pword1.to_le_bytes()[0],
        53 => al.pword1.to_le_bytes()[1],
        _ => return None,
    })
}

/// Write one byte of an `alx_start`-relative slot (C `alien_alx_byte_ptr` write).
fn alx_write(al: &mut Alien, offset: u16, value: u8) -> bool {
    fn set_lo(word: &mut u16, v: u8) {
        *word = (*word & 0xFF00) | v as u16;
    }
    fn set_hi(word: &mut u16, v: u8) {
        *word = (*word & 0x00FF) | ((v as u16) << 8);
    }
    fn set_lo_i(word: &mut i16, v: u8) {
        *word = (*word as u16 & 0xFF00 | v as u16) as i16;
    }
    fn set_hi_i(word: &mut i16, v: u8) {
        *word = (*word as u16 & 0x00FF | ((v as u16) << 8)) as i16;
    }
    match offset {
        0 => set_lo_i(&mut al.swpx1, value),
        1 => set_hi_i(&mut al.swpx1, value),
        2 => set_lo_i(&mut al.swpy1, value),
        3 => set_hi_i(&mut al.swpy1, value),
        4 => set_lo_i(&mut al.swpz1, value),
        5 => set_hi_i(&mut al.swpz1, value),
        // alx strategy pointers (6..17) denied — see module docs.
        18 => al.stratstate = value,
        19 => set_lo(&mut al.fireobjptr, value),
        20 => set_hi(&mut al.fireobjptr, value),
        21 => set_lo_i(&mut al.depthoffset, value),
        22 => set_hi_i(&mut al.depthoffset, value),
        23 => al.relposx = value,
        24 => al.relposy = value,
        25 => al.relposz = value,
        26 => set_lo(&mut al.debrisshape, value),
        27 => set_hi(&mut al.debrisshape, value),
        28 => al.colframe = value,
        29 => al.animframe = value,
        30 => al.snd1 = value,
        31 => al.snd2 = value,
        32 => set_lo(&mut al.coltab, value),
        33 => set_hi(&mut al.coltab, value),
        34 => al.childx = value,
        35 => al.childy = value,
        36 => al.childz = value,
        37 => al.childrotx = value,
        38 => al.childroty = value,
        39 => al.childrotz = value,
        40 => set_lo(&mut al.childrotobj, value),
        41 => set_hi(&mut al.childrotobj, value),
        42 => al.tx = value,
        43 => al.ty = value,
        44 => set_lo(&mut al.memptr, value),
        45 => set_hi(&mut al.memptr, value),
        46 => set_lo(&mut al.stackptr, value),
        47 => set_hi(&mut al.stackptr, value),
        48 => set_lo(&mut al.stratmem, value),
        49 => set_hi(&mut al.stratmem, value),
        50 => al.pbyte1 = value,
        51 => al.pbyte2 = value,
        52 => set_lo(&mut al.pword1, value),
        53 => set_hi(&mut al.pword1, value),
        _ => return false,
    }
    true
}

/// C `AlienCompat_Read8` (src/game/alien_compat.c).
pub fn read8(al: &Alien, offset: u16, is_alx: bool) -> Option<u8> {
    if is_alx {
        alx_read(al, offset)
    } else {
        base_read(al, offset)
    }
}

/// C `AlienCompat_Read16` (src/game/alien_compat.c): two 8-bit reads, LE.
pub fn read16(al: &Alien, offset: u16, is_alx: bool) -> Option<u16> {
    let lo = read8(al, offset, is_alx)?;
    let hi = read8(al, offset.wrapping_add(1), is_alx)?;
    Some(u16::from(lo) | (u16::from(hi) << 8))
}

/// C `AlienCompat_Write8` (src/game/alien_compat.c).
pub fn write8(al: &mut Alien, offset: u16, is_alx: bool, value: u8) -> bool {
    if is_alx {
        alx_write(al, offset, value)
    } else {
        base_write(al, offset, value)
    }
}

/// C `AlienCompat_Write16` (src/game/alien_compat.c): two 8-bit writes, LE.
/// Like C, the low byte is committed even if the high byte slot is invalid.
pub fn write16(al: &mut Alien, offset: u16, is_alx: bool, value: u16) -> bool {
    if !write8(al, offset, is_alx, (value & 0xFF) as u8) {
        return false;
    }
    write8(al, offset.wrapping_add(1), is_alx, (value >> 8) as u8)
}

/// C `AlienCompat_Add8` — 8-bit wrapping add in place.
pub fn add8(al: &mut Alien, offset: u16, is_alx: bool, delta: i8) -> bool {
    let cur = match read8(al, offset, is_alx) {
        Some(v) => v,
        None => return false,
    };
    write8(al, offset, is_alx, cur.wrapping_add(delta as u8))
}

/// C `AlienCompat_Add16` — 16-bit wrapping add in place.
pub fn add16(al: &mut Alien, offset: u16, is_alx: bool, delta: i16) -> bool {
    let cur = match read16(al, offset, is_alx) {
        Some(v) => v,
        None => return false,
    };
    write16(al, offset, is_alx, cur.wrapping_add(delta as u16))
}

// Path bytecode variable offsets use PALVAROFFSET encoding:
// low 7 bits = offset, bit7 set = alx offset (alien_compat.h inline helpers).

/// C `AlienCompat_PathRead8` (src/game/alien_compat.h).
pub fn path_read8(al: &Alien, encoded_offset: u8) -> Option<u8> {
    read8(al, u16::from(encoded_offset & 0x7F), encoded_offset & 0x80 != 0)
}

/// C `AlienCompat_PathRead16` (src/game/alien_compat.h).
pub fn path_read16(al: &Alien, encoded_offset: u8) -> Option<u16> {
    read16(al, u16::from(encoded_offset & 0x7F), encoded_offset & 0x80 != 0)
}

/// C `AlienCompat_PathWrite8` (src/game/alien_compat.h).
pub fn path_write8(al: &mut Alien, encoded_offset: u8, value: u8) -> bool {
    write8(al, u16::from(encoded_offset & 0x7F), encoded_offset & 0x80 != 0, value)
}

/// C `AlienCompat_PathWrite16` (src/game/alien_compat.h).
pub fn path_write16(al: &mut Alien, encoded_offset: u8, value: u16) -> bool {
    write16(al, u16::from(encoded_offset & 0x7F), encoded_offset & 0x80 != 0, value)
}

/// C `AlienCompat_PathAdd8` (src/game/alien_compat.h).
pub fn path_add8(al: &mut Alien, encoded_offset: u8, delta: i8) -> bool {
    add8(al, u16::from(encoded_offset & 0x7F), encoded_offset & 0x80 != 0, delta)
}

/// C `AlienCompat_PathAdd16` (src/game/alien_compat.h).
pub fn path_add16(al: &mut Alien, encoded_offset: u8, delta: i16) -> bool {
    add16(al, u16::from(encoded_offset & 0x7F), encoded_offset & 0x80 != 0, delta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alien::Alien;

    #[test]
    fn word_fields_are_little_endian() {
        let mut al = Alien::default();
        assert!(write16(&mut al, 12, false, 0xBEEF));
        assert_eq!(al.worldx as u16, 0xBEEF);
        assert_eq!(read8(&al, 12, false), Some(0xEF));
        assert_eq!(read8(&al, 13, false), Some(0xBE));
    }

    #[test]
    fn function_pointer_offsets_denied() {
        let al = Alien::default();
        for off in [0u16, 1, 2, 3, 22, 23, 24] {
            assert_eq!(read8(&al, off, false), None, "base offset {off}");
        }
        for off in 6u16..=17 {
            assert_eq!(read8(&al, off, true), None, "alx offset {off}");
        }
    }

    #[test]
    fn path_encoding_selects_alx() {
        let mut al = Alien::default();
        // PAL_PBYTE1 = 0x80 | 50
        assert!(path_write8(&mut al, 0x80 | 50, 7));
        assert_eq!(al.pbyte1, 7);
        // base offset 42 = HP
        assert!(path_write8(&mut al, 42, 9));
        assert_eq!(al.hp, 9);
    }
}
