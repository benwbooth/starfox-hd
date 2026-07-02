//! SNES alien byte-offset compatibility layer — game-core lane copy.
//!
//! C origin: `src/game/alien_compat.c/h`. Maps 16-bit-era `al_`/`alx_` byte
//! offsets used by map bytecode (setalvar*/setalxvar*/addalvarp*) onto the
//! flat [`Alien`] struct fields, preserving little-endian lo/hi semantics.
//! Offsets covering 24-bit SNES function pointers are denied exactly like
//! the C layer (`al_stratptr` 22..24, alx 6..17).
//!
//! TODO(consolidation): `sf-path/src/alien_compat.rs` is the same port over
//! the path-lane Alien copy; merge when the Alien type consolidates.

use crate::alien::Alien;

fn lo(w: u16) -> u8 {
    (w & 0xFF) as u8
}
fn hi(w: u16) -> u8 {
    (w >> 8) as u8
}
fn set_lo(w: &mut u16, v: u8) {
    *w = (*w & 0xFF00) | v as u16;
}
fn set_hi(w: &mut u16, v: u8) {
    *w = (*w & 0x00FF) | ((v as u16) << 8);
}
fn lo_i(w: i16) -> u8 {
    (w as u16 & 0xFF) as u8
}
fn hi_i(w: i16) -> u8 {
    ((w as u16) >> 8) as u8
}
fn set_lo_i(w: &mut i16, v: u8) {
    *w = ((*w as u16 & 0xFF00) | v as u16) as i16;
}
fn set_hi_i(w: &mut i16, v: u8) {
    *w = ((*w as u16 & 0x00FF) | ((v as u16) << 8)) as i16;
}

/// C `alien_base_byte_ptr` read half (src/game/alien_compat.c:6).
fn base_read(al: &Alien, offset: u16) -> Option<u8> {
    Some(match offset {
        4 => lo(al.shape),
        5 => hi(al.shape),
        6 => lo(al.ptr),
        7 => hi(al.ptr),
        8 => al.flags,
        9 => al.type_,
        10 => al.count,
        11 => al.count1,
        12 => lo_i(al.worldx),
        13 => hi_i(al.worldx),
        14 => lo_i(al.worldy),
        15 => hi_i(al.worldy),
        16 => lo_i(al.worldz),
        17 => hi_i(al.worldz),
        18 => al.rotx,
        19 => al.roty,
        20 => al.rotz,
        21 => al.vel,
        // al_stratptr (22..24) is a 24-bit function pointer: denied.
        25 => lo(al.immuneptr),
        26 => hi(al.immuneptr),
        27 => lo(al.collobjptr),
        28 => hi(al.collobjptr),
        29 => al.sflags,
        30 => al.sflags2,
        31 => al.sflags3,
        32 => al.sflags4,
        33 => al.skidy,
        34 => al.sbyte1,
        35 => al.sbyte2,
        36 => al.sbyte3,
        37 => al.sbyte4,
        38 => lo_i(al.sword1),
        39 => hi_i(al.sword1),
        40 => lo_i(al.sword2),
        41 => hi_i(al.sword2),
        42 => al.hp,
        43 => al.ap,
        _ => return None,
    })
}

/// C `alien_base_byte_ptr` write half.
fn base_write(al: &mut Alien, offset: u16, v: u8) -> bool {
    match offset {
        4 => set_lo(&mut al.shape, v),
        5 => set_hi(&mut al.shape, v),
        6 => set_lo(&mut al.ptr, v),
        7 => set_hi(&mut al.ptr, v),
        8 => al.flags = v,
        9 => al.type_ = v,
        10 => al.count = v,
        11 => al.count1 = v,
        12 => set_lo_i(&mut al.worldx, v),
        13 => set_hi_i(&mut al.worldx, v),
        14 => set_lo_i(&mut al.worldy, v),
        15 => set_hi_i(&mut al.worldy, v),
        16 => set_lo_i(&mut al.worldz, v),
        17 => set_hi_i(&mut al.worldz, v),
        18 => al.rotx = v,
        19 => al.roty = v,
        20 => al.rotz = v,
        21 => al.vel = v,
        25 => set_lo(&mut al.immuneptr, v),
        26 => set_hi(&mut al.immuneptr, v),
        27 => set_lo(&mut al.collobjptr, v),
        28 => set_hi(&mut al.collobjptr, v),
        29 => al.sflags = v,
        30 => al.sflags2 = v,
        31 => al.sflags3 = v,
        32 => al.sflags4 = v,
        33 => al.skidy = v,
        34 => al.sbyte1 = v,
        35 => al.sbyte2 = v,
        36 => al.sbyte3 = v,
        37 => al.sbyte4 = v,
        38 => set_lo_i(&mut al.sword1, v),
        39 => set_hi_i(&mut al.sword1, v),
        40 => set_lo_i(&mut al.sword2, v),
        41 => set_hi_i(&mut al.sword2, v),
        42 => al.hp = v,
        43 => al.ap = v,
        _ => return false,
    }
    true
}

/// C `alien_alx_byte_ptr` read half (src/game/alien_compat.c:52).
fn alx_read(al: &Alien, offset: u16) -> Option<u8> {
    Some(match offset {
        0 => lo_i(al.swpx1),
        1 => hi_i(al.swpx1),
        2 => lo_i(al.swpy1),
        3 => hi_i(al.swpy1),
        4 => lo_i(al.swpz1),
        5 => hi_i(al.swpz1),
        // alx strategy pointers (6..17) are 24-bit fn pointers: denied.
        18 => al.stratstate,
        19 => lo(al.fireobjptr),
        20 => hi(al.fireobjptr),
        21 => lo_i(al.depthoffset),
        22 => hi_i(al.depthoffset),
        23 => al.relposx,
        24 => al.relposy,
        25 => al.relposz,
        26 => lo(al.debrisshape),
        27 => hi(al.debrisshape),
        28 => al.colframe,
        29 => al.animframe,
        30 => al.snd1,
        31 => al.snd2,
        32 => lo(al.coltab),
        33 => hi(al.coltab),
        34 => al.childx,
        35 => al.childy,
        36 => al.childz,
        37 => al.childrotx,
        38 => al.childroty,
        39 => al.childrotz,
        40 => lo(al.childrotobj),
        41 => hi(al.childrotobj),
        42 => al.tx,
        43 => al.ty,
        44 => lo(al.memptr),
        45 => hi(al.memptr),
        46 => lo(al.stackptr),
        47 => hi(al.stackptr),
        48 => lo(al.stratmem),
        49 => hi(al.stratmem),
        50 => al.pbyte1,
        51 => al.pbyte2,
        52 => lo(al.pword1),
        53 => hi(al.pword1),
        _ => return None,
    })
}

/// C `alien_alx_byte_ptr` write half.
fn alx_write(al: &mut Alien, offset: u16, v: u8) -> bool {
    match offset {
        0 => set_lo_i(&mut al.swpx1, v),
        1 => set_hi_i(&mut al.swpx1, v),
        2 => set_lo_i(&mut al.swpy1, v),
        3 => set_hi_i(&mut al.swpy1, v),
        4 => set_lo_i(&mut al.swpz1, v),
        5 => set_hi_i(&mut al.swpz1, v),
        18 => al.stratstate = v,
        19 => set_lo(&mut al.fireobjptr, v),
        20 => set_hi(&mut al.fireobjptr, v),
        21 => set_lo_i(&mut al.depthoffset, v),
        22 => set_hi_i(&mut al.depthoffset, v),
        23 => al.relposx = v,
        24 => al.relposy = v,
        25 => al.relposz = v,
        26 => set_lo(&mut al.debrisshape, v),
        27 => set_hi(&mut al.debrisshape, v),
        28 => al.colframe = v,
        29 => al.animframe = v,
        30 => al.snd1 = v,
        31 => al.snd2 = v,
        32 => set_lo(&mut al.coltab, v),
        33 => set_hi(&mut al.coltab, v),
        34 => al.childx = v,
        35 => al.childy = v,
        36 => al.childz = v,
        37 => al.childrotx = v,
        38 => al.childroty = v,
        39 => al.childrotz = v,
        40 => set_lo(&mut al.childrotobj, v),
        41 => set_hi(&mut al.childrotobj, v),
        42 => al.tx = v,
        43 => al.ty = v,
        44 => set_lo(&mut al.memptr, v),
        45 => set_hi(&mut al.memptr, v),
        46 => set_lo(&mut al.stackptr, v),
        47 => set_hi(&mut al.stackptr, v),
        48 => set_lo(&mut al.stratmem, v),
        49 => set_hi(&mut al.stratmem, v),
        50 => al.pbyte1 = v,
        51 => al.pbyte2 = v,
        52 => set_lo(&mut al.pword1, v),
        53 => set_hi(&mut al.pword1, v),
        _ => return false,
    }
    true
}

/// C `AlienCompat_Read8` (src/game/alien_compat.c:108).
pub fn read8(al: &Alien, offset: u16, is_alx: bool) -> Option<u8> {
    if is_alx {
        alx_read(al, offset)
    } else {
        base_read(al, offset)
    }
}

/// C `AlienCompat_Read16` (src/game/alien_compat.c:116).
pub fn read16(al: &Alien, offset: u16, is_alx: bool) -> Option<u16> {
    let lo = read8(al, offset, is_alx)?;
    let hi = read8(al, offset.wrapping_add(1), is_alx)?;
    Some(lo as u16 | ((hi as u16) << 8))
}

/// C `AlienCompat_Write8` (src/game/alien_compat.c:126).
pub fn write8(al: &mut Alien, offset: u16, is_alx: bool, value: u8) -> bool {
    if is_alx {
        alx_write(al, offset, value)
    } else {
        base_write(al, offset, value)
    }
}

/// C `AlienCompat_Write16` (src/game/alien_compat.c:134) — both byte writes
/// must land, and the low byte is committed first, exactly like C.
pub fn write16(al: &mut Alien, offset: u16, is_alx: bool, value: u16) -> bool {
    if !write8(al, offset, is_alx, (value & 0xFF) as u8) {
        return false;
    }
    write8(al, offset.wrapping_add(1), is_alx, (value >> 8) as u8)
}

/// C `AlienCompat_Add8` (src/game/alien_compat.c:141).
pub fn add8(al: &mut Alien, offset: u16, is_alx: bool, delta: i8) -> bool {
    let cur = match read8(al, offset, is_alx) {
        Some(v) => v,
        None => return false,
    };
    write8(al, offset, is_alx, cur.wrapping_add(delta as u8))
}

/// C `AlienCompat_Add16` (src/game/alien_compat.c:148).
pub fn add16(al: &mut Alien, offset: u16, is_alx: bool, delta: i16) -> bool {
    let cur = match read16(al, offset, is_alx) {
        Some(v) => v,
        None => return false,
    };
    write16(al, offset, is_alx, (cur as i16).wrapping_add(delta) as u16)
}
