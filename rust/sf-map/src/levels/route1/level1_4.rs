//! MAP_ID_1_4 — Asteroid Belt 2 / Macbeth (Level 4, Route 1).
//!
//! C oracle: `src/map/levels.c` `build_level1_4_wrapper_slice()`
//! (lines 8969-9255) + `register_level1_4_inline_callbacks()`
//! (lines 9256-9278).
//!
//! ASM sources transcribed (via the C port):
//! - `LEVEL1_4.ASM` — level wrapper: init, mapjsr map1_4, exit gro_6 trio,
//!   cl_ground clear demo, mapend.
//! - `MAP1_4.ASM`   — the full Macbeth run + boss_h_0 boss block.
//! - `CL_GND.ASM`   — shared ground clear demo (`cl_ground` label).

use super::Route1Level;
use crate::builder::MapBuilder;
use crate::consts::*;
use crate::levels::BuiltLevel;

/// Level-local constants from `src/map/levels.c` #defines.
// TODO(consolidation): move to consts.rs
mod lc {
    // Shapes (levels.c SH_* block)
    pub const SH_WALKER_0: u16 = 26;
    pub const SH_HELI: u16 = 130;
    pub const SH_TANK_1: u16 = 167;
    pub const SH_HOU_5: u16 = 168;
    pub const SH_S_TANK_0: u16 = 229;
    // Exact ISTRATS.ASM def_shape rows.  These were once mapped to generic
    // BU_* buildings even though all five Macbeth terrain meshes are present
    // in the generated Rust shape catalog.
    pub const SH_GRO_0_PROXY: u16 = 183;
    pub const SH_GRO_1_PROXY: u16 = 184;
    pub const SH_GRO_4_PROXY: u16 = 187;
    pub const SH_GRO_5_PROXY: u16 = 188;
    pub const SH_GRO_6_PROXY: u16 = 189;
    pub const SH_BASE_0_0_PROXY: u16 = 165;
    pub const SH_BASE_0_1_PROXY: u16 = 166;
    pub const SH_BTANK_1_PROXY: u16 = 239;
    pub const SH_TANK_2_PROXY: u16 = 134;
    // Direct-address boss mesh: not present in ISTRATS def_shape, compiled by
    // tools/shape_compiler.py into its stable extended slot.
    pub const SH_BOSS_H_0: u16 = 300;

    // Strategies (levels.c IS_* block)
    pub const IS_WALKING: u32 = 77;
    pub const IS_UP1MAN: u32 = 89;
    /// bossH ("gggy") synthetic strategy address (sf-strat bossh::register).
    /// MAP1_4.ASM:217 places boss_h_0/bossh_istrat here — the level-1_4 boss is
    /// gggy, NOT Boss2 (which belongs at route3 3_5, MAP3_5.ASM:303). The port
    /// stubbed this to IS_BOSS2 before bossH was ported.
    pub const STRATEGY_BOSSH: crate::consts::DirectStrategy = crate::consts::DirectStrategy::BossH;
    pub const IS_BASE0: u32 = 137;
    pub const IS_TANK2: u32 = 161;
    // Exact zero-based ISTRATS.ASM row.
    pub const IS_HARD180YRFOG: u32 = 180;
    // ROM MAP1_4.ASM:156/190/191 places base_0/base1_istrat; sf-strat registers
    // `base1_istrat` is row 181; this is distinct from D3STRATS `base_1` row 229.
    pub const IS_BASE1: u32 = 181;
    pub const IS_TANK1A: u32 = 183;
    pub const IS_HOUDAI5F: u32 = 187;

    // Path ids (src/path/path_literals.h)
    pub const PATH_ID_CHASE7_1: u16 = 243;
    pub const PATH_ID_CHASE7_2: u16 = 244;
    pub const PATH_ID_E_WALK_1: u16 = 317;
    pub const PATH_ID_E_TANK: u16 = 320;
    pub const PATH_ID_KAMOME: u16 = 334;
}

/// C `build_level1_4_wrapper_slice()` + `register_level1_4_inline_callbacks()`.
pub fn build() -> Route1Level {
    let mut b = MapBuilder::new();

    // LEVEL1_4.ASM: initlevel 1_4,mscramwipe_circle
    // Generic level init approximation.
    b.mapcodejsl_builtin(cb::INITBLACK_L);
    b.mapwait(1);
    b.mapcodejsl_builtin(cb::SETRESTART_L);
    b.mapcodejsl_builtin(cb::SET_PLAYER_ONPLANET_L);

    // LEVEL1_4.ASM:5 — mapjsr map1_4 (inlined below)
    b.mapjsr("level1_4.map1_4");

    // LEVEL1_4.ASM:7-9 — three gro_6 ground objects flanking the exit path.
    // NOTE: C passes 0x10000u / 0x12000u to an int16 z parameter — the
    // builder's internal `as i16` truncation reproduces 0 / 8192.
    b.mapobj(
        0x0000,
        -0x0800,
        0,
        0x8000,
        lc::SH_GRO_6_PROXY,
        is::HARD180YR,
    );
    b.mapobj(
        0x0000,
        0x1000,
        0,
        0x10000,
        lc::SH_GRO_6_PROXY,
        is::HARD180YR,
    );
    b.mapobj(
        0x0000,
        -0x1200,
        0,
        0x12000,
        lc::SH_GRO_6_PROXY,
        is::HARD180YR,
    );

    // LEVEL1_4.ASM:10 — mapjsr cl_ground
    b.mapjsr("cl_ground");
    // LEVEL1_4.ASM:11 — mapend
    b.mapend(1);

    // =================================================================
    // MAP1_4.ASM inlined — Asteroid Belt 2 map content
    // =================================================================
    b.label("level1_4.map1_4");

    // MAP1_4.ASM:11 — setvar.n infog,1
    b.setvarb(wm::INFOG, 1);
    b.mapwait(2000);

    // MAP1_4.ASM:13-14 — walkers
    b.cspecial(0x0200, 0x0750, 0, 0, lc::SH_WALKER_0, lc::IS_WALKING);
    b.cspecial(0x5000, 0x0450, 0, 0, lc::SH_WALKER_0, lc::IS_WALKING);

    // MAP1_4.ASM:17-18 — houdai (turrets)
    b.cspecial(0x0000, 0x0650, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.cspecial(0x3000, -0x0650, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);

    // MAP1_4.ASM:21-22 — tanks (path)
    b.pathspecial(
        0x0000,
        -0x0800,
        0,
        3000,
        lc::SH_S_TANK_0,
        lc::PATH_ID_E_TANK,
        10,
        10,
    );
    b.pathspecial(
        0x4000,
        0x0800,
        0,
        3000,
        lc::SH_S_TANK_0,
        lc::PATH_ID_E_TANK,
        10,
        10,
    );

    // MAP1_4.ASM:24-25 — friend chase6
    b.pathobj(
        0x0000,
        -0x0750,
        -400,
        0,
        sh::FRIENDSHIP_4,
        path::CHASE6_1,
        10,
        10,
    );
    b.pathcspecial(0x2500, -0x0720, -400, 0, sh::ZACO_A, path::CHASE6_2, 10, 10);

    // MAP1_4.ASM:27-29 — more tanks
    b.pathspecial(
        0x1500,
        0,
        0,
        3000,
        lc::SH_S_TANK_0,
        lc::PATH_ID_E_TANK,
        10,
        10,
    );
    b.pathcspecial(
        0x0000,
        -0x0450,
        0,
        3000,
        lc::SH_TANK_1,
        lc::PATH_ID_E_TANK,
        10,
        10,
    );
    b.pathcspecial(
        0x8000,
        0x0450,
        0,
        3000,
        lc::SH_TANK_1,
        lc::PATH_ID_E_TANK,
        10,
        10,
    );

    // MAP1_4.ASM:32-57 — pillar field (r_bu_7 rocks with items)
    b.mapobj(0x0000, 0x0250, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x1000, -0x0150, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, 0x0100, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, 0x0400, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x1000, -0x0200, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, -0x0400, -120, 1250, sh::R_BU_7, is::HARD180YR);
    // item_5 ring
    b.mapobj(0x0000, -0x0250, -120, 1250, sh::ITEM_5, is::ITEM5);
    b.setalvarb(al::SBYTE1, 1);
    b.mapobj(0x0000, -0x0100, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x1000, 0x0200, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, 0x0000, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, 0x0300, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x1000, -0x0300, -120, 1250, sh::R_BU_7, is::HARD180YR);
    // houdai in field
    b.cspecial(0x0000, -0x0700, 0, 3000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.mapobj(0x0000, 0x0400, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, 0x0100, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x1000, -0x0200, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.cspecial(0x0000, 0x0000, 0, 2800, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.mapobj(0x0000, 0x0050, -120, 1250, sh::R_BU_7, is::HARD180YR);
    // item_7 twin laser
    b.mapobj(0x0000, 0x0200, -120, 1250, sh::ITEM_7, is::ITEM7);
    b.setalvarb(al::SBYTE1, 1);
    b.mapobj(0x0000, 0x0350, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x1000, -0x0250, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, -0x0100, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, 0x0200, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x1000, -0x0400, -120, 1250, sh::R_BU_7, is::HARD180YR);

    b.mapobj(0x0000, 0x0400, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, 0x0100, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x1000, -0x0200, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, 0x0500, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, -0x0500, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, 0x0300, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, 0x0100, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0000, -0x0100, -120, 1250, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x2000, -0x0300, -120, 1250, sh::R_BU_7, is::HARD180YR);

    // MAP1_4.ASM:70-86 — rock section (gro shapes with fog)
    b.mapobj(
        0x0100,
        0x0050,
        0,
        1500,
        lc::SH_GRO_6_PROXY,
        lc::IS_HARD180YRFOG,
    );
    b.pathcspecial(
        0x1000,
        -0x0050,
        0,
        2050,
        lc::SH_WALKER_0,
        lc::PATH_ID_E_WALK_1,
        10,
        10,
    );

    b.mapobj(
        0x0000,
        -0x0800,
        0,
        2000,
        lc::SH_GRO_4_PROXY,
        lc::IS_HARD180YRFOG,
    );
    b.mapobj(
        0x1000,
        0x0600,
        0,
        2000,
        lc::SH_GRO_5_PROXY,
        lc::IS_HARD180YRFOG,
    );
    b.mapobj(
        0x0000,
        -0x0600,
        0,
        2000,
        lc::SH_GRO_4_PROXY,
        lc::IS_HARD180YRFOG,
    );
    b.mapobj(
        0x0000,
        0x0400,
        0,
        2000,
        lc::SH_GRO_5_PROXY,
        lc::IS_HARD180YRFOG,
    );
    b.pathcspecial(
        0x1000,
        0x0350,
        0,
        2550,
        lc::SH_WALKER_0,
        lc::PATH_ID_E_WALK_1,
        10,
        10,
    );

    b.mapobj(
        0x0000,
        -0x0300,
        0,
        2000,
        lc::SH_GRO_4_PROXY,
        lc::IS_HARD180YRFOG,
    );
    b.mapobj(
        0x1000,
        0x0400,
        0,
        2000,
        lc::SH_GRO_5_PROXY,
        lc::IS_HARD180YRFOG,
    );
    b.pathcspecial(
        0x0000,
        0,
        0,
        3000,
        lc::SH_TANK_1,
        lc::PATH_ID_E_TANK,
        10,
        10,
    );
    b.mapobj(
        0x0000,
        -0x0280,
        0,
        2000,
        lc::SH_GRO_0_PROXY,
        lc::IS_HARD180YRFOG,
    );
    b.mapobj(
        0x1000,
        0x0280,
        0,
        2000,
        lc::SH_GRO_1_PROXY,
        lc::IS_HARD180YRFOG,
    );
    b.mapobj(
        0x0000,
        -0x0250,
        0,
        2000,
        lc::SH_GRO_0_PROXY,
        lc::IS_HARD180YRFOG,
    );
    b.pathspecial(
        0x0000,
        -0x0300,
        0,
        3000,
        lc::SH_S_TANK_0,
        lc::PATH_ID_E_TANK,
        10,
        10,
    );
    b.mapobj(
        0x1000,
        0x0250,
        0,
        2000,
        lc::SH_GRO_1_PROXY,
        lc::IS_HARD180YRFOG,
    );

    // MAP1_4.ASM:88-89 — walkers
    b.cspecial(0x0200, 0x0700, 0, 0, lc::SH_WALKER_0, lc::IS_WALKING);
    b.cspecial(0x2500, 0x0400, 0, 0, lc::SH_WALKER_0, lc::IS_WALKING);

    // MAP1_4.ASM:92-101 — palette fade (fog transition)
    b.setvarb(wm::FADEPAL, 32);
    b.setvarw(wm::PALFROM, 64);
    b.setvarw(wm::PALTO, 1 * 32);
    b.setvarw(wm::PALLEN, 16);
    b.mapwait(1500);
    b.setvarb(wm::FADEPAL, 32);
    b.setvarw(wm::PALFROM, 96);
    b.setvarw(wm::PALTO, 5 * 32);
    b.setvarw(wm::PALLEN, 15);

    // MAP1_4.ASM:104-111 — heli section with ground rocks
    b.pathspecial(
        0x1000,
        0x0450,
        0,
        3000,
        lc::SH_S_TANK_0,
        lc::PATH_ID_E_TANK,
        10,
        10,
    );
    b.mapobj(0x1000, 0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.pathcspecial(
        0x0000,
        -0x0800,
        -170,
        -100,
        lc::SH_HELI,
        lc::PATH_ID_KAMOME,
        10,
        10,
    );
    // .groloop: 2x gro_6 pair, loop 2 times
    b.label("level1_4.groloop");
    b.mapobj(0x1000, 0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.maploop("level1_4.groloop", 2);

    // MAP1_4.ASM:113-123 — heli + bom_wing + houdai
    b.pathcspecial(
        0x0000,
        -0x0800,
        -170,
        -100,
        lc::SH_HELI,
        lc::PATH_ID_KAMOME,
        10,
        10,
    );
    b.mapobj(0x1000, 0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.cspecial(0x0800, 0, -30, 3000, sh::BOM_WING, is::BOMWING);

    b.cspecial(0x0800, 0, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.cspecial(0x0000, -0x0700, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.mapobj(0x1000, 0x0400, -100, 4000, sh::NULLSHAPE, lc::IS_UP1MAN);
    b.mapobj(0x1000, -0x1700, 0, 5000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x0000, 0x0600, 0, 5000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.pathcspecial(
        0x1500,
        0x0600,
        0,
        5300,
        lc::SH_WALKER_0,
        lc::PATH_ID_E_WALK_1,
        10,
        10,
    );

    // MAP1_4.ASM:125-132 — friend chase7 + more heli
    b.pathobj(
        0x0000,
        0,
        -400,
        -150,
        sh::FRIENDSHIP_4,
        lc::PATH_ID_CHASE7_1,
        10,
        10,
    );
    b.pathcspecial(
        0x1000,
        0,
        -400,
        -150,
        sh::ZACO_A,
        lc::PATH_ID_CHASE7_2,
        10,
        10,
    );
    b.pathcspecial(
        0x0000,
        -0x0800,
        -170,
        -100,
        lc::SH_HELI,
        lc::PATH_ID_KAMOME,
        10,
        10,
    );
    b.mapobj(0x1000, 0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.pathcspecial(
        0x0000,
        0x0800,
        -170,
        -100,
        lc::SH_HELI,
        lc::PATH_ID_KAMOME,
        10,
        10,
    );
    b.mapobj(0x1000, -0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.pathcspecial(
        0x0000,
        -0x0800,
        -170,
        -100,
        lc::SH_HELI,
        lc::PATH_ID_KAMOME,
        10,
        10,
    );
    b.mapobj(0x1000, 0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);

    // MAP1_4.ASM:134-135 — houdai pair
    b.cspecial(0x1000, 0x0400, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.cspecial(0x1000, -0x0400, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);

    // MAP1_4.ASM:137-139 — bu_3 corridor + walker
    b.mapobj(0x0000, 0x0400, 0, 5500, sh::BU_3, is::HARD180YR);
    b.pathcspecial(
        0x0000,
        0x0450,
        0,
        6000,
        lc::SH_WALKER_0,
        lc::PATH_ID_E_WALK_1,
        10,
        10,
    );
    b.mapobj(0x0000, 0x0500, 0, 6500, sh::BU_3, is::HARD180YR);

    // MAP1_4.ASM:142-152 — base & tank section
    b.mapobj(0x1000, 0x0700, 0, 7000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x0600, 0, 7000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(
        0x0000,
        -0x1500,
        0,
        8000,
        lc::SH_BASE_0_0_PROXY,
        lc::IS_BASE0,
    );
    b.special(0x0000, -0x1500, 0, 8004, lc::SH_S_TANK_0, lc::IS_TANK1A);
    b.setalvarb(al::SBYTE1, 50);
    b.cspecial(
        0x0000,
        -0x1200,
        0,
        8004,
        lc::SH_BTANK_1_PROXY,
        lc::IS_TANK1A,
    );
    b.setalvarb(al::SBYTE1, 55);
    b.cspecial(
        0x0000,
        -0x0900,
        0,
        8004,
        lc::SH_BTANK_1_PROXY,
        lc::IS_TANK1A,
    );
    b.setalvarb(al::SBYTE1, 60);
    b.mapobj(
        0x3000,
        -0x1500,
        0,
        8005,
        lc::SH_BASE_0_1_PROXY,
        lc::IS_BASE0,
    );
    b.mapobj(0x7500, 0x1500, 0, 7000, lc::SH_GRO_6_PROXY, is::HARD180YR);

    // MAP1_4.ASM:154-156 — houdai + base
    b.cspecial(0x1000, -0x0400, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.cspecial(0x3500, -0x0400, 0, 4000, lc::SH_HOU_5, lc::IS_HOUDAI5F);
    b.mapobj(0x0000, 0x0200, 0, 5000, lc::SH_BASE_0_0_PROXY, lc::IS_BASE1);

    // MAP1_4.ASM:158-160 — friend chase8
    b.pathobj(
        0x0000,
        0x0750,
        -100,
        0,
        sh::FRIENDSHIP_4,
        path::CHASE8_1,
        10,
        10,
    );
    b.pathcspecial(
        0x0000,
        0x3800,
        -3600,
        4260,
        sh::ZACO_A,
        path::CHASE8_2,
        10,
        10,
    );
    b.pathcspecial(0x0000, 0x0750, -100, 0, sh::ZACO_A, path::CHASE8_3, 10, 10);

    // MAP1_4.ASM:162-163 — gate
    b.mapobj(0x0000, 0x0200, -100, 5500, sh::GATE_0, is::GATE);
    b.pathobj(
        0x1000,
        3000,
        3000,
        3000,
        sh::NULLSHAPE,
        path::E_GATE,
        10,
        10,
    );

    b.mapwait(3000);

    // MAP1_4.ASM:167-176 — palette fade (second transition)
    b.setvarb(wm::FADEPAL, 32);
    b.setvarw(wm::PALFROM, 64);
    b.setvarw(wm::PALTO, 1 * 32);
    b.setvarw(wm::PALLEN, 16);
    b.mapwait(1500);
    b.setvarb(wm::FADEPAL, 32);
    b.setvarw(wm::PALFROM, 96);
    b.setvarw(wm::PALTO, 5 * 32);
    b.setvarw(wm::PALLEN, 15);

    // MAP1_4.ASM:178-208 — tank2 + heli section + bases + gro_6 corridors
    b.mapobj(0x1000, -0x1000, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.cspecial(0x1000, 0x0300, 0, 4000, lc::SH_TANK_2_PROXY, lc::IS_TANK2);
    b.pathobj(
        0x0000,
        0x0300,
        -600,
        3000,
        lc::SH_HELI,
        lc::PATH_ID_KAMOME,
        10,
        10,
    );
    b.pathobj(
        0x0000,
        -0x0300,
        -600,
        3000,
        lc::SH_HELI,
        lc::PATH_ID_KAMOME,
        10,
        10,
    );
    b.mapobj(0x1000, 0x1100, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x1100, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1000, 0x1100, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x1100, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1000, 0x1100, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x0600, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x2000, -0x1650, 0, 4500, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x3000, 0x1150, 0, 5000, lc::SH_GRO_6_PROXY, is::HARD180YR);

    b.mapobj(
        0x0000,
        -0x0300,
        0,
        5000,
        lc::SH_BASE_0_0_PROXY,
        lc::IS_BASE1,
    );
    b.mapobj(0x0000, 0x0300, 0, 5000, lc::SH_BASE_0_0_PROXY, lc::IS_BASE1);
    b.mapobj(0x0000, -0x0300, -50, 5300, sh::ITEM_5, is::ITEM5);
    b.setalvarb(al::SBYTE1, 1);
    b.mapobj(0x1300, 0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1300, -0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1300, 0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1300, 0x1000, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.pathobj(
        0x0000,
        -0x0100,
        -400,
        -150,
        sh::FRIENDSHIP_4,
        lc::PATH_ID_CHASE7_1,
        10,
        10,
    );
    b.pathcspecial(
        0x0000,
        -0x0100,
        -400,
        -150,
        sh::ZACO_A,
        lc::PATH_ID_CHASE7_2,
        10,
        10,
    );
    b.cspecial(0x0000, -0x0100, 0, 4000, lc::SH_TANK_2_PROXY, lc::IS_TANK2);
    b.pathobj(
        0x0000,
        0x0300,
        -600,
        3000,
        lc::SH_HELI,
        lc::PATH_ID_KAMOME,
        10,
        10,
    );
    b.pathobj(
        0x0000,
        -0x0300,
        -600,
        2500,
        lc::SH_HELI,
        lc::PATH_ID_KAMOME,
        10,
        10,
    );
    b.pathobj(
        0x1000,
        0,
        -600,
        2000,
        lc::SH_HELI,
        lc::PATH_ID_KAMOME,
        10,
        10,
    );
    b.mapobj(0x1000, 0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1000, 0x0900, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x1000, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);
    b.mapobj(0x6000, -0x0500, 0, 4000, lc::SH_GRO_6_PROXY, is::HARD180YR);

    // MAP1_4.ASM:214 — setbgm bgm_boss1
    b.setbgm(BGM_BOSS1);

    // MAP1_4.ASM:217 — boss_h_0 / bossh_istrat (gggy legged boss).
    // The assembler's default radix is decimal: MAP1_4.ASM's `2000` is
    // 2000, not $2000.  Spawning at 8192 put the boss far outside the play
    // corridor; it could cross the camera plane and be z-removed before its
    // child family finished the fight.
    b.mapobj(0, 2000, -600, 1000, lc::SH_BOSS_H_0, lc::STRATEGY_BOSSH);

    // mapwaitboss pattern
    let mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("level1_4.bosswait.loop");
    b.mapif_builtin(cb::CHKBOSSDEAD, "level1_4.bosswait.cont");
    b.mapgoto("level1_4.bosswait.loop");
    b.label("level1_4.bosswait.cont");
    let mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let mapwaitboss_cleanup_ptr = b.mapcode65816_inline();
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(cb::MARKBOSS_L);

    // MAP1_4.ASM:222-224 — markboss boss14
    b.mapwait(1000);
    b.maprts();

    // Shared clear-demo subroutine (called via mapjsr cl_ground above)
    append_cl_ground_submap(&mut b);

    b.resolve();

    let (data, labels) = b.finish();

    // C `register_level1_4_inline_callbacks()` — registration-call order,
    // guarded by ptr != 0 like C.
    let mut inline_regs: Vec<(u16, &'static str)> = Vec::new();
    for (ptr, name) in [
        (mapwaitboss_trigse_ptr, "level1_4_mapwaitboss_trigse"),
        (mapwaitboss_cantdie_ptr, "level1_4_mapwaitboss_cantdie"),
        (mapwaitboss_cleanup_ptr, "level1_4_mapwaitboss_cleanup"),
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
        native_regs: vec![], // level1_4 registers no natives
        inline_regs,
    }
}

/// C `append_cl_ground_submap()` (levels.c lines 4566-4619) — CL_GND.ASM
/// shared ground clear demo.
// DUPLICATE: consolidate (shared with level1_1.rs and other routes).
fn append_cl_ground_submap(b: &mut MapBuilder) {
    b.label("cl_ground");
    b.setbgm(BGM_FADEOUT);
    b.mapwait(2000);
    b.setbgm(BGM_FANFARE);
    b.mapwait(3000);
    b.setvarb(wm::STAGECLEAR, 1);
    b.mapcodejsl_builtin(cb::SET_PLAYER_CLEARDEMO_L);

    b.sendmsg(1);
    b.mapwait(CL_GND_FRIENDWAIT);

    b.mapif_builtin(cb::FROG_ALIVE, "cl_ground.frog_alive");
    b.mapgoto("cl_ground.nf");
    b.label("cl_ground.frog_alive");
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_FROG);
    b.mapobj(
        CL_GND_FRIENDWAIT,
        500,
        -50,
        50,
        sh::MYSHIP_4,
        is::CLSHIPGNDB,
    );
    b.label("cl_ground.nf");

    b.mapif_builtin(cb::BUNNY_ALIVE, "cl_ground.bunny_alive");
    b.mapgoto("cl_ground.nb");
    b.label("cl_ground.bunny_alive");
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_BUNNY);
    b.mapobj(
        CL_GND_FRIENDWAIT,
        -500,
        -50,
        50,
        sh::MYSHIP_4,
        is::CLSHIPGNDA,
    );
    b.label("cl_ground.nb");

    b.mapif_builtin(cb::COCK_ALIVE, "cl_ground.cock_alive");
    b.mapgoto("cl_ground.nc");
    b.label("cl_ground.cock_alive");
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_COCK);
    b.mapobj(
        CL_GND_FRIENDWAIT,
        0,
        -500,
        -300,
        sh::MYSHIP_4,
        is::CLSHIPGNDC,
    );
    b.label("cl_ground.nc");

    b.mapwait(3800);
    b.setvarb(wm::CLB2, 0);
    b.setvarb(wm::STAGECLEAR, 0);
    b.mapcodejsl_builtin(cb::CL_GROUND_PRINTLEVELFIN);
    b.label("cl_ground.eswait");
    b.mapwait(1);
    b.maploop("cl_ground.eswait", 100);
    b.mapcodejsl_builtin(cb::CL_GROUND_WIPEOUT);
    b.setbgm(BGM_FADEOUT);
    b.mapwait(32 * MEDPSPEED);
    b.setvarb(wm::CLB2, 1);
    b.maprts();
}
