//! Map executor state: strategy/callback registries, level loading,
//! builtin spacebar istrats, shape-word resolution.
//!
//! C oracle: `src/game/world.c` (state + registration half; the opcode
//! dispatcher `map_exec` itself lives in [`crate::game`]) and
//! `src/map/map_exec.c` (`MapExec_LoadLevel` facade).

use crate::alien::StratId;
use crate::game::{Game, StrategyFn};
use crate::vars::{COLLTYPE_ENEMY1, HARD_AP, HARD_HP};
use sf_map::levels::{BuiltLevel, InlineCallback, NativeCallback};

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
    pub const RESERVED: u8 = 136;
    pub const WAIT2: u8 = 138;
    pub const SETPATH: u8 = 140;
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
pub const MAP_ISTRAT_SPACEBAR: usize = 166;
pub const MAP_ISTRAT_SPINSPACEBAR: usize = 167;
pub const MAP_ISTRAT_SPACEBAR1: usize = 168;
pub const MAP_ISTRAT_SPACEBAR3: usize = 169;

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
    /// C `level_scramble_keep_player_strat` (src/map/levels.c:1690).
    LevelScrambleKeepPlayerStrat,
    /// C `level1_1_skillfly_bonus_guard` (src/map/levels.c:1671).
    SkillflyGuard { skip_ptr: u16 },
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

/// C `Shapes_ResolveShapeWord()` (src/renderer/shapes.c:452) — canonicalize
/// live raw 16-bit shape words into flat runtime ids.
/// TODO(consolidation): belongs to the sf-render lane; local literal copy
/// until that crate exposes it.
pub fn resolve_shape_word(shape_id: u16) -> u16 {
    match shape_id {
        241 => 56,  // RAW_SHAPE_BOSS_7_1 -> SHAPE_BOSS7_1
        278 => 278, // SHAPE_ALIAS_MOTHER1
        551 => 508, // SHAPE_ALIAS_OP_0
        552 => 509, // SHAPE_ALIAS_OP_1
        553 => 510, // SHAPE_ALIAS_OP_2
        557 => 282, // SH_MY_BIRD -> SHAPE_EXT_MY_BIRD
        554 => 2,   // RAW_SHAPE_IMYSHIP_4 -> SHAPE_MYSHIP_4
        other => other,
    }
}

/// Map executor state (C file-statics of `src/game/world.c` plus the
/// exported `g_lastplayz`/`g_lastzchange`/`g_lastmapobj`/... globals).
pub struct World {
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

    /// C `s_native_callbacks` — level-registered entries only; world.c
    /// builtins are matched statically in [`Game::find_native_callback`].
    pub native_cbs: Vec<(u32, NativeCallback)>,
    /// C `s_inline_map_funcs` (script ptr -> inline callback).
    pub inline_cbs: Vec<(u16, InlineCb)>,
    /// C `s_strat_addr_maps` (24-bit strategy address -> registry id).
    pub strat_addrs: Vec<(u32, StratId)>,

    /// C `g_istrats[]` — strategy id -> registry handle (None = unported,
    /// object stays inert).
    pub istrats: [Option<StratId>; ISTRAT_CAPACITY],
    /// C `g_istrat_shapes[]` — shape associated with each strategy
    /// (all zero until the strat lane ports its table).
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
    pub sid_spacebar3: StratId,
}

impl World {
    /// C `World_Init()` (src/game/world.c:833): clear all executor state,
    /// register builtin callbacks/istrats, seed the shape table.
    pub fn init() -> Self {
        let mut w = World {
            map: Vec::new(),
            map_loaded: false,
            lastplayz: 0,
            lastzchange: 0,
            last_obj: None,
            lastmapobj: 0,
            specialobjtotal: 0,
            levelfinished: 0,
            jsr_stack: [0; MAP_JSR_STACK_SIZE],
            jsr_top: 0,
            num_jsr: 0,
            loop_addrs: [0; MAP_MAX_LOOPS],
            loop_counts: [0; MAP_MAX_LOOPS],
            num_loops: 0,
            native_cbs: Vec::new(),
            inline_cbs: Vec::new(),
            strat_addrs: Vec::new(),
            istrats: [None; ISTRAT_CAPACITY],
            istrat_shapes: [0; ISTRAT_CAPACITY],
            shapes_table: [0; MAX_SHAPES],
            strat_registry: Vec::new(),
            sid_spacebar: StratId(0),
            sid_spinspacebar: StratId(0),
            sid_spacebar1: StratId(0),
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
        self.strat_addrs
            .iter()
            .find(|e| e.0 == addr24)
            .map(|e| e.1)
    }

    /// C `world_register_builtin_istrats()` (src/game/world.c:320) —
    /// bounded MAPMACS.INC space-bar strategies.
    fn register_builtin_istrats(&mut self) {
        self.sid_spacebar = self.register_strategy(spacebar_strat);
        self.sid_spinspacebar = self.register_strategy(spinspacebar_strat);
        self.sid_spacebar1 = self.register_strategy(spacebar1_strat);
        self.sid_spacebar3 = self.register_strategy(spacebar3_strat);
        let init_spacebar = self.register_strategy(istrat_spacebar_init);
        let init_spin = self.register_strategy(istrat_spinspacebar_init);
        let init_sb1 = self.register_strategy(istrat_spacebar1_init);
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
            InlineCallback::LevelScrambleKeepPlayerStrat => {
                InlineCb::LevelScrambleKeepPlayerStrat
            }
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
}

// ============================================================
// Builtin spacebar strategies (C src/game/world.c:177-325).
// These are the only strategies the world lane owns; everything else
// arrives with sf-strat.
// ============================================================

/// C `world_set_spacebar_hardvars` (world.c:177).
fn set_spacebar_hardvars(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.collflags |= COLLTYPE_ENEMY1;
    al.hp = HARD_HP;
    al.ap = HARD_AP;
}

/// C `world_spacebar_scroll` (world.c:186).
fn spacebar_scroll(g: &mut Game, idx: u16) {
    let v = g.vars.pviewvelz;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(v);
}

/// C `world_achase_angle` (world.c:193).
fn achase_angle(current: &mut u8, target: u8, shift: u32) -> bool {
    if *current == target {
        return true;
    }
    let diff = current.wrapping_sub(target) as i8;
    let mut step = diff >> shift;
    if step == 0 {
        step = if diff > 0 { 1 } else { -1 };
    }
    *current = current.wrapping_sub(step as u8);
    *current == target
}

/// C `world_rotate_spacebar_offset` (world.c:214).
/// NOTE: float trig — bit-parity with C depends on libm cosf/sinf; the
/// fixture levels do not exercise spacebar3, so this is covered by unit
/// tests only.
fn rotate_spacebar_offset(x: i16, y: i16, rotz: u8) -> (i16, i16) {
    let radians = rotz as f32 * (2.0 * 3.141_592_65 / 256.0);
    let (sin_v, cos_v) = (radians.sin(), radians.cos());
    let (xf, yf) = (x as f32, y as f32);
    (
        (xf * cos_v - yf * sin_v) as i16,
        (xf * sin_v + yf * cos_v) as i16,
    )
}

/// C `world_spacebar_strat` (world.c:230).
fn spacebar_strat(g: &mut Game, idx: u16) {
    spacebar_scroll(g, idx);
}

/// C `world_spinspacebar_strat` (world.c:234).
fn spinspacebar_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(al.sbyte1);
        let mut roty = al.roty;
        achase_angle(&mut roty, 0, 3);
        al.roty = roty;
    }
    spacebar_scroll(g, idx);
}

/// C `world_spacebar1_strat` (world.c:244).
fn spacebar1_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(al.sbyte1);
    }
    spacebar_scroll(g, idx);
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

/// C `world_spacebar3_strat` (world.c:253).
fn spacebar3_strat(g: &mut Game, idx: u16) {
    let parent_ptr = g.objs.aliens[idx as usize].ptr;
    let parent = obj_from_ptr(g, parent_ptr)
        .filter(|&p| g.objs.aliens[p as usize].active);
    let Some(p) = parent else {
        spacebar_scroll(g, idx);
        return;
    };
    let par = g.objs.aliens[p as usize];
    let (sw1, sw2, imm) = {
        let al = &g.objs.aliens[idx as usize];
        (al.sword1, al.sword2, al.immuneptr)
    };
    let (rx, ry) = rotate_spacebar_offset(sw1, sw2, 0u8.wrapping_sub(par.rotz));
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = par.worldx.wrapping_add(rx);
    al.worldy = par.worldy.wrapping_add(ry);
    al.worldz = par.worldz.wrapping_add(imm as i16);
    al.rotz = al.rotz.wrapping_add(par.sbyte1);
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
    let parent = obj_from_ptr(g, parent_ptr)
        .filter(|&p| g.objs.aliens[p as usize].active);
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
