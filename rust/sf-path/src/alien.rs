//! Alien (object) structure and flag constants.
//!
//! C origin: `src/game/obj.h` (`Alien` struct, decompiled from the SNES
//! STRUCTS.INC `al_start` + `alx_start` blocks, and the ASF*/ACF* flag
//! defines).
//!
//! NOTE: this lives in `sf-path` because the path interpreter is the first
//! Rust consumer. Once the game-core lane (obj.c / strat lane) is ported this
//! type should migrate to a shared crate (`sf-core`) — kept here for now per
//! the "don't publish shared types speculatively" rule.
//!
//! Differences from C, all documented:
//! - C `next`/`prev` are `Alien *` linked-list pointers; here they are
//!   `Option<u16>` indices into the `g_aliens`-equivalent array. The SNES
//!   byte-offset compat layer never exposes them (offsets 0..3 are denied in
//!   `src/game/alien_compat.c`), so the representation is free.
//! - C strategy function pointers (`stratptr`, `expstratptr`, `collstratptr`,
//!   `endcollstratptr`, `tempstratptr`) become [`StratRef`] enum values; the
//!   flat runtime denies byte access to them just like the C compat layer.
//! - The packed source `ssprite` display bit becomes [`ObjectVisualKind`], so
//!   presentation intent is a typed field instead of an emulated flag.

/// Max aliens (from VARS.INC `number_al`, `src/game/obj.h` NUMBER_AL).
pub const NUMBER_AL: usize = 70;

/// Strategy routine reference — replaces C `StrategyFunc` function pointers
/// (`src/game/obj.h` `typedef void (*StrategyFunc)(struct Alien *self)`).
///
/// Variants cover every routine `paths.c`/`path_literals.c` store into an
/// alien strategy slot; the future obj-lane port dispatches on this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StratRef {
    /// `Strat_Path_Tick` (src/path/paths.c).
    PathTick,
    /// `path_on_collision` (src/path/paths.c static).
    PathOnCollision,
    /// `Strat_Explode` (src/strat/strat_enemy.c) — provided by the strat lane.
    StratExplode,
    /// `path_particleexplode_istrat` (src/path/paths.c static).
    ParticleExplodeIstrat,
    /// `path_particleexplode_strat` (src/path/paths.c static).
    ParticleExplodeStrat,
    /// `path_trail_tick` (src/path/paths.c static).
    TrailTick,
    /// `path_literal_particlepollen_strat` (src/path/path_literals.c static).
    ParticlePollenStrat,
    /// Opaque handle for a native strategy already registered by the game
    /// runtime. This is deliberately a typed handle, not a synthetic source
    /// address: native strategy identity has no relationship to ROM layout.
    Native(u16),
    /// Strategy resolved through `World_FindStrategyAddress` from a 24-bit
    /// SNES address (P_SETSTRAT / tow0explode binding). The host maps the
    /// address to a real routine.
    External(u32),
}

/// Native presentation selected for an object.
///
/// The source stores software-sprite promotion in a packed strategy flag.
/// The Rust port models the resulting rendering concept directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObjectVisualKind {
    #[default]
    Mesh,
    ScaledSprite,
}

/// Alien (object) structure — every 3D entity in the game: enemies, items,
/// player wingmen, obstacles, player. Models the source `Alien`
/// (`src/game/obj.h`) fields one-for-one, with typed Rust fields replacing
/// packed presentation and strategy identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Alien {
    /// Linked-list next (C `struct Alien *next`; index into the alien array).
    pub next: Option<u16>,
    /// Linked-list prev (C `struct Alien *prev`).
    pub prev: Option<u16>,

    // --- al_start fields (STRUCTS.INC) ---
    /// al_shape: Shape reference.
    pub shape: u16,
    /// Typed replacement for the source `ssprite` display flag.
    pub visual_kind: ObjectVisualKind,
    /// al_ptr: Attached alien pointer (array index; 0 = null, matching the
    /// C/SNES convention where object "pointer" 0 means none).
    pub ptr: u16,
    /// al_flags: AFEXP, AFHIT, AF_FRONT_PL, etc.
    pub flags: u8,
    /// al_type: ATGND, ATMISSILE, ATLASER, ATZREMOVE, etc.
    pub type_: u8,
    /// al_count: Frame/explosion counter.
    pub count: u8,
    /// al_count1: Secondary counter.
    pub count1: u8,
    /// al_worldx/y/z: World position.
    pub worldx: i16,
    pub worldy: i16,
    pub worldz: i16,
    /// al_rotx/y/z: Rotation angles (0-255).
    pub rotx: u8,
    pub roty: u8,
    pub rotz: u8,
    /// al_vel: Velocity magnitude.
    pub vel: u8,

    // --- GILESAL.INC fields (strategy data, part of al_start block) ---
    /// al_stratptr: Current strategy routine.
    pub stratptr: Option<StratRef>,
    /// Pointer to immunity object.
    pub immuneptr: u16,
    /// Pointer to collision object.
    pub collobjptr: u16,
    /// al_sflags: Strategy/display flags (asf_*).
    pub sflags: u8,
    pub sflags2: u8,
    pub sflags3: u8,
    pub sflags4: u8,
    /// Skid Y.
    pub skidy: u8,
    /// Temp bytes 1-4 (sbyte1 doubles as particle amount, etc.).
    pub sbyte1: u8,
    pub sbyte2: u8,
    pub sbyte3: u8,
    pub sbyte4: u8,
    /// Temp words.
    pub sword1: i16,
    pub sword2: i16,
    /// al_hp: Hit points.
    pub hp: u8,
    /// al_ap: Armor points.
    pub ap: u8,
    /// Weapon type.
    pub weapontype: u8,
    /// Collision time counter.
    pub collcount: u8,
    /// al_collflags: Collision flags (acf_*).
    pub collflags: u8,
    /// Velocity vector.
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
    /// Hit flags.
    pub hitflags: u8,
    pub sbyte5: u8,
    pub sbyte6: u8,

    // --- Extended alien data (STRUCTS.INC alx_start, GILESALX.INC) ---
    /// Weapon position.
    pub swpx1: i16,
    pub swpy1: i16,
    pub swpz1: i16,
    /// Explosion strategy.
    pub expstratptr: Option<StratRef>,
    /// Collision strategy.
    pub collstratptr: Option<StratRef>,
    /// End-collision strategy.
    pub endcollstratptr: Option<StratRef>,
    /// Temporary/saved strategy.
    pub tempstratptr: Option<StratRef>,
    /// Strategy state machine state.
    pub stratstate: u8,
    /// Fire object pointer.
    pub fireobjptr: u16,
    /// al_depthoffset: Depth offset for draw list.
    pub depthoffset: i16,
    /// Relative position.
    pub relposx: u8,
    pub relposy: u8,
    pub relposz: u8,
    /// Debris shape ID.
    pub debrisshape: u16,

    // --- STRUCTS.INC alx extended fields ---
    /// al_colframe: Color animation frame.
    pub colframe: u8,
    /// al_animframe: Animation frame.
    pub animframe: u8,
    /// Sound IDs.
    pub snd1: u8,
    pub snd2: u8,
    /// al_coltab: Color table pointer.
    pub coltab: u16,
    /// Child relative offsets.
    pub childx: u8,
    pub childy: u8,
    pub childz: u8,
    /// Child rotation.
    pub childrotx: u8,
    pub childroty: u8,
    pub childrotz: u8,
    /// Child rotation object.
    pub childrotobj: u16,
    /// al_tx, al_ty: Texture scroll coordinates.
    pub tx: u8,
    pub ty: u8,
    /// Strategy memory pointer.
    pub memptr: u16,
    /// Stack pointer.
    pub stackptr: u16,
    /// Strategy memory.
    pub stratmem: u16,
    /// Permanent bytes.
    pub pbyte1: u8,
    pub pbyte2: u8,
    /// Permanent word.
    pub pword1: u16,

    /// Active flag (not on SNES — used instead of list membership checks).
    pub active: bool,
}

// Alien strategy flags (al_sflags) — from VARS.INC / STRATEQU.INC
// (C origin: src/game/obj.h ASF_*).
pub const ASF_INVISIBLE: u8 = 0x01;
pub const ASF_HITFLASH: u8 = 0x02;
pub const ASF_SHADOW: u8 = 0x08;
pub const ASF_PARTOBJ: u8 = 0x10;
pub const ASF_TEXTOBJ: u8 = 0x40;
pub const ASF_COLLDISABLE: u8 = 0x10;
pub const ASF_COLLIDE: u8 = 0x80;
pub const ASF_NOHITAFFECT: u8 = 0x40;
pub const ASF_LCOLLIDE: u8 = 0x80;

pub const ASF2_COLLDISABLE: u8 = 0x01;
pub const ASF2_LCOLLIDE: u8 = 0x02;

// al_sflags3 bits (C origin: src/game/obj.h ASF3_*).
pub const ASF3_SFLAG5: u8 = 0x01; // path trigger: hit by player weapon
pub const ASF3_SFLAG7: u8 = 0x04; // path trigger: any collision hit
pub const ASF3_REALOBJ: u8 = 0x08;
pub const ASF3_CHILDOBJ: u8 = 0x10;
pub const ASF3_MOTHEROBJ: u8 = 0x20;
pub const ASF3_TEXTOBJ: u8 = 0x40;
pub const ASF3_NOHITAFFECT: u8 = 0x20;

// al_sflags4 bits (C origin: src/game/obj.h ASF4_*).
pub const ASF4_PLAYEROBJ: u8 = 0x01;
pub const ASF4_DONESND: u8 = 0x02;
pub const ASF4_NOPOLYEXP: u8 = 0x04;
pub const ASF4_INVISIBLE: u8 = 0x08;
pub const ASF4_SFLAG8: u8 = 0x20;

// al_sflags2 explosion-system bits (C origin: src/game/obj.h ASF2_*).
pub const ASF2_RELEXPLODE: u8 = 0x04;
pub const ASF2_NOEXPSND: u8 = 0x08;
pub const ASF2_SFLAG1: u8 = 0x10;

// al_flags / al_type bits used by the path lane (C origin: src/variables.h).
/// al_flags bit AFEXP — exploding (`src/variables.h`).
pub const AFEXP: u8 = 1;
/// al_type bit ATZREMOVE — remove when behind (`src/variables.h`).
pub const ATZREMOVE: u8 = 8;

// Alien collision flags (al_collflags; C origin: src/game/obj.h ACF_*).
pub const ACF_COLLTYPE6: u8 = 0x01;
pub const ACF_WEAPON: u8 = 0x02;
pub const ACF_FIRSTFRAME: u8 = 0x04;
pub const ACF_COLLTYPE1: u8 = 0x08;
pub const ACF_COLLTYPE2: u8 = 0x10;
pub const ACF_COLLTYPE3: u8 = 0x20;
pub const ACF_COLLTYPE4: u8 = 0x40;
pub const ACF_COLLTYPE5: u8 = 0x80;
