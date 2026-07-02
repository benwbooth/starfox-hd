//! Alien (object) structure and flag constants — game-core lane copy.
//!
//! C origin: `src/game/obj.h` (`Alien` struct, decompiled from the SNES
//! STRUCTS.INC `al_start` + `alx_start` blocks) and the ASF*/ACF*/AF*/AT*
//! flag defines (`src/game/obj.h`, `src/variables.h`).
//!
//! TODO(consolidation): `sf-path/src/alien.rs` carries a near-identical copy
//! whose strategy slots are a path-lane `StratRef` enum. This copy diverges
//! on purpose: the game core dispatches strategies through a registry of fn
//! pointers (see [`crate::world::World`]), so strategy slots here are opaque
//! [`StratId`] registry handles. Once sf-strat lands, both copies should
//! merge into one shared type (likely in `sf-core`) with the registry-id
//! representation.

/// Max aliens (VARS.INC `number_al`; C `src/game/obj.h` NUMBER_AL).
pub const NUMBER_AL: usize = 70;

/// Opaque strategy-registry handle replacing C `StrategyFunc` function
/// pointers (`src/game/obj.h`). Index into `World::strat_registry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StratId(pub u16);

/// Alien (object) structure — every 3D entity in the game. Mirrors C `Alien`
/// (`src/game/obj.h`) field-for-field with the same integer widths.
/// `next`/`prev` are array indices instead of pointers (the SNES compat
/// layer denies offsets 0..3, so the representation is free).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Alien {
    /// Linked-list next (C `struct Alien *next`).
    pub next: Option<u16>,
    /// Linked-list prev (C `struct Alien *prev`).
    pub prev: Option<u16>,

    // --- al_start fields (STRUCTS.INC) ---
    /// al_shape.
    pub shape: u16,
    /// al_ptr: attached alien "pointer" (index+1 encoding; 0 = none).
    pub ptr: u16,
    /// al_flags (AFEXP, AFHIT, ...).
    pub flags: u8,
    /// al_type (ATGND, ATZREMOVE, ATNUKED, ...).
    pub type_: u8,
    /// al_count.
    pub count: u8,
    /// al_count1.
    pub count1: u8,
    /// al_worldx/y/z.
    pub worldx: i16,
    pub worldy: i16,
    pub worldz: i16,
    /// al_rotx/y/z (0-255).
    pub rotx: u8,
    pub roty: u8,
    pub rotz: u8,
    /// al_vel.
    pub vel: u8,

    // --- GILESAL.INC strategy fields ---
    /// al_stratptr — current strategy routine (registry handle).
    pub stratptr: Option<StratId>,
    /// Immunity object pointer (index+1-style u16, as C).
    pub immuneptr: u16,
    /// Collision object pointer.
    pub collobjptr: u16,
    /// al_sflags (ASF_*).
    pub sflags: u8,
    pub sflags2: u8,
    pub sflags3: u8,
    pub sflags4: u8,
    pub skidy: u8,
    pub sbyte1: u8,
    pub sbyte2: u8,
    pub sbyte3: u8,
    pub sbyte4: u8,
    pub sword1: i16,
    pub sword2: i16,
    /// al_hp.
    pub hp: u8,
    /// al_ap.
    pub ap: u8,
    pub weapontype: u8,
    /// Collision time counter.
    pub collcount: u8,
    /// al_collflags (ACF_*).
    pub collflags: u8,
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
    pub hitflags: u8,
    pub sbyte5: u8,
    pub sbyte6: u8,

    // --- Extended alien data (STRUCTS.INC alx_start, GILESALX.INC) ---
    pub swpx1: i16,
    pub swpy1: i16,
    pub swpz1: i16,
    /// Explosion strategy.
    pub expstratptr: Option<StratId>,
    /// Collision strategy.
    pub collstratptr: Option<StratId>,
    /// End-collision strategy.
    pub endcollstratptr: Option<StratId>,
    /// Temporary/saved strategy.
    pub tempstratptr: Option<StratId>,
    pub stratstate: u8,
    pub fireobjptr: u16,
    /// al_depthoffset.
    pub depthoffset: i16,
    pub relposx: u8,
    pub relposy: u8,
    pub relposz: u8,
    pub debrisshape: u16,

    // --- STRUCTS.INC alx extended fields ---
    pub colframe: u8,
    pub animframe: u8,
    pub snd1: u8,
    pub snd2: u8,
    pub coltab: u16,
    pub childx: u8,
    pub childy: u8,
    pub childz: u8,
    pub childrotx: u8,
    pub childroty: u8,
    pub childrotz: u8,
    pub childrotobj: u16,
    pub tx: u8,
    pub ty: u8,
    pub memptr: u16,
    pub stackptr: u16,
    pub stratmem: u16,
    pub pbyte1: u8,
    pub pbyte2: u8,
    pub pword1: u16,

    /// Active flag (not on SNES — replaces list-membership checks).
    pub active: bool,
}

// Alien strategy flags al_sflags (C `src/game/obj.h` ASF_*).
pub const ASF_INVISIBLE: u8 = 0x01;
pub const ASF_HITFLASH: u8 = 0x02;
pub const ASF_SHADOW: u8 = 0x04;
pub const ASF_PARTOBJ: u8 = 0x08;
pub const ASF_COLLDISABLE: u8 = 0x10;
pub const ASF_COLLIDE: u8 = 0x20;
pub const ASF_NOHITAFFECT: u8 = 0x40;
pub const ASF_LCOLLIDE: u8 = 0x80;

// al_sflags3 bits (C ASF3_*).
pub const ASF3_REALOBJ: u8 = 0x08;

// al_sflags4 bits (C `src/game/obj.h` ASF4_* + `src/game/world.h` markers).
pub const ASF4_PLAYEROBJ: u8 = 0x01;
pub const ASF4_SFLAG8: u8 = 0x20;
/// Map `special` marker (C `src/game/world.h` ASF4_SPECIAL).
pub const ASF4_SPECIAL: u8 = 0x40;
/// Map `cspecial` marker (C `src/game/world.h` ASF4_CSPECIAL).
pub const ASF4_CSPECIAL: u8 = 0x80;

// al_flags bits (C `src/variables.h`).
pub const AFEXP: u8 = 1;

// al_type bits (C `src/variables.h`).
pub const ATGND: u8 = 1;
pub const ATMISSILE: u8 = 2;
pub const ATLASER: u8 = 4;
pub const ATZREMOVE: u8 = 8;
pub const ATNUKED: u8 = 16;

// Alien collision flags al_collflags (C `src/game/obj.h` ACF_*).
pub const ACF_COLLTYPE6: u8 = 0x01;
pub const ACF_WEAPON: u8 = 0x02;
pub const ACF_FIRSTFRAME: u8 = 0x04;
pub const ACF_COLLTYPE1: u8 = 0x08;
pub const ACF_COLLTYPE2: u8 = 0x10;
pub const ACF_COLLTYPE3: u8 = 0x20;
pub const ACF_COLLTYPE4: u8 = 0x40;
pub const ACF_COLLTYPE5: u8 = 0x80;
