//! Core shared types for the Star Fox HD Rust port.
//!
//! Rust rewrite of the C reference implementation in `../src/`. The C build
//! is the behavioral oracle: every module here mirrors a C translation unit
//! and must match its per-tick observable state (see `sf-difftest`).
//!
//! C source mapping:
//! - `src/types.h`  -> [`DrawListEntry`], limits
//! - `src/sf_rtl.h` -> [`pad`]

pub mod pad {
    //! SNES joypad bits, matching `src/sf_rtl.h` PAD_*.
    pub const B: u16 = 1 << 15;
    pub const Y: u16 = 1 << 14;
    pub const SELECT: u16 = 1 << 13;
    pub const START: u16 = 1 << 12;
    pub const UP: u16 = 1 << 11;
    pub const DOWN: u16 = 1 << 10;
    pub const LEFT: u16 = 1 << 9;
    pub const RIGHT: u16 = 1 << 8;
    pub const A: u16 = 1 << 7;
    pub const X: u16 = 1 << 6;
    pub const TLEFT: u16 = 1 << 5;
    pub const TRIGHT: u16 = 1 << 4;
}

pub const MAX_OBJECTS: usize = 128;
pub const MAX_DRAW_LIST: usize = 128;

/// One entry of the per-tick draw list, matching C `DrawListEntry`
/// (`src/types.h`, decompiled from the SNES STRUCTS.INC `dl_` structure).
/// This is the primary differential-test surface: the C oracle dumps these
/// per tick under `SF_STATE_DUMP`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DrawListEntry {
    /// World position, fixed-point 16.16.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Rotation angles, SNES 0-255 units.
    pub rx: i16,
    pub ry: i16,
    pub rz: i16,
    pub shape_id: u16,
    pub color_table: u16,
    /// Sort depth; ground objects use 15000.
    pub sort_z: i16,
    pub sflags: u8,
    /// Explosion frame count, 0 = normal.
    pub explosion_cnt: u8,
    pub anim_frame: u8,
    pub col_frame: u8,
    /// Per-object depth-band override (SNES dl_depth).
    pub depth_offset: u8,
    pub flags: u8,
    pub shad_x: i16,
    pub shad_y: i16,
    pub shad_z: i16,
    pub tscroll_x: u8,
    pub tscroll_y: u8,
    /// Stable object identity (alien index + 1) for render interpolation
    /// pairing across ticks; 0 = unpaired.
    pub obj_id: u16,
}

pub mod dl_flags {
    //! Draw list flags, matching `src/types.h` DL_FLAG_*.
    pub const VISIBLE: u8 = 0x01;
    pub const SHADOW: u8 = 0x02;
    pub const HIGHLIGHT: u8 = 0x04;
    pub const WIREFRAME: u8 = 0x08;
}

/// Game tick rate: fixed 20 Hz logic, matching C `GAME_TICK_MS 50`.
pub const GAME_TICK_MS: u32 = 50;
