//! Map executor state: strategy/callback registries, level loading,
//! builtin spacebar istrats, shape-word resolution.
//!
//! C oracle: `src/game/world.c` (state + registration half; the opcode
//! dispatcher `map_exec` itself lives in [`crate::game`]) and
//! `src/map/map_exec.c` (`MapExec_LoadLevel` facade).

use crate::alien::{StratId, ACF_COLLTYPE2};
use crate::game::{Game, StrategyFn};
use crate::vars::{HARD_AP, HARD_HP};
use sf_map::consts::DirectStrategy;
use sf_map::levels::{BuiltLevel, InlineCallback, NativeCallback};

/// Synthetic strategy-address key for `Strat_PlayerExitBase`, registered by
/// the strat lane (`sf_strat::table::register_all`) and looked up by the map
/// VM's SET_PLAYER_EXITBASE_L builtin callback (C `world_cb_set_player_exitbase_l`
/// calls `Strat_PlayerExitBase` directly; the Rust port routes through the
/// strat registry to avoid an sf-game -> sf-strat dependency). Value chosen to
/// not collide with the other 0x03xxxx synthetic addresses (table.rs).
pub const STRAT_ADDR_PLAYER_EXITBASE: u32 = 0x03_0003;

/// Map opcode constants (C `src/game/world.h` MAP_OP_*, from MAPMACS.INC).
pub mod op {
    pub const MAPOBJ: u8 = 0;
    pub const END: u8 = 2;
    pub const LOOP: u8 = 4;
    pub const DEBUG: u8 = 6;
    pub const NOP: u8 = 8;
    pub const MOTHER: u8 = 10;
    pub const REMOVE: u8 = 12;
    pub const SETSTAGE: u8 = 14;
    pub const SETBG: u8 = 16;
    pub const WAIT: u8 = 18;
    pub const SETBGM: u8 = 20;
    pub const NODOTS: u8 = 22;
    pub const GNDDOTS: u8 = 24;
    pub const SPACEDUST: u8 = 26;
    pub const SETOTHMUS: u8 = 28;
    pub const VOFSON: u8 = 30;
    pub const VOFSOFF: u8 = 32;
    pub const HOFSON: u8 = 34;
    pub const HOFSOFF: u8 = 36;
    pub const OBJZROT: u8 = 38;
    pub const JSR: u8 = 40;
    pub const RTS: u8 = 42;
    pub const IF: u8 = 44;
    pub const GOTO: u8 = 46;
    pub const SETXROT: u8 = 48;
    pub const SETYROT: u8 = 50;
    pub const SETZROT: u8 = 52;
    pub const SETALVARB: u8 = 54;
    pub const SETALVARW: u8 = 56;
    pub const SETALVARL: u8 = 58;
    pub const SETALXVARB: u8 = 60;
    pub const SETALXVARW: u8 = 62;
    pub const SETALXVARL: u8 = 64;
    pub const FADEUP: u8 = 66;
    pub const FADEDOWN: u8 = 68;
    pub const SETALVARPB: u8 = 70;
    pub const SETALVARPW: u8 = 72;
    pub const SETVAROBJ: u8 = 74;
    pub const WAITFADE: u8 = 76;
    pub const QFADEUP: u8 = 78;
    pub const QFADEDOWN: u8 = 80;
    pub const SCREENOFF: u8 = 82;
    pub const SCREENON: u8 = 84;
    pub const ZROTOFF: u8 = 86;
    pub const ZROTON: u8 = 88;
    pub const SPECIAL: u8 = 90;
    pub const SETVARB: u8 = 92;
    pub const SETVARW: u8 = 94;
    pub const SETVARL: u8 = 96;
    pub const SETBGSLOW: u8 = 98;
    pub const WAITSETBG: u8 = 100;
    pub const SETBGINFO: u8 = 102;
    pub const ADDALVARPB: u8 = 104;
    pub const ADDALVARPW: u8 = 106;
    pub const FADETOSEA: u8 = 108;
    pub const FADETOGROUND: u8 = 110;
    pub const QOBJ: u8 = 112;
    pub const OBJ8: u8 = 114;
    pub const DOBJ: u8 = 116;
    pub const QOBJ2: u8 = 118;
    pub const CODE65816: u8 = 120;
    pub const CODEJSL: u8 = 122;
    pub const JMPVARLESS: u8 = 124;
    pub const JMPVARMORE: u8 = 126;
    pub const JMPVAREQ: u8 = 128;
    pub const SENDMSG: u8 = 130;
    pub const CSPECIAL: u8 = 132;
    pub const NORMOBJ: u8 = 134;
    /// Retail dispatch-table alias that falls through to [`SETBGM`].
    pub const SETBGM_ALIAS: u8 = 136;
    pub const WAIT2: u8 = 138;
    pub const SETPATH: u8 = 140;
    pub const DIRECTOBJ: u8 = 142;
    pub const DIRECTMOTHER: u8 = 144;
    pub const PRESERVE_BEHIND_VIEW_OBJECTS: u8 = 146;
}

/// C `MAP_JSR_STACK_SIZE` (src/game/world.h).
pub const MAP_JSR_STACK_SIZE: usize = 16;
/// C `MAP_MAX_LOOPS`.
pub const MAP_MAX_LOOPS: usize = 4;
/// C `ISTRAT_CAPACITY` (src/strat/istrat_shapes.h).
pub const ISTRAT_CAPACITY: usize = 256;
/// C `MAX_SHAPES` (src/game/world.c:82).
pub const MAX_SHAPES: usize = 256;

// Builtin spacebar istrat ids (C `src/game/world.h` MAP_ISTRAT_*).
pub const MAP_ISTRAT_SPACEBAR: usize = 165;
pub const MAP_ISTRAT_SPINSPACEBAR: usize = 166;
pub const MAP_ISTRAT_SPACEBAR1: usize = 167;
pub const MAP_ISTRAT_SPACEBAR3: usize = 168;
/// `spacebar2` has no `def_Istrat` row — spawned via `s_make_obj` + stratptr.
/// World lane still owns the tick/init for the solid-spacebar family.

/// ISTRATS def_shape index for ro_0 (C `src/game/world.c` WORLD_SHAPE_RO_0,
/// used by the kill_robot_l builtin).
pub const WORLD_SHAPE_RO_0: u16 = 170;

/// Native callback identity resolved from a 24-bit script address.
/// C equivalent: the `s_native_callbacks` table entries — builtins from
/// `world_register_builtin_callbacks()` (src/game/world.c:780) plus
/// level-registered callbacks (levels.c `register_*` -> here the identities
/// carried by [`BuiltLevel::native_callbacks`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCb {
    /// A world.c builtin, keyed by its MAP_CB_* addr24.
    Builtin(u32),
    /// A level-registered callback identity from sf-map.
    Level(NativeCallback),
}

/// Inline CODE65816 callback identity (C `s_inline_map_funcs` entries;
/// levels.c `World_RegisterInlineMapCode` registrations). The skillfly
/// guard carries its resolved skip offset (C `s_level1_1_skillfly_bonus_
/// skip_ptr` etc., captured at registration time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCb {
    /// INTRO/CREDITS opening 65816 block: reset scroll/camera Z and player Z.
    ResetViewPlayerZ,
    /// INTRO/CREDITS post-fade block: disable wobble and player input/fire.
    DisableWobbleAndControl,
    /// LEVEL1_1 scramble handoff: remain on the inline instruction until the
    /// typed presentation fade completes, without changing map distance.
    Level1_1WaitFade,
    /// C `level_scramble_keep_player_strat` (src/map/levels.c:1690).
    LevelScrambleKeepPlayerStrat,
    /// C `level1_1_skillfly_bonus_guard` (src/map/levels.c:1671).
    SkillflyGuard { skip_ptr: u16 },
    /// Inlined `mapif chkstratdone1,<target>` used by route maps whose old
    /// builder represented the source macro as a CODE65816 hook.
    Stratdone1Guard { skip_ptr: u16 },
    /// Inlined `mapgotoifplayerdead <loop>` used by Fortuna's ground/water
    /// transition gates.
    PlayerDeadLoop { loop_ptr: u16 },
    /// MAP3_6 `.boss` gate: retry while the player's no-control bit remains
    /// set, otherwise continue at the instruction after CODE65816.
    NoctrlLoop { loop_ptr: u16 },
    /// MAP3_6 `.owait` gate: retry while the player is dead, then normalize
    /// the inside-cockpit view if necessary and jump to the boss spawn.
    HpFlymodeGate { hp_loop_ptr: u16, cont_ptr: u16 },
    /// MAP2_3A `eguchi2fly_goto`: stay in the fog encounter until the
    /// `tenki_on` path raises `ebyte3`, then allow the following mapgoto.
    FogGuard { continue_ptr: u16 },
    /// TRAINING.ASM `eguchifly_goto`: skip its following mapgoto while fewer
    /// than fifteen rings have been flown; otherwise repeat the course.
    EguchiFlyGate { continue_ptr: u16 },
    /// MAP2_3A post-fog 65816 block (ground dots and palette/fog globals).
    PostFog,
    /// MAP2_3B wave gate driven by `maptrigger` bits from the sea boss.
    MapTriggerGate { carryon_ptr: u16, waitabit_ptr: u16 },
    /// MAP2_3B seamon wave counter wait.
    SeaTestLoop { loop_ptr: u16 },
    /// TRUCKER.ASM waits until every `air_1` escort bike is gone.
    TruckerBikerGate { carryon_ptr: u16 },
    /// TRUCKER.ASM consumes Mad Trucker's road-block/death trigger bits.
    TruckerTriggerGate {
        rightblock_ptr: u16,
        continue_ptr: u16,
    },
    /// SPECIAL.ASM end-of-slot-machine block: re-enable fire/death and hide
    /// the boss HUD before the credits fly-through.
    SpecialBossCleanup,
    /// SPECIAL.ASM THE END loop gate.
    SpecialTheEndGate { loop_ptr: u16, cont_ptr: u16 },
    /// C `level1_1_mapwaitboss_trigse` (src/map/levels.c:1769).
    MapwaitbossTrigse,
    /// C `level1_1_mapwaitboss_cantdie` (src/map/levels.c:1775).
    MapwaitbossCantdie,
    /// C `level1_1_mapwaitboss_cleanup` (src/map/levels.c:1781).
    MapwaitbossCleanup,
    /// C `title_init_inline` (src/map/levels.c:2245).
    TitleInit,
    /// C `contmap_init_inline` (src/map/levels.c:2253).
    ContmapInit,
}

/// Map a route lane's native-callback C-fn-name to its typed identity
/// (C `register_*_native_callbacks`). Only the clear-demo natives have ported
/// identities. A completeness regression covers every name emitted by the map
/// catalog, so `None` is reserved for malformed or out-of-catalog data.
fn native_cb_from_name(name: &str) -> Option<NativeCallback> {
    if name.ends_with("printlevelfin") {
        Some(NativeCallback::ClGroundPrintlevelfin)
    } else if name.ends_with("wipeout") {
        Some(NativeCallback::ClGroundWipeout)
    } else if name.contains("enginesnd") {
        Some(NativeCallback::ClDiveClearEnginesnd)
    } else if name == "level2_3_bg_1_4b_1" {
        Some(NativeCallback::TitaniaClearFogScene)
    } else {
        None
    }
}

/// Map a route lane's inline (CODE65816) C-fn-name to its typed [`InlineCb`].
/// `guard_ptr` is the inline op's script ptr; `labels` resolves the skillfly
/// guard's skip target as the nearest following `skillfly*skip` label (guards
/// and their skip labels are emitted in interleaved order, so the closest skip
/// label after the guard is its own). Unrecognized names return `None` — those
/// map VMs will still halt at the op (a pre-existing gap for that specific
/// level; the launch/boss/clear inlines that gate player control are covered).
fn inline_cb_from_name(name: &str, guard_ptr: u16, labels: &[(String, u16)]) -> Option<InlineCb> {
    if name.ends_with("reset_view_inline") {
        Some(InlineCb::ResetViewPlayerZ)
    } else if name == "intro_init_inline" || name == "credits_init_inline" {
        Some(InlineCb::DisableWobbleAndControl)
    } else if name.contains("scramble_keep_player_strat") {
        Some(InlineCb::LevelScrambleKeepPlayerStrat)
    } else if name.contains("skillfly") || name.contains("bonus_guard") {
        let skip_ptr = labels
            .iter()
            .filter(|(n, off)| *off > guard_ptr && n.contains("bonus") && n.contains("skip"))
            .map(|&(_, off)| off)
            .min()
            .unwrap_or(0);
        Some(InlineCb::SkillflyGuard { skip_ptr })
    } else if name.contains("chkstratdone1") {
        let skip_ptr = labels
            .iter()
            .filter(|(n, off)| {
                *off > guard_ptr
                    && (n.ends_with(".cont") || n.ends_with(".end") || n.ends_with(".endcont"))
            })
            .map(|&(_, off)| off)
            .min()
            .unwrap_or(0);
        Some(InlineCb::Stratdone1Guard { skip_ptr })
    } else if name.contains("pdead2_check") || name.contains("pdead_check") {
        let wanted = if name.contains("pdead2_check") {
            ".pdead2"
        } else {
            ".pdead"
        };
        let loop_ptr = labels
            .iter()
            .filter(|(n, off)| *off < guard_ptr && n.ends_with(wanted))
            .map(|(_, off)| *off)
            .max()
            .unwrap_or(0);
        Some(InlineCb::PlayerDeadLoop { loop_ptr })
    } else if name.contains("map3_6_noctrl_wait") {
        let loop_ptr = labels
            .iter()
            .filter(|(n, off)| *off < guard_ptr && n.ends_with(".boss"))
            .map(|(_, off)| *off)
            .max()
            .unwrap_or(0);
        Some(InlineCb::NoctrlLoop { loop_ptr })
    } else if name.contains("map3_6_hpcheck_wait") {
        let hp_loop_ptr = labels
            .iter()
            .filter(|(n, off)| *off < guard_ptr && n.ends_with(".owait"))
            .map(|(_, off)| *off)
            .max()
            .unwrap_or(0);
        let cont_ptr = labels
            .iter()
            // The builder's inline registration key is the address just past
            // CODE65816.  A continuation label emitted immediately after the
            // opcode therefore has the SAME offset as `guard_ptr`.
            .filter(|(n, off)| *off >= guard_ptr && n.ends_with(".cont2"))
            .map(|(_, off)| *off)
            .min()
            .unwrap_or(0);
        Some(InlineCb::HpFlymodeGate {
            hp_loop_ptr,
            cont_ptr,
        })
    } else if name == "level2_3_fog_guard" {
        let continue_ptr = labels
            .iter()
            .filter(|(n, off)| *off >= guard_ptr && n.ends_with(".fog_guard_continue"))
            .map(|(_, off)| *off)
            .min()
            .unwrap_or(0);
        Some(InlineCb::FogGuard { continue_ptr })
    } else if name == "training_eguchifly_check" {
        let continue_ptr = labels
            .iter()
            .filter(|(n, off)| *off >= guard_ptr && n.ends_with(".eguchifly_continue"))
            .map(|(_, off)| *off)
            .min()
            .unwrap_or(0);
        Some(InlineCb::EguchiFlyGate { continue_ptr })
    } else if name == "level2_3_setvar_inline" {
        Some(InlineCb::PostFog)
    } else if name == "level2_3b_trigger_check" {
        let carryon_ptr = labels
            .iter()
            .filter(|(n, off)| *off > guard_ptr && n.ends_with(".carryon"))
            .map(|(_, off)| *off)
            .min()
            .unwrap_or(0);
        let waitabit_ptr = labels
            .iter()
            .filter(|(n, off)| *off < guard_ptr && n.ends_with(".waitabit"))
            .map(|(_, off)| *off)
            .max()
            .unwrap_or(0);
        Some(InlineCb::MapTriggerGate {
            carryon_ptr,
            waitabit_ptr,
        })
    } else if name == "level2_3b_seatest_check" {
        let loop_ptr = labels
            .iter()
            .filter(|(n, off)| *off < guard_ptr && n.ends_with(".seatest"))
            .map(|(_, off)| *off)
            .max()
            .unwrap_or(0);
        Some(InlineCb::SeaTestLoop { loop_ptr })
    } else if name == "trucker_biker_check" {
        let carryon_ptr = labels
            .iter()
            .filter(|(n, off)| *off > guard_ptr && n.ends_with(".trucker.carryon"))
            .map(|(_, off)| *off)
            .min()
            .unwrap_or(0);
        Some(InlineCb::TruckerBikerGate { carryon_ptr })
    } else if name == "trucker_trigger_check" {
        let rightblock_ptr = labels
            .iter()
            .filter(|(n, off)| *off > guard_ptr && n.ends_with(".trucker.rightblockbit"))
            .map(|(_, off)| *off)
            .min()
            .unwrap_or(0);
        let continue_ptr = labels
            .iter()
            .filter(|(n, off)| *off > guard_ptr && n.ends_with(".trucker.continue"))
            .map(|(_, off)| *off)
            .min()
            .unwrap_or(0);
        Some(InlineCb::TruckerTriggerGate {
            rightblock_ptr,
            continue_ptr,
        })
    } else if name == "special_boss_cleanup" {
        Some(InlineCb::SpecialBossCleanup)
    } else if name == "special_theenddead_check" {
        let loop_ptr = labels
            .iter()
            .find(|(n, _)| n.ends_with(".theenddead_check"))
            .map(|(_, off)| *off)
            .unwrap_or(0);
        let cont_ptr = labels
            .iter()
            .find(|(n, _)| n.ends_with(".theenddead_cont"))
            .map(|(_, off)| *off)
            .unwrap_or(0);
        Some(InlineCb::SpecialTheEndGate { loop_ptr, cont_ptr })
    } else if name.ends_with("mapwaitboss_trigse") {
        Some(InlineCb::MapwaitbossTrigse)
    } else if name.ends_with("mapwaitboss_cantdie") {
        Some(InlineCb::MapwaitbossCantdie)
    } else if name.ends_with("mapwaitboss_cleanup") {
        Some(InlineCb::MapwaitbossCleanup)
    } else {
        None
    }
}

/// C `Shapes_ResolveShapeWord()` (src/renderer/shapes.c:452) — canonicalize
/// live raw 16-bit shape words into flat runtime ids.
/// TODO(consolidation): belongs to the sf-render lane; local literal copy
/// until that crate exposes it.
pub fn resolve_shape_word(shape_id: u16) -> u16 {
    sf_core::shape::resolve_shape_word(shape_id)
}

/// ROM `set_restart_position_l` / `restart_l` checkpoint (DSTRATS.ASM:1805).
/// Saves the typed map-program cursor, BG/palette fade, and the map-language
/// call/loop stacks. The source bank byte is intentionally absent: native map
/// programs live in one flat collection.
#[derive(Debug, Clone, Default)]
pub struct MapRestart {
    pub mapptr: u16,
    pub bg: u16,
    pub palfade: u16,
    pub jsr_stack: [u16; MAP_JSR_STACK_SIZE],
    pub jsr_top: usize,
    pub num_jsr: i32,
    pub loop_addrs: [u16; MAP_MAX_LOOPS],
    pub loop_counts: [u16; MAP_MAX_LOOPS],
    pub num_loops: usize,
}

/// Map executor state (C file-statics of `src/game/world.c` plus the
/// exported `g_lastplayz`/`g_lastzchange`/`g_lastmapobj`/... globals).
pub struct World {
    /// Semantic catalog identity of the currently loaded map. Direct test
    /// programs may omit it; shell-loaded retail maps always set it.
    pub loaded_map_id: Option<u32>,
    /// Encounter-marker ordinal within [`Self::loaded_map_id`]. This
    /// disambiguates catalog entries that contain both a stage boss and the
    /// shared final battle.
    pub boss_marker_ordinal: u8,
    /// C `s_map_data`/`s_map_length` (owned copy of the level bytecode).
    pub map: Vec<u8>,
    /// True once a level is loaded (C `s_map_data != NULL`).
    pub map_loaded: bool,

    /// C `g_lastplayz`.
    pub lastplayz: i16,
    /// C `g_lastzchange`.
    pub lastzchange: i16,
    /// C `s_lastmapobj` (slot index of the last spawned map object; kept
    /// even after the object is freed, exactly like the C pointer).
    pub last_obj: Option<u16>,
    /// C `g_lastmapobj` (index+1 encoding, 0 = invalid).
    pub lastmapobj: u16,
    /// C `g_specialobjtotal`.
    pub specialobjtotal: u8,
    /// Stable per-stage special-object denominator for the end-of-stage hit
    /// percentage (ROM `specialobjtotal` is set once at map build and never
    /// decremented, MAIN.ASM:1057). Kept in sync with [`Self::specialobjtotal`]
    /// at map SPECIAL/CSPECIAL ops; explode only increments `specials_dead`
    /// (tick 148 removed the port invention that decremented specialobjtotal).
    /// Reset each stage in [`Self::load_level`].
    pub total_specials: u8,
    /// C `g_levelfinished`.
    pub levelfinished: u8,

    // C `s_jsr_stack`/`s_jsr_top`/`s_num_jsr` (mapjsrdo/maprtsdo).
    pub jsr_stack: [u16; MAP_JSR_STACK_SIZE],
    pub jsr_top: usize,
    pub num_jsr: i32,

    // C `s_loop_addrs`/`s_loop_counts`/`s_num_loops` (maploopdo).
    pub loop_addrs: [u16; MAP_MAX_LOOPS],
    pub loop_counts: [u16; MAP_MAX_LOOPS],
    pub num_loops: usize,

    /// ROM `set_restart_position_l` snapshot (DSTRATS.ASM:1805).
    pub restart: MapRestart,

    /// C `s_native_callbacks` — level-registered entries only; world.c
    /// builtins are matched statically in [`Game::find_native_callback`].
    pub native_cbs: Vec<(u32, NativeCallback)>,
    /// C `s_inline_map_funcs` (script ptr -> inline callback).
    pub inline_cbs: Vec<(u16, InlineCb)>,
    /// C `s_strat_addr_maps` (24-bit strategy address -> registry id).
    pub strat_addrs: Vec<(u32, StratId)>,

    /// Typed native strategies used by Rust-authored maps. This registry is
    /// intentionally disjoint from ROM compatibility addresses.
    pub direct_strategies: [Option<StratId>; DirectStrategy::COUNT],

    /// C `g_istrats[]` — strategy id -> registry handle. Every canonical row
    /// is populated by `sf_strat::table::register_all`.
    pub istrats: [Option<StratId>; ISTRAT_CAPACITY],
    /// C `g_istrat_shapes[]` — generated canonical shape associated with each
    /// strategy row.
    pub istrat_shapes: [u16; ISTRAT_CAPACITY],
    /// C `g_shapes_table[]` — shape byte-index -> resolved shape word.
    pub shapes_table: [u16; MAX_SHAPES],

    /// Strategy function registry — replaces C function pointers.
    pub strat_registry: Vec<StrategyFn>,
    // Registry handles of the builtin spacebar tick strategies
    // (world.c world_spacebar_strat etc.), needed by their init strats.
    pub sid_spacebar: StratId,
    pub sid_spinspacebar: StratId,
    pub sid_spacebar1: StratId,
    pub sid_spacebar2: StratId,
    /// Init entry for `spacebar2_Istrat` (no map IS row).
    pub sid_spacebar2_init: StratId,
    pub sid_spacebar3: StratId,
}

impl World {
    /// C `World_Init()` (src/game/world.c:833): clear all executor state,
    /// register builtin callbacks/istrats, seed the shape table.
    pub fn init() -> Self {
        let mut w = World {
            loaded_map_id: None,
            boss_marker_ordinal: 0,
            map: Vec::new(),
            map_loaded: false,
            lastplayz: 0,
            lastzchange: 0,
            last_obj: None,
            lastmapobj: 0,
            specialobjtotal: 0,
            total_specials: 0,
            levelfinished: 0,
            jsr_stack: [0; MAP_JSR_STACK_SIZE],
            jsr_top: 0,
            num_jsr: 0,
            loop_addrs: [0; MAP_MAX_LOOPS],
            loop_counts: [0; MAP_MAX_LOOPS],
            num_loops: 0,
            restart: MapRestart::default(),
            native_cbs: Vec::new(),
            inline_cbs: Vec::new(),
            strat_addrs: Vec::new(),
            direct_strategies: [None; DirectStrategy::COUNT],
            istrats: [None; ISTRAT_CAPACITY],
            istrat_shapes: [0; ISTRAT_CAPACITY],
            shapes_table: [0; MAX_SHAPES],
            strat_registry: Vec::new(),
            sid_spacebar: StratId(0),
            sid_spinspacebar: StratId(0),
            sid_spacebar1: StratId(0),
            sid_spacebar2: StratId(0),
            sid_spacebar2_init: StratId(0),
            sid_spacebar3: StratId(0),
        };
        for i in 0..MAX_SHAPES {
            w.shapes_table[i] = resolve_shape_word(i as u16);
        }
        w.register_builtin_istrats();
        w
    }

    /// Register a strategy function, returning its registry handle
    /// (Rust-side replacement for taking a C function's address).
    pub fn register_strategy(&mut self, f: StrategyFn) -> StratId {
        let id = StratId(self.strat_registry.len() as u16);
        self.strat_registry.push(f);
        id
    }

    /// C `World_RegisterStrategyAddress()` (src/game/world.c:388).
    pub fn register_strategy_address(&mut self, addr24: u32, id: StratId) {
        for e in self.strat_addrs.iter_mut() {
            if e.0 == addr24 {
                e.1 = id;
                return;
            }
        }
        self.strat_addrs.push((addr24, id));
    }

    /// C `World_FindStrategyAddress()` (src/game/world.c:404).
    pub fn find_strategy_address(&self, addr24: u32) -> Option<StratId> {
        self.strat_addrs.iter().find(|e| e.0 == addr24).map(|e| e.1)
    }

    pub fn register_direct_strategy(&mut self, strategy: DirectStrategy, id: StratId) {
        self.direct_strategies[strategy.index()] = Some(id);
    }

    pub fn find_direct_strategy(&self, strategy: DirectStrategy) -> Option<StratId> {
        self.direct_strategies[strategy.index()]
    }

    /// C `world_register_builtin_istrats()` (src/game/world.c:320) —
    /// bounded MAPMACS.INC space-bar strategies.
    fn register_builtin_istrats(&mut self) {
        self.sid_spacebar = self.register_strategy(spacebar_strat);
        self.sid_spinspacebar = self.register_strategy(spinspacebar_strat);
        self.sid_spacebar1 = self.register_strategy(spacebar1_strat);
        self.sid_spacebar2 = self.register_strategy(spacebar2_strat);
        self.sid_spacebar3 = self.register_strategy(spacebar3_strat);
        let init_spacebar = self.register_strategy(istrat_spacebar_init);
        let init_spin = self.register_strategy(istrat_spinspacebar_init);
        let init_sb1 = self.register_strategy(istrat_spacebar1_init);
        self.sid_spacebar2_init = self.register_strategy(istrat_spacebar2_init);
        let init_sb3 = self.register_strategy(istrat_spacebar3_init);
        self.istrats[MAP_ISTRAT_SPACEBAR] = Some(init_spacebar);
        self.istrats[MAP_ISTRAT_SPINSPACEBAR] = Some(init_spin);
        self.istrats[MAP_ISTRAT_SPACEBAR1] = Some(init_sb1);
        self.istrats[MAP_ISTRAT_SPACEBAR3] = Some(init_sb3);
    }

    /// C `World_RegisterInlineMapCode()` (src/game/world.c:363).
    pub fn register_inline(&mut self, script_ptr: u16, cb: InlineCb) {
        for e in self.inline_cbs.iter_mut() {
            if e.0 == script_ptr {
                e.1 = cb;
                return;
            }
        }
        self.inline_cbs.push((script_ptr, cb));
    }

    /// Register a route lane's name-keyed callback records (from
    /// `sf_map::catalog::get_map_callback_regs`) into the executor.
    ///
    /// The route lanes (`Route{1,2,3}Level`) record their
    /// `World_RegisterNativeCallback` / `World_RegisterInlineMapCode` calls as
    /// raw `(addr/ptr, C-fn-name)` pairs and leave `BuiltLevel`'s typed callback
    /// vectors empty (a deliberate lane-isolation choice, see their module
    /// docs). Without wiring these the map VM halts forever at the first
    /// unregistered inline CODE65816 op — for the launch levels that op is
    /// `level_scramble_keep_player_strat`, sitting between the scramble intro and
    /// the exit-base setup, so the player never regains control (BUG: "arwing
    /// gets stuck"). This maps each C-fn-name to its typed identity and appends
    /// it. `labels` (the level's label table) resolves the skillfly guard's skip
    /// target. Call AFTER `load_level` (which seeds the lists from `BuiltLevel`).
    pub fn register_named_callbacks(
        &mut self,
        natives: &[(u32, &'static str)],
        inlines: &[(u16, &'static str)],
        labels: &[(String, u16)],
    ) {
        for &(addr, name) in natives {
            if let Some(cb) = native_cb_from_name(name) {
                if !self.native_cbs.iter().any(|e| e.0 == addr) {
                    self.native_cbs.push((addr, cb));
                }
            }
        }
        for &(ptr, name) in inlines {
            if let Some(cb) = inline_cb_from_name(name, ptr, labels) {
                self.register_inline(ptr, cb);
            }
        }
    }

    /// C `world_find_inline_map_func()` (src/game/world.c:379).
    pub fn find_inline(&self, script_ptr: u16) -> Option<InlineCb> {
        self.inline_cbs
            .iter()
            .find(|e| e.0 == script_ptr)
            .map(|e| e.1)
    }

    /// Map an sf-map inline callback identity to the executor's dispatch
    /// enum (C: the fn pointers passed by `register_*_inline_callbacks`).
    fn inline_from_level(level: &BuiltLevel, id: InlineCallback) -> InlineCb {
        match id {
            InlineCallback::Level1_1WaitFade => InlineCb::Level1_1WaitFade,
            InlineCallback::LevelScrambleKeepPlayerStrat => InlineCb::LevelScrambleKeepPlayerStrat,
            InlineCallback::Level1_1SkillflyBonusGuard => InlineCb::SkillflyGuard {
                // C `s_level1_1_skillfly_bonus_skip_ptr` = label lookup at
                // registration time (src/map/levels.c register fn).
                skip_ptr: level
                    .label_offset("level1_1.map1_1b.skillfly_bonus_0_skip")
                    .unwrap_or(0),
            },
            InlineCallback::Level1_1MapwaitbossTrigse => InlineCb::MapwaitbossTrigse,
            InlineCallback::Level1_1MapwaitbossCantdie => InlineCb::MapwaitbossCantdie,
            InlineCallback::Level1_1MapwaitbossCleanup => InlineCb::MapwaitbossCleanup,
            InlineCallback::TitleInit => InlineCb::TitleInit,
            InlineCallback::ContmapInit => InlineCb::ContmapInit,
        }
    }

    /// C `World_LoadLevel()` (src/game/world.c:869) + the registration side
    /// of `MapExec_LoadLevel`/`Levels_GetMapData` (src/map/map_exec.c:14,
    /// levels.c `ensure_literal_levels_built`). Resets execution state and
    /// installs the level's callback registrations.
    ///
    /// Difference from C (documented): C registers every level's callbacks
    /// once into a global table; here only the loaded level's registrations
    /// are installed. Identical behavior for the loaded level because
    /// callback keys never collide across levels for a single run.
    pub fn load_level(&mut self, level: &BuiltLevel, mapptr: &mut u16, mapcnt: &mut u16) {
        self.map = level.data.clone();
        self.map_loaded = true;

        *mapptr = 0;
        *mapcnt = 0;
        self.lastplayz = 0;
        self.lastzchange = 0;
        self.last_obj = None;
        self.lastmapobj = 0;
        // ROM `initlevel` resets specialobjtotal/specials_dead per stage
        // (MAPMACS.INC:876-877). Game::load_level owns the typed numerator;
        // reset both denominator views here.
        self.specialobjtotal = 0;
        self.total_specials = 0;
        self.levelfinished = 0;
        self.jsr_top = 0;
        self.num_jsr = 0;
        self.num_loops = 0;
        self.loop_addrs = [0; MAP_MAX_LOOPS];
        self.loop_counts = [0; MAP_MAX_LOOPS];

        self.native_cbs = level.native_callbacks.clone();
        self.inline_cbs = level
            .inline_callbacks
            .iter()
            .map(|&(ptr, id)| (ptr, Self::inline_from_level(level, id)))
            .collect();
    }

    /// ROM `set_restart_position_l` (DSTRATS.ASM:1805) — snapshot the map
    /// cursor, current BG + last palette fade, and the map call/loop stacks.
    pub fn set_restart_position(&mut self, mapptr: u16, currentbg: u16, last_palfade: u16) {
        self.restart.mapptr = mapptr;
        self.restart.bg = currentbg;
        self.restart.palfade = last_palfade;
        self.restart.jsr_stack = self.jsr_stack;
        self.restart.jsr_top = self.jsr_top;
        self.restart.num_jsr = self.num_jsr;
        self.restart.loop_addrs = self.loop_addrs;
        self.restart.loop_counts = self.loop_counts;
        self.restart.num_loops = self.num_loops;
    }

    /// ROM `restart_l` (DSTRATS.ASM:1845) — restore a prior
    /// [`Self::set_restart_position`] snapshot into the live map VM.
    pub fn apply_restart(&mut self, mapptr: &mut u16) -> (u16, u16) {
        *mapptr = self.restart.mapptr;
        self.jsr_stack = self.restart.jsr_stack;
        self.jsr_top = self.restart.jsr_top;
        self.num_jsr = self.restart.num_jsr;
        self.loop_addrs = self.restart.loop_addrs;
        self.loop_counts = self.restart.loop_counts;
        self.num_loops = self.restart.num_loops;
        (self.restart.bg, self.restart.palfade)
    }
}

// ============================================================
// Builtin spacebar strategies (C src/game/world.c:177-325).
// These are the only strategies the world lane owns; everything else
// arrives with sf-strat.
// ============================================================

/// C `world_set_spacebar_hardvars` (world.c:177).
fn set_spacebar_hardvars(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.collflags |= ACF_COLLTYPE2; // ROM ENEMY1
    al.hp = HARD_HP;
    al.ap = HARD_AP;
}

/// ROM `s_add_playerZ` stand-in for spacebar2 only (GA2STRAT.ASM:1611).
/// Plain spacebar / SPINspacebar / spacebar1 have no playerZ add.
fn spacebar_scroll(g: &mut Game, idx: u16) {
    let v = g.vars.pviewvelz;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(v);
}

/// ROM `Achase_var2A` / `sr8_achase_alvarN` — shared with strat lane.
fn achase_angle(current: &mut u8, target: u8, shift: u32) -> bool {
    crate::trig8::achase_angle_8(current, target, shift)
}

/// ROM `spacebar3` local XY park via `rotate_16xz_l` (GA2STRAT.ASM:1632):
/// `x1=sword1`, `z1=sword2` (rel Y), angle=`nega(parent.rotz)`; `x2→worldx`,
/// `z2→worldy`.
fn rotate_spacebar3_offset(rel_x: i16, rel_y: i16, parent_rotz: u8) -> (i16, i16) {
    let angle = parent_rotz.wrapping_neg();
    crate::trig8::rotate_16xz(angle, rel_x, rel_y)
}

/// ROM `spacebar_strat` (GA2STRAT.ASM:1528) — `s_spacemist` only (no playerZ).
fn spacebar_strat(g: &mut Game, idx: u16) {
    spacebar_apply_spacemist(g, idx);
}

/// ROM `SPINspacebar_strat` (GA2STRAT.ASM:1540): spacemist + rotz+=sbyte1 +
/// achase roty→0 (no playerZ).
fn spinspacebar_strat(g: &mut Game, idx: u16) {
    spacebar_apply_spacemist(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(al.sbyte1);
        let mut roty = al.roty;
        achase_angle(&mut roty, 0, 3);
        al.roty = roty;
    }
}

/// ROM `spacebar1_strat` (GA2STRAT.ASM:1559): rotz+=sbyte1 + spacemist
/// (`s_add_playerZ` is commented out in ROM).
fn spacebar1_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(al.sbyte1);
    }
    spacebar_apply_spacemist(g, idx);
}

/// ROM byte-mode half-length conversion turns 250 into signed **−6**.
/// (W-mode Roffs is rejected by STRATMAC; large lengths need BYTE+SHIFT.)
use crate::trig8::XSPACEBAR_HALF_B;

/// Elevation angle from `src` → `dst` (`Xanglexy_l`): atan2(dy, xzdiffs).
fn xangle_xy(src: &crate::alien::Alien, dst: &crate::alien::Alien) -> u8 {
    sf_core::aim_angle::xanglexy(
        dst.worldy.wrapping_sub(src.worldy),
        dst.worldx.wrapping_sub(src.worldx),
        dst.worldz.wrapping_sub(src.worldz),
    )
}

/// `s_spacemist` — depth-bucket colframe (same as spacebarshoot).
fn spacebar_apply_spacemist(g: &mut Game, idx: u16) {
    // WRAM pviewposz @ $0540 (camera.rs SV_PVIEWPOSZ).
    let pvz = g.vars.strategy.player_view_position[2];
    let al = &mut g.objs.aliens[idx as usize];
    let bucket = ((al.worldz as i32 - pvz as i32 + 500) >> 9) as i16;
    let mut frame = bucket as u8;
    if (frame.wrapping_sub(8) as i8) >= 0 {
        frame = 7;
    }
    al.colframe = 0x80 | frame;
}

/// ROM `spacebar2_strat` (GA2STRAT.ASM:1586-1616): follow parent tip, set
/// rotz from elevation to the tip, place self at tip+half-bar, scroll+mist.
///
/// Both Roffs use flags **0,0,1** (rotz / `rotate_8yx` only) with B-mode
/// `#Xspacebarlen/2` → i8 −6. First ROT=parent; second ROT=self (after elev).
fn spacebar2_strat(g: &mut Game, idx: u16) {
    let parent_ptr = g.objs.aliens[idx as usize].ptr;
    if let Some(p) = obj_from_ptr(g, parent_ptr).filter(|&p| g.objs.aliens[p as usize].active) {
        // Push parent world pos, move parent to its tip, XangleXY(self→tip),
        // place self at tip+half, then restore parent.
        let (px, py, pz, protz) = {
            let par = &g.objs.aliens[p as usize];
            (par.worldx, par.worldy, par.worldz, par.rotz)
        };
        // s_add_Roffs2pos B,y,y,y,#Xspacebarlen/2,#0,#0,0,0,1
        let (tx, ty, tz) = crate::trig8::strat_roffs_roll(protz, XSPACEBAR_HALF_B, 0, 0);
        let tip_x = px.wrapping_add(tx);
        let tip_y = py.wrapping_add(ty);
        let tip_z = pz.wrapping_add(tz);
        // Temporarily write tip into parent for XangleXY(src=self, dst=parent).
        {
            let par = &mut g.objs.aliens[p as usize];
            par.worldx = tip_x;
            par.worldy = tip_y;
            par.worldz = tip_z;
        }
        let elev = {
            let me = &g.objs.aliens[idx as usize];
            let tip = &g.objs.aliens[p as usize];
            xangle_xy(me, tip)
        };
        g.objs.aliens[idx as usize].rotz = elev;
        // s_add_Roffs2pos B,x,y,x,#Xspacebarlen/2,#0,#0,0,0,1 — ROT=self.rotz
        let srotz = g.objs.aliens[idx as usize].rotz;
        let (ox, oy, oz) = crate::trig8::strat_roffs_roll(srotz, XSPACEBAR_HALF_B, 0, 0);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldx = tip_x.wrapping_add(ox);
            al.worldy = tip_y.wrapping_add(oy);
            al.worldz = tip_z.wrapping_add(oz);
        }
        // Restore parent.
        {
            let par = &mut g.objs.aliens[p as usize];
            par.worldx = px;
            par.worldy = py;
            par.worldz = pz;
        }
    }
    spacebar_scroll(g, idx);
    spacebar_apply_spacemist(g, idx);
}

/// ROM `spacebar2_Istrat` (GA2STRAT.ASM:1582-1585).
fn istrat_spacebar2_init(g: &mut Game, idx: u16) {
    set_spacebar_hardvars(g, idx);
    let sid = g.world.sid_spacebar2;
    g.objs.aliens[idx as usize].stratptr = Some(sid);
}

/// Public entry for tests / make_obj callers (no map IS row).
pub fn spacebar2_istrat(g: &mut Game, idx: u16) {
    istrat_spacebar2_init(g, idx);
}

/// Public tick for tests.
pub fn spacebar2_strat_pub(g: &mut Game, idx: u16) {
    spacebar2_strat(g, idx);
}

/// C `world_obj_from_ptr` (world.c:207) — index+1 "pointer" decode.
fn obj_from_ptr(g: &Game, ptr: u16) -> Option<u16> {
    if ptr == 0 {
        return None;
    }
    let idx = ptr as i32 - 1;
    g.objs.get(idx)?;
    Some(idx as u16)
}

/// ROM `spacebar3_strat` (GA2STRAT.ASM:1632-1667).
fn spacebar3_strat(g: &mut Game, idx: u16) {
    let parent_ptr = g.objs.aliens[idx as usize].ptr;
    let parent = obj_from_ptr(g, parent_ptr).filter(|&p| g.objs.aliens[p as usize].active);
    if let Some(p) = parent {
        let par = g.objs.aliens[p as usize];
        let (sw1, sw2, imm) = {
            let al = &g.objs.aliens[idx as usize];
            (al.sword1, al.sword2, al.immuneptr)
        };
        // rotate_16xz(nega(parent.rotz), sword1, sword2) → (x2, z2→worldy)
        let (rx, ry) = rotate_spacebar3_offset(sw1, sw2, par.rotz);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldx = par.worldx.wrapping_add(rx);
            al.worldy = par.worldy.wrapping_add(ry);
            al.worldz = par.worldz.wrapping_add(imm as i16);
            al.rotz = al.rotz.wrapping_add(par.sbyte1);
        }
    }
    spacebar_apply_spacemist(g, idx);
}

/// Public tick for tests.
pub fn spacebar3_strat_pub(g: &mut Game, idx: u16) {
    spacebar3_strat(g, idx);
}

/// Public init for tests.
pub fn spacebar3_istrat(g: &mut Game, idx: u16) {
    istrat_spacebar3_init(g, idx);
}

/// C `world_istrat_spacebar_init` (world.c:277).
fn istrat_spacebar_init(g: &mut Game, idx: u16) {
    set_spacebar_hardvars(g, idx);
    let sid = g.world.sid_spacebar;
    g.objs.aliens[idx as usize].stratptr = Some(sid);
}

/// C `world_istrat_spinspacebar_init` (world.c:284).
fn istrat_spinspacebar_init(g: &mut Game, idx: u16) {
    set_spacebar_hardvars(g, idx);
    let sid = g.world.sid_spinspacebar;
    g.objs.aliens[idx as usize].stratptr = Some(sid);
}

/// C `world_istrat_spacebar1_init` (world.c:291).
fn istrat_spacebar1_init(g: &mut Game, idx: u16) {
    set_spacebar_hardvars(g, idx);
    let sid = g.world.sid_spacebar1;
    let al = &mut g.objs.aliens[idx as usize];
    if al.sbyte2 != 0 {
        al.sflags |= crate::alien::ASF_COLLDISABLE;
    }
    al.stratptr = Some(sid);
}

/// C `world_istrat_spacebar3_init` (world.c:303).
fn istrat_spacebar3_init(g: &mut Game, idx: u16) {
    set_spacebar_hardvars(g, idx);
    let parent_ptr = g.objs.aliens[idx as usize].ptr;
    let parent = obj_from_ptr(g, parent_ptr).filter(|&p| g.objs.aliens[p as usize].active);
    if let Some(p) = parent {
        let par = g.objs.aliens[p as usize];
        let al = &mut g.objs.aliens[idx as usize];
        al.sword1 = al.worldx.wrapping_sub(par.worldx);
        al.sword2 = al.worldy.wrapping_sub(par.worldy);
        al.immuneptr = al.worldz.wrapping_sub(par.worldz) as u16;
    }
    let sid = g.world.sid_spacebar3;
    g.objs.aliens[idx as usize].stratptr = Some(sid);
}
