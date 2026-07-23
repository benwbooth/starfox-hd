//! MAP_ID_1_6 — Venom 1 Surface (Route 1 final level).
//!
//! Direct transcription of the original map assembly.
//!
//! ASM sources transcribed (via the C port):
//! - `LEVEL1_6.ASM` — level wrapper: `initlevel 1_6a,0`, jsr map1_6a,
//!   then `incmap finalmap`.
//! - `FINALMAP.ASM` — Andross final tunnel & boss (`level1_6.final.*`).
//! - `MAP1_6A.ASM`  — Venom 1 surface content.

use super::Route1Level;
use crate::builder::{BarShapeMode, MapBuilder};
use crate::consts::*;
use crate::levels::BuiltLevel;

// Local constants from levels.c not yet in consts.rs.
// TODO(consolidation): move to consts.rs
mod lc {
    // ---- MAP1_6A.ASM symbols (Venom 1 surface part A) ----
    // Shape ids (ISTRATS.ASM def_shape MACRO-counted numbering; verified via
    // tools/shape_compiler.py parse of ISTRATS.ASM).
    /// def_shape wall1 (ISTRATS.ASM:147) = id 25.
    pub const SH_WALL1: u16 = 24;
    /// def_shape r_bu_0 (ISTRATS.ASM:222) = id 96.
    pub const SH_R_BU_0: u16 = 95;
    /// def_shape r_bu_2 (ISTRATS.ASM:224) = id 98.
    pub const SH_R_BU_2: u16 = 97;
    /// def_shape hou_5 (ISTRATS.ASM:618-area) = id 169 (== rc.rs SH_HOU_5).
    pub const SH_HOU_5: u16 = 168;
    pub const SH_RPILLAR3_PROXY: u16 = 439;
    /// mother1 has no ASM geometry; extended-bank alias slot 278
    /// (blackhole.rs `SH_MOTHER1`).
    pub const SH_MOTHER1: u16 = 278;
    /// Canonical ISTRATS.ASM `boss_b_1` def_shape row.  The generated Rust
    /// catalog contains this mesh; the old nullshape proxy made the Andross
    /// robot fight entirely invisible.
    pub const SH_BOSS_B_1_PROXY: u16 = 76;

    // Strategy indices (must equal the index the sf-strat lane REGISTERS at; see
    // docs/istrat_index_map.tsv header — raw ASM rows are only a rough guide).
    /// walll = ISTRATS.ASM row 76; sf-strat enemies_ground::IS_WALLL = 76.
    pub const IS_WALLL: u32 = 75;
    /// wallr = ISTRATS.ASM row 77; sf-strat enemies_ground::IS_WALLR = 77.
    pub const IS_WALLR: u32 = 76;
    /// houdai5f = ISTRATS.ASM:618 index 188 (after hard90yrfog@183 + tanks).
    pub const IS_HOUDAI5F: u32 = 187;
    /// flypillars: the C oracle aliased flypillar_istrat to IS_PILLAR3 (=79);
    /// sf-strat leaves 74 unregistered and runs pillar3 behaviour here
    /// (route3/common.rs `IS_FLYPILLARS = 79`).
    pub const IS_FLYPILLARS: u32 = 73;
    /// bossBrob robot = def_Istrat 118 (ISTRATS.ASM:542); sf-strat bossb.rs
    /// registers world.istrats[118] = bossbrob_init, and the address-map loop
    /// mints flat-id 0x000076 — so the compact MAPOBJ strat byte 118 resolves.
    pub const IS_BOSSBROB: u32 = 117;

    // Path ids (sf-path ids.rs PATH_ID_*).
    pub const PATH_CHASE7_1: u16 = 243;
    pub const PATH_CHASE7_2: u16 = 244;
    pub const PATH_E_DOSUN: u16 = 356;
    pub const PATH_ITADOSUN: u16 = 357;

    /// MAP1_6A.ASM:24 `speed = 30` — the SBtype16/17 shot velocity.
    pub const SPEED: i32 = 30;
}

/// C `build_level1_6_slice()` + `register_level1_6_inline_callbacks()`.
pub fn build() -> Route1Level {
    let mut b = MapBuilder::new();

    // MAP1_6A boss (bossBrob) mapwaitboss inline hooks — captured below.
    let mut map1_6a_trigse_ptr: u16 = 0;
    let mut map1_6a_cantdie_ptr: u16 = 0;
    let mut map1_6a_cleanup_ptr: u16 = 0;

    // LEVEL1_6.ASM: initlevel 1_6a,0
    // mapjsr map1_6a — Route 1 Venom surface content (MAP1_6A.ASM)
    b.mapjsr("level1_6.map1_6a");

    // level1_end: incmap finalmap — Andross final tunnel & boss
    let (mapwaitboss_cantdie_ptr, mapwaitboss_cleanup_ptr) =
        crate::levels::route3::common::append_finalmap_content(&mut b, "level1_6.final", 1);

    // ---- MAP1_6A.ASM subroutine (Venom 1 surface part A) ----
    // Emitted AFTER the finalmap content, so the finalmap inline hooks (758/759)
    // keep their offsets; this only appends. The mapjsr above enters at the
    // `level1_6.map1_6a` label.
    append_map1_6a_content(
        &mut b,
        "level1_6",
        &mut map1_6a_trigse_ptr,
        &mut map1_6a_cantdie_ptr,
        &mut map1_6a_cleanup_ptr,
    );

    b.resolve();

    let (data, labels) = b.finish();

    // C `register_level1_6_inline_callbacks()` — registration-call order,
    // guarded by ptr != 0 like C.
    let mut inline_regs: Vec<(u16, &'static str)> = Vec::new();
    for (ptr, name) in [
        (mapwaitboss_cantdie_ptr, "level1_1_mapwaitboss_cantdie"),
        (mapwaitboss_cleanup_ptr, "level1_1_mapwaitboss_cleanup"),
        // MAP1_6A bossBrob `mapwaitboss` (with sound) reuses the shared
        // level1_1_* closures, same as level1_5 / level3_6.
        (map1_6a_trigse_ptr, "level1_1_mapwaitboss_trigse"),
        (map1_6a_cantdie_ptr, "level1_1_mapwaitboss_cantdie"),
        (map1_6a_cleanup_ptr, "level1_1_mapwaitboss_cleanup"),
    ] {
        if ptr != 0 {
            inline_regs.push((ptr, name));
        }
    }

    Route1Level {
        level: BuiltLevel {
            data,
            labels,
            native_callbacks: vec![],
            inline_callbacks: vec![],
        },
        native_regs: vec![],
        inline_regs,
    }
}

/// MAP1_6A.ASM — Venom 1 Surface part A (Route 1 final level content).
///
/// Faithful transcription of `reference/ultrastarfox/SF/MAPS/MAP1_6A.ASM`.
/// Ends with the Andross robot (`bossBrob`) spawn + `mapwaitboss`.
///
/// NUMBER RADIX: MAP1_6A's coordinate/frame literals are read as DECIMAL with
/// leading zeros as column-alignment padding (`0200`=200, `-0125`=-125,
/// `0400`=400). This is what planet.bin proves the C oracle did for PLANET.ASM
/// (`-1000`,`0400`→400, etc.), and it matches the `mapwait` magnitudes
/// (`mapwait 0100`=100, not 0x100). The one `incmap planet` block below is
/// replicated byte-for-byte from `planet.rs` (shared source → shared bytes).
///
/// The three `&mut` ptr outs receive the `mapwaitboss` (with-sound) inline
/// CODE65816 script ptrs (trigse / cantdie / cleanup), registered by the
/// caller against the shared `level1_1_mapwaitboss_*` closures.
fn append_map1_6a_content(
    b: &mut MapBuilder,
    prefix: &str,
    trigse_ptr: &mut u16,
    cantdie_ptr: &mut u16,
    cleanup_ptr: &mut u16,
) {
    let mm = crate::mothers::mother_maps();

    // Lines 2-4: restart1_6 — set_restart target (SETRESTART_L only sets a flag
    // in this port; the label is emitted for byte fidelity + the mapgoto).
    b.label(&format!("{prefix}.restart1_6"));
    b.mapwait(2000);
    b.mapgoto(&format!("{prefix}.cont1_6"));

    // Line 6: map1_6a — the mapjsr entry point.
    b.label(&format!("{prefix}.map1_6a"));

    // Line 7: `incmap planet` — inlines PLANET.ASM (mapnozremove is a renderer
    // flag, no opcode; see planet.rs). Replicated verbatim from planet.rs so
    // both PLANET.ASM transcriptions stay byte-identical.
    b.mapobj(0, 0x0220, -1000, -200, sh::R_BU_4, is::HARD);
    b.mapobj(0, 0x0220, -500, -200, sh::R_BU_4, is::HARD);
    b.mapobj(0, 0x0220, -10, -200, sh::R_BU_4, is::HARD);
    b.mapobj(0, -500, 0, 400, sh::BU_6, is::HARD);
    b.setalvarb(al::ROTY, -64);
    b.mapobj(0, 500, 0, 400, sh::BU_6, is::HARD);
    b.setalvarb(al::ROTY, 64);
    b.mapobj(0, 800, 0, -300, sh::BU_0, is::HARD);
    b.mapobj(0, -800, 0, -300, sh::BU_0, is::HARD);
    b.mapobj(0, -300, 0, -800, sh::BU_2, is::HARD);
    b.mapobj(0, 300, 0, -800, sh::BU_2, is::HARD);
    b.mapobj(0, -0x0220, -1000, -200, sh::R_BU_4, is::HARD);
    b.mapobj(0, -0x0220, -500, -200, sh::R_BU_4, is::HARD);
    b.mapobj(0x0200, -0x0220, -10, -200, sh::R_BU_4, is::HARD);
    b.mapobj(0, -200, -300, 600, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0200, 200, -300, 600, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0, 200, -300, 800, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0200, -200, -300, 800, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0, 180, -250, 1000, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0400, -180, -250, 1000, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0, 150, -200, 1000, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0300, -150, -200, 1000, sh::R_BU_7, is::HARD180YR);

    // Lines 9-16: r_bu_7 intro obstacle course (8 alternating buildings).
    for _ in 0..4 {
        b.mapobj(0, 200, -125, 2000, sh::R_BU_7, is::HARD180YR);
        b.mapobj(400, -200, -125, 2000, sh::R_BU_7, is::HARD180YR);
    }

    // Line 18: setrestart restart1_6.
    b.mapcodejsl_builtin(cb::SETRESTART_L);

    // Line 19: cont1_6 — restart re-entry point.
    b.label(&format!("{prefix}.cont1_6"));

    // Lines 21-30: bu_6 pair, solid bars, three shooting SBtype16 bars.
    b.mapobj(0, -500, 0, 4000, sh::BU_6, is::HARD180YR);
    b.mapobj(400, 500, 0, 4000, sh::BU_6, is::HARD180YR);
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.map_sbtype16(4, -10, -1, 0, lc::SPEED, 0);
    b.map_sbtype16(0, 10, -2, 0, -lc::SPEED, 0);
    b.map_sbtype16(4, 10, 0, 0, -lc::SPEED, 0);
    b.mapwait(800);

    // Lines 33-38: bu_0 gate pair + wall + R_BU_0 pair.
    b.mapobj(0, -600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(0, 600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(3400, 0, 0, 4200, lc::SH_WALL1, lc::IS_WALLR);
    b.mapobj(0, -400, -20, 5000, lc::SH_R_BU_0, is::HARD180YR);
    b.mapobj(800, 400, -20, 5000, lc::SH_R_BU_0, is::HARD180YR);

    // Lines 40-57: R_BU_1 diagonal wall (mapblocksnd before each), zaco patrol.
    b.mapcodejsl_builtin(cb::BLOCKSND_L);
    b.mapobj(100, -100, -50, 1000, sh::R_BU_1, is::HARD180YR);
    for (x, y) in [
        (0, -150),
        (100, -250),
        (200, -350),
        (300, -450),
        (350, -350),
        (400, -250),
        (450, -150),
    ] {
        b.mapcodejsl_builtin(cb::BLOCKSND_L);
        b.mapobj(100, x, y, 1000, sh::R_BU_1, is::HARD180YR);
    }
    b.pathspecial(0, 1000, -600, 2000, sh::ZACO_A, path::PATROL, 10, 10);
    b.mapobj(300, 500, -50, 1000, sh::R_BU_1, is::HARD180YR);

    // Lines 60-77: second R_BU_1 diagonal.
    for (x, y) in [
        (300, -50),
        (200, -150),
        (100, -250),
        (50, -350),
        (0, -450),
        (-50, -350),
        (-150, -250),
        (-250, -150),
    ] {
        b.mapcodejsl_builtin(cb::BLOCKSND_L);
        b.mapobj(100, x, y, 1000, sh::R_BU_1, is::HARD180YR);
    }
    b.mapobj(400, -350, -50, 1000, sh::R_BU_1, is::HARD180YR);

    // Lines 79-103: paired R_BU_1 columns (frame 0 left, frame 80 right).
    b.mapcodejsl_builtin(cb::BLOCKSND_L);
    b.mapobj(0, 450, -50, 1000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(80, -450, -50, 1000, sh::R_BU_1, is::HARD180YR);
    for (yl, z) in [(350, -150), (250, -250), (150, -350)] {
        b.mapcodejsl_builtin(cb::BLOCKSND_L);
        b.mapobj(0, yl, z, 1000, sh::R_BU_1, is::HARD180YR);
        b.mapobj(80, -yl, z, 1000, sh::R_BU_1, is::HARD180YR);
    }
    b.mapcodejsl_builtin(cb::BLOCKSND_L);
    b.mapobj(0, 100, -450, 1000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(80, -100, -450, 1000, sh::R_BU_1, is::HARD180YR);
    b.mapcodejsl_builtin(cb::BLOCKSND_L);
    b.mapobj(0, 50, -350, 1000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(100, -50, -350, 1000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(100, 0, -250, 1000, sh::R_BU_1, is::HARD180YR);
    b.mapcodejsl_builtin(cb::BLOCKSND_L);
    b.mapobj(0, -100, -150, 1000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(100, 100, -150, 1000, sh::R_BU_1, is::HARD180YR);
    b.mapcodejsl_builtin(cb::BLOCKSND_L);
    b.mapobj(0, -200, -50, 1000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(0, 200, -50, 1000, sh::R_BU_1, is::HARD180YR);

    // Lines 105-106: R_BU_0 pair.
    b.mapobj(0, -400, -20, 5000, lc::SH_R_BU_0, is::HARD180YR);
    b.mapobj(800, 400, -20, 5000, lc::SH_R_BU_0, is::HARD180YR);

    // Lines 109-118: friend (friendship_4 + chase6) + zaco patrols + houdai.
    b.pathobj(0, -750, -480, 0, sh::FRIENDSHIP_4, path::CHASE6_1, 10, 10);
    b.pathcspecial(2000, -720, -480, 0, sh::ZACO_A, path::CHASE6_2, 10, 10);
    b.mapobj(1000, -600, -20, 5000, lc::SH_R_BU_0, is::HARD180YR);
    b.mapobj(2000, 500, -20, 5000, lc::SH_R_BU_0, is::HARD180YR);
    b.pathspecial(2500, 1000, -600, 2000, sh::ZACO_A, path::PATROL, 10, 10);
    b.pathspecial(2500, -1000, -400, 2000, sh::ZACO_A, path::PATROL, 10, 10);
    b.mapobj(0, -600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.cspecial(2500, 500, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.mapobj(0, 600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.cspecial(2500, -700, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);

    // Lines 120-134: moving walls section.
    b.mapobj(2000, -600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(2000, 600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(0, -700, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(1000, 100, 0, 4200, lc::SH_WALL1, lc::IS_WALLR);
    b.mapobj(2800, 500, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(0, 150, -50, 4500, sh::ITEM_5, is::ITEM5);
    b.setalvarb(al::SBYTE1, 1);
    b.mapobj(0, 500, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(0, -700, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(2500, -100, 0, 4200, lc::SH_WALL1, lc::IS_WALLR);
    b.mapobj(0, -500, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(1000, 100, 0, 4200, lc::SH_WALL1, lc::IS_WALLL);
    b.mapobj(1000, 700, 0, 4000, sh::BU_0, is::HARD180YR);

    // Lines 136-145: shooting bars + walls.
    b.map_sbtype16(0, 10, -4, 0, -lc::SPEED, 0);
    b.map_sbtype16(5, -10, -3, 0, lc::SPEED, 0);
    b.map_sbtype16(0, 10, -4, 0, -lc::SPEED, 0);
    b.map_sbtype16(5, -10, -3, 0, lc::SPEED, 0);
    b.mapwait(100);
    b.mapobj(1500, -300, 0, 4000, lc::SH_WALL1, lc::IS_WALLR);
    b.mapobj(1500, 300, 0, 4200, lc::SH_WALL1, lc::IS_WALLL);
    b.mapobj(0, 600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(1500, 0, 0, 4200, lc::SH_WALL1, lc::IS_WALLL);
    b.mapobj(1400, -600, 0, 4000, sh::BU_0, is::HARD180YR);

    // Lines 147-152: six shooting bars.
    b.map_sbtype16(0, 10, -4, 0, -lc::SPEED, 0);
    b.map_sbtype16(5, -10, -3, 0, lc::SPEED, 0);
    b.map_sbtype16(0, 10, -4, 0, -lc::SPEED, 0);
    b.map_sbtype16(5, -10, -3, 0, lc::SPEED, 0);
    b.map_sbtype16(0, 10, -4, 0, -lc::SPEED, 0);
    b.map_sbtype16(4, -10, -3, 0, lc::SPEED, 0);

    // Lines 155-156: exact extending-pillar mother stream.
    b.mapmother(
        3000,
        0,
        0,
        1500,
        lc::SH_MOTHER1,
        STRATEGY_MOTHER1,
        mm.map_pillars,
    );
    b.mapremove(lc::SH_MOTHER1);

    // Lines 158-180: zaco patrols, friend (chase7), houdai emplacements.
    b.pathspecial(500, -1000, -700, 2000, sh::ZACO_A, path::PATROL, 10, 10);
    b.pathspecial(2500, -1000, -450, 2000, sh::ZACO_A, path::PATROL, 10, 10);
    b.pathobj(
        0,
        0,
        -400,
        -150,
        sh::FRIENDSHIP_4,
        lc::PATH_CHASE7_1,
        10,
        10,
    );
    b.pathcspecial(2000, 0, -400, -150, sh::ZACO_A, lc::PATH_CHASE7_2, 10, 10);
    b.mapobj(1000, -400, -20, 5000, lc::SH_R_BU_0, is::HARD180YR);
    b.mapobj(1000, 600, -20, 5000, lc::SH_R_BU_0, is::HARD180YR);
    b.mapobj(1000, -800, -20, 5000, lc::SH_R_BU_0, is::HARD180YR);
    b.cspecial(0, 500, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.cspecial(2500, -500, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.mapobj(1000, -600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(1000, 600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(1000, -600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.pathspecial(500, 1000, -700, 2000, sh::ZACO_A, path::PATROL, 10, 10);
    b.pathspecial(2500, -1000, -450, 2000, sh::ZACO_A, path::PATROL, 10, 10);
    b.cspecial(0, 500, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.cspecial(2000, -500, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.mapobj(1500, 600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(0, -600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.cspecial(1500, 200, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.mapobj(1000, -600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(1000, 600, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(1000, -600, 0, 4000, sh::BU_0, is::HARD180YR);

    // Lines 182-197: flying pillars (rpillar3 / flypillars).
    b.mapobj(0, 800, 0, 4000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(0, -1000, 0, 5000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(0, 1200, 0, 6000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(0, -900, 0, 6000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(0, 0, 0, 3000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(0, 200, 0, 3000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(400, -200, 0, 3000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(0, 500, 0, 3000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(400, -500, 0, 3000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(0, 350, 0, 3000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(400, -350, 0, 3000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(0, 150, 0, 3000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(400, -150, 0, 3000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(0, 400, 0, 3000, lc::SH_RPILLAR3_PROXY, lc::IS_FLYPILLARS);
    b.mapobj(
        2000,
        -400,
        0,
        3000,
        lc::SH_RPILLAR3_PROXY,
        lc::IS_FLYPILLARS,
    );

    // Lines 200-202: fly-pillars mother.
    b.mapmother(
        6000,
        0,
        0,
        4000,
        lc::SH_MOTHER1,
        STRATEGY_MOTHER1,
        mm.map_flypillars,
    );
    b.mapremove(lc::SH_MOTHER1);
    b.mapwait(2000);

    // Lines 203-222: SBtype17 shooting-bar walls (three rows).
    for (wait, x) in [(0, -4), (0, -2), (0, 0), (0, 2), (6, 4)] {
        b.map_sbtype17(wait, x, -11, 0, lc::SPEED, 0);
    }
    for (wait, x) in [(0, -5), (0, -3), (0, -1), (0, 1), (0, 3), (6, 5)] {
        b.map_sbtype17(wait, x, -12, 0, lc::SPEED, 0);
    }
    for (wait, x) in [(0, -4), (0, -2), (0, 0), (0, 2), (5, 4)] {
        b.map_sbtype17(wait, x, -12, 0, lc::SPEED, 0);
    }

    // Lines 224-229: gates + e_gate path message.
    b.mapobj(0, -250, -100, 4000, sh::GATE_0, STRAT_ADDR_GATE3);
    b.mapobj(0, 250, -100, 4000, sh::GATE_0, STRAT_ADDR_GATE3);
    b.mapobj(0, 0, -200, 4000, sh::GATE_0, is::GATE);
    b.pathobj(1000, 3000, 3000, 3000, sh::NULLSHAPE, path::E_GATE, 10, 10);
    b.mapwait(2000);

    // Lines 232-246: two more SBtype17 rows + an SBtype16 triple between them.
    for (wait, x) in [(0, -4), (0, -2), (0, 0), (0, 2), (5, 4)] {
        b.map_sbtype17(wait, x, -12, 0, lc::SPEED, 0);
    }
    b.map_sbtype16(0, -10, 0, 0, lc::SPEED, 0);
    b.map_sbtype16(0, -11, -2, 0, lc::SPEED, 0);
    b.map_sbtype16(6, -12, -4, 0, lc::SPEED, 0);
    for (wait, x) in [(0, -4), (0, -2), (0, 0), (0, 2), (8, 4)] {
        b.map_sbtype17(wait, x, -12, 0, lc::SPEED, 0);
    }

    // Lines 248-276: "dossun" — r_bu_1/r_bu_2 blocks + e_dosun/itadosun paths.
    b.mapobj(0, 300, -250, 3000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(0, -300, -250, 3000, sh::R_BU_1, is::HARD180YR);
    b.pathobj(300, 0, -250, 3000, sh::R_BU_1, lc::PATH_E_DOSUN, 10, 8);
    b.mapobj(0, 150, -50, 3000, sh::ITEM_5, is::ITEM5);
    b.setalvarb(al::SBYTE1, 1);
    b.pathobj(300, 300, -250, 3000, sh::R_BU_1, lc::PATH_E_DOSUN, 10, 8);
    b.mapobj(0, 600, -150, 3000, sh::R_BU_1, is::HARD180YR);
    b.pathobj(300, -450, -300, 3000, sh::R_BU_1, lc::PATH_E_DOSUN, 10, 8);
    b.mapobj(0, 300, -250, 3000, sh::R_BU_1, is::HARD180YR);
    b.pathobj(300, -150, -250, 3000, sh::R_BU_1, lc::PATH_E_DOSUN, 10, 8);
    b.mapobj(0, -300, -150, 3000, sh::R_BU_1, is::HARD180YR);
    b.pathobj(300, 150, -350, 3000, sh::R_BU_1, lc::PATH_E_DOSUN, 10, 8);
    b.mapobj(0, -150, -250, 3000, sh::R_BU_1, is::HARD180YR);
    b.pathobj(300, 600, -250, 3000, sh::R_BU_1, lc::PATH_E_DOSUN, 10, 8);
    b.pathobj(300, 0, -250, 3000, sh::R_BU_1, lc::PATH_E_DOSUN, 10, 8);
    b.pathobj(300, -450, -350, 3000, sh::R_BU_1, lc::PATH_E_DOSUN, 10, 8);
    b.pathobj(300, 300, -150, 3000, sh::R_BU_1, lc::PATH_E_DOSUN, 10, 8);
    b.mapobj(0, 450, -250, 3000, sh::R_BU_1, is::HARD180YR);
    b.pathobj(300, -300, -250, 3000, sh::R_BU_1, lc::PATH_E_DOSUN, 10, 8);
    b.mapobj(0, -600, -350, 3000, sh::R_BU_1, is::HARD180YR);
    b.pathobj(300, 0, -250, 3000, sh::R_BU_1, lc::PATH_E_DOSUN, 10, 8);
    b.mapobj(0, 0, -50, 3800, sh::ITEM_7, is::ITEM7);
    b.setalvarb(al::SBYTE1, 1);
    b.mapobj(0, 0, -250, 3000, lc::SH_R_BU_2, is::HARD180YR);
    b.pathobj(0, 450, -350, 3000, lc::SH_R_BU_2, lc::PATH_ITADOSUN, 10, 8);
    b.pathobj(
        400,
        -450,
        -250,
        3000,
        lc::SH_R_BU_2,
        lc::PATH_ITADOSUN,
        10,
        8,
    );
    b.mapobj(0, 450, -250, 3000, lc::SH_R_BU_2, is::HARD180YR);
    b.mapobj(0, -450, -200, 3000, lc::SH_R_BU_2, is::HARD180YR);
    b.pathobj(1500, 0, -250, 3000, lc::SH_R_BU_2, lc::PATH_ITADOSUN, 10, 8);
    b.mapobj(3200, 0, 0, 4000, lc::SH_WALL1, lc::IS_WALLL);

    // Lines 279-283: final fly-pillars mother + wait.
    b.mapmother(
        8000,
        0,
        0,
        4000,
        lc::SH_MOTHER1,
        STRATEGY_MOTHER1,
        mm.map_flypillars,
    );
    b.mapremove(lc::SH_MOTHER1);
    b.mapwait(3000);

    // Lines 286-301: boss1666 — the Andross robot (bossBrob) boss.
    b.label(&format!("{prefix}.boss1666"));
    // Line 288: fadeoutbgm = setbgm $f1 + mapwait medpspeed*30.
    b.setbgm(BGM_FADEOUT);
    b.mapwait(MEDPSPEED * 30);
    // Line 289: setbgm 5 (boss music).
    b.setbgm(BGM_BOSS1);
    // Line 292: mapobj boss_b_1, bossBrob_Istrat — the robot. Compact MAPOBJ
    // strat byte 118 resolves to sf-strat world.istrats[118] = bossbrob_init.
    b.mapobj(0, 0, -1000, 4000, lc::SH_BOSS_B_1_PROXY, lc::IS_BOSSBROB);

    // Line 293: mapwaitboss (with sound) — trigse / poll chkbossdead / cantdie /
    // cleanup / setbgm $f1, mirroring level1_5.rs + level3_6.rs.
    *trigse_ptr = b.mapcode65816_inline();
    b.label(&format!("{prefix}.bossbrob.loop"));
    b.mapif_builtin(cb::CHKBOSSDEAD, &format!("{prefix}.bossbrob.cont"));
    b.mapgoto(&format!("{prefix}.bossbrob.loop"));
    b.label(&format!("{prefix}.bossbrob.cont"));
    *cantdie_ptr = b.mapcode65816_inline();
    *cleanup_ptr = b.mapcode65816_inline();
    b.setbgm(BGM_FADEOUT);

    // Line 294: markboss boss16.
    b.mapcodejsl_builtin(cb::MARKBOSS_L);
    // Lines 295-300: the original condition is assembled only when
    // `hidehudonbossdeath` is enabled; this build uses that enabled variant.
    b.setvarb24(wm::M_METERS, 1);
    // Line 301: maprts.
    b.maprts();
}
