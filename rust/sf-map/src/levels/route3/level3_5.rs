//! MAP_ID_3_5 — Macbeth (Level 3, Route 3).
//!
//! C oracle: `src/map/levels.c` `build_level3_5_wrapper_slice()` +
//! `register_level3_5_inline_callbacks()`.
//! ASM: LEVEL3_5.ASM / MAP3_5.ASM / CL_UNDER.ASM.

use super::common::*;
use super::finish_level;
use super::Route3Level;
use crate::builder::MapBuilder;

pub(crate) fn build() -> Route3Level {
    let mut b = MapBuilder::new();

    // (MAP_ID_3_5)
    // ============================================================
    // (C mb_ttruck/mb_thoriz/mb_tvert/mb_tcorner helpers live in
    // common::Route3Ext.)

    // LEVEL3_5.ASM — 3-5 Macbeth wrapper (Venom 1 Surface)
    // Generic level init for ground stage.
    b.mapcodejsl_builtin(MAP_CB_INITBLACK_L);
    b.mapwait(1);
    b.mapcodejsl_builtin(MAP_CB_SETRESTART_L);
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_ONPLANET_L);

    // LEVEL3_5.ASM:5 — mapjsr map3_5
    b.mapjsr("level3_5.map3_5");

    // LEVEL3_5.ASM:7-12 — ro_6 objects flanking exit path
    b.mapobj(0x0000, 800, 0, 8000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, 800, 0, 8150, SH_RO_6_PROXY, IS_HARD);
    b.mapobj(0x0000, -800, 0, 10000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, -800, 0, 10150, SH_RO_6_PROXY, IS_HARD);
    b.mapobj(0x0000, 800, 0, 12000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, 800, 0, 12150, SH_RO_6_PROXY, IS_HARD);

    // LEVEL3_5.ASM:14-15 — mapjsr cl_under / mapend
    b.mapjsr("cl_under");
    b.mapend(1);

    // === MAP3_5.ASM subroutine — Venom 1 Surface map content ===
    b.label("level3_5.map3_5");

    // MAP3_5.ASM high = -600

    // MAP3_5.ASM:8-14 — initial rock corridor
    b.mapobj(0x0000, -0x0600, 0, 0x0800, SH_RO_4_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, 0x0600, 0, 0x0800, SH_RO_5_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, -0x0500, 0, 0x1800, SH_RO_4_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, 0x0500, 0, 0x1800, SH_RO_5_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, -0x0400, 0, 0x2800, SH_RO_4_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, 0x0400, 0, 0x2800, SH_RO_5_PROXY, IS_ROCKHARD);

    // MAP3_5.ASM:16-20 — mixed rocks
    b.mapobj(0x0000, -0x0400, 0, 0x3800, SH_RO_4_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, 0x0550, 0, 0x3800, SH_RO_1_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, -0x0400, 0, 0x4800, SH_RO_0_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, 0x0400, 0, 0x4800, SH_RO_5_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, -0x0900, 0, 0x5800, SH_RO_2_PROXY, IS_ROCKHARD);

    // MAP3_5.ASM:22-23 — tumble_robot: item_5
    b.mapobj(0x0000, -0x0150, -100, 0x5000, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.mapobj(0x3000, 0x0100, 0, 0x5800, SH_RO_5_PROXY, IS_ROCKHARD);

    // MAP3_5.ASM:26-37 — more rocks + walker + houdai
    b.mapobj(0x0000, -0x1000, 0, 0x3800, SH_RO_0_PROXY, IS_ROCKHARD);
    b.mapobj(0x1000, -0x0100, 0, 0x3800, SH_RO_5_PROXY, IS_ROCKHARD);
    b.pathcspecial(
        0x0000,
        -0x0100,
        0,
        3350,
        SH_WALKER_0,
        PATH_ID_E_WALK_1,
        10,
        10,
    );
    b.mapobj(0x0000, -0x0800, 0, 0x3800, SH_RO_4_PROXY, IS_ROCKHARD);
    b.mapobj(0x2000, -0x0500, 0, 0x4800, SH_RO_4_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, 0x0500, 0, 0x2800, SH_RO_1_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, -0x0200, 0, 0x3800, SH_RO_4_PROXY, IS_ROCKHARD);
    b.cspecial(0x0000, 0x0200, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    b.mapobj(0x1000, 0x0600, 0, 0x3800, SH_RO_1_PROXY, IS_ROCKHARD);
    b.pathobj(
        0x0500,
        0x0600,
        0,
        3350,
        SH_WALKER_0,
        PATH_ID_E_WALK_1,
        10,
        10,
    );
    // ceiling column (RO_6 with deg180 z-rot)
    b.mapobj(0x0000, 0x0450, -600, 4000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapwait(0x0500);

    // MAP3_5.ASM:39-56 — rock field section 2
    b.mapobj(0x0000, 0, 0, 0x2800, SH_RO_4_PROXY, IS_ROCKHARD);
    b.mapobj(0x1000, 0x0800, 0, 0x2800, SH_RO_1_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, 0x0100, 0, 0x2800, SH_RO_0_PROXY, IS_ROCKHARD);
    b.pathcspecial(
        0x0000,
        0x0100,
        0,
        3350,
        SH_WALKER_0,
        PATH_ID_E_WALK_1,
        10,
        10,
    );
    b.mapobj(0x0000, 0x1000, 0, 0x2800, SH_RO_5_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, 0x0800, 0, 0x3800, SH_RO_5_PROXY, IS_ROCKHARD);
    b.mapobj(0x2500, 0x0500, 0, 0x4800, SH_RO_5_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, -0x0500, 0, 0x2300, SH_RO_0_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, 0, 0, 0x3100, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, -0x0800, 0, 0x3300, SH_RO_0_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, 0x0800, 0, 0x3300, SH_RO_1_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, -0x0600, 0, 0x4100, SH_RO_2_PROXY, IS_ROCKHARD);
    // skillfly_init + set
    b.skillfly_init();
    b.skillfly_set(-280, -50, 3000, 100);
    b.mapobj(0x0500, 0x0600, 0, 0x4100, SH_RO_3_PROXY, IS_ROCKHARD);
    b.mapobj(0x0000, 0x0100, -600, 4000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);

    // MAP3_5.ASM:58-63 — exit_of_rocks: tanks
    b.pathspecial(0x0000, 250, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    b.mapobj(0x0000, -0x1200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x1000, 0x1200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.pathspecial(
        0x0000,
        -0x0100,
        0,
        3000,
        SH_S_TANK_0,
        PATH_ID_E_TANK,
        10,
        10,
    );
    b.mapobj(0x0000, -0x1200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, 0x1500, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);

    // MAP3_5.ASM:65-68 — friend chase6
    b.pathobj(
        0x0000,
        -0x0750,
        -400,
        0,
        SH_FRIENDSHIP_4,
        PATH_ID_CHASE6_1,
        10,
        10,
    );
    b.pathobj(
        0x1500,
        -0x0750,
        -400,
        0,
        SH_ZACO_A,
        PATH_ID_CHASE6_2,
        10,
        10,
    );
    // skillfly_bonus item_5
    let skillfly_bonus_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0x0000, 0, -120, 1300, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.label("level3_5.skillfly_bonus_skip");

    // MAP3_5.ASM:69-75 — more ceiling rocks
    b.mapobj(0x0000, 0x0700, -600, 4000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x0000, -0x1200, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapwait(0x1000);
    b.mapobj(0x0000, 0x0600, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);

    // MAP3_5.ASM:77-82 — across_robot: ceiling + walker
    b.mapobj(0x1000, -0x1200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x1000, -0x0400, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, -0x0700, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.pathobj(
        0x0000,
        -0x0400,
        0,
        4500,
        SH_WALKER_0,
        PATH_ID_E_WALK_1,
        6,
        4,
    );
    b.mapobj(0x2000, 0x0900, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);

    // MAP3_5.ASM:85-90 — ceiling_town: houdai pair + inverted houdai
    b.mapobj(0x0000, -0x1200, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x2000, 0x0500, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.cspecial(0x1000, -0x0300, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    b.pathcspecial(0x1000, 0, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    b.cspecial(0x2000, 0x0200, 0, 3300, SH_HOU_5, IS_HOUDAI5F);

    // MAP3_5.ASM:91-105 — train track (tstart/teast/tsouth pattern)
    // tstart 0,1,east => tx=0, tz=1, ta=dirEAST=DEG270
    // (literal transcription of the C tx/tz/ta bookkeeping, including the
    // dead trailing assignments the C block also performs)
    #[allow(unused_assignments)]
    {
        let mut tx: i32 = 0;
        let mut tz: i32 = 1;
        let mut ta: i32 = DIR_EAST;
        b.ttruck(tx, tz, ta); // tstart

        // teast: from east, go east => straight east
        b.thoriz(tx, tz);
        tx += 1;
        ta = DIR_EAST;
        // teast
        b.thoriz(tx, tz);
        tx = tx + 1;
        ta = DIR_EAST;
        // tsouth: from east, turn south => right corner
        ta -= DEG90;
        b.tcorner(tx, tz, ta, 1);
        tz = tz - 1;
        // tsouth straight
        b.tvert(tx, tz);
        tz = tz - 1;
        ta = DIR_SOUTH;
        // tsouth
        b.tvert(tx, tz);
        tz = tz - 1;
        // tsouth
        b.tvert(tx, tz);
        tz = tz - 1;
        // tanothertruck
        b.ttruck(tx, tz, ta);
        // tsouth
        b.tvert(tx, tz);
        tz = tz - 1;
        // teast: from south, turn east => left corner
        ta += DEG90;
        b.tcorner(tx, tz, ta, 0);
        tx = tx + 1;
        ta = DIR_EAST;
        // tsouth: from east, turn south => right corner
        ta -= DEG90;
        b.tcorner(tx, tz, ta, 1);
        tz = tz - 1;
        ta = DIR_SOUTH;
        // tsouth
        b.tvert(tx, tz);
        tz = tz - 1;
        // tsouth
        b.tvert(tx, tz);
        tz = tz - 1;
        // teast: from south, turn east => left corner
        ta += DEG90;
        b.tcorner(tx, tz, ta, 0);
        tx = tx + 1;
        ta = DIR_EAST;
        // teast
        b.thoriz(tx, tz);
        tx = tx + 1;
    }

    // MAP3_5.ASM:106-117 — ceiling buildings (bu_2 + bu_0)
    b.mapobj(0x0000, -0x0700, -600, 4000, SH_BU_2, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x0000, 0x0700, -600, 4000, SH_BU_2, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapwait(0x1000);

    b.mapobj(0x0000, -0x0500, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(0x1000, 0x0500, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(0x0000, -0x0700, -600, 4000, SH_BU_2, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x0000, 0x0700, -600, 4000, SH_BU_2, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);

    // MAP3_5.ASM:118-123 — miss_1_1/miss_1_2 pair (missile launcher)
    b.mapobj(0x0000, 0, -600, 3000, SH_MISS_1_1, IS_NOCOLL);
    b.setvarobj(WM_MAPVAR1);
    b.setalvarb(AL_ROTX, 127);
    b.mapobj(0x0000, 0, -580, 3000, SH_MISS_1_2, IS_WOODS);
    b.setalvarptrw(AL_PTR, WM_MAPVAR1);
    b.setalvarb(AL_ROTX, DEG90);

    // MAP3_5.ASM:124-132 — tanks + bu_0 ceiling + bu_3 ground
    b.pathspecial(
        0x0000,
        -0x0150,
        0,
        3000,
        SH_S_TANK_0,
        PATH_ID_E_TANK,
        10,
        10,
    );
    b.pathspecial(0x2000, 0x0150, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    b.mapobj(0x0000, -0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x0000, 0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.pathspecial(0x1000, 0x0150, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    b.mapobj(0x0000, -0x0800, 0, 4500, SH_BU_3, IS_HARD180YR);
    b.mapobj(0x2000, 0x0800, 0, 4500, SH_BU_3, IS_HARD180YR);

    // MAP3_5.ASM:134-139 — second miss_1_1/miss_1_2 pair
    b.mapobj(0x0000, 0, -600, 3000, SH_MISS_1_1, IS_NOCOLL);
    b.setvarobj(WM_MAPVAR1);
    b.setalvarb(AL_ROTX, 127);
    b.mapobj(0x0000, 0, -580, 3000, SH_MISS_1_2, IS_WOODS);
    b.setalvarptrw(AL_PTR, WM_MAPVAR1);
    b.setalvarb(AL_ROTX, DEG90);

    b.mapobj(0x0000, 0x1200, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.pathcspecial(0x0000, 0, -600, 3000, SH_TANK_1, PATH_ID_E_TANK, 10, 10);
    b.setalvarb(AL_ROTZ, DEG180);

    // MAP3_5.ASM:144-151 — .ceiltown loop (3 iterations)
    b.label("level3_5.ceiltown");
    b.mapobj(0x0000, -0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x0000, 0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x0000, -0x0800, 0, 4500, SH_BU_3, IS_HARD180YR);
    b.mapobj(0x1000, 0x0800, 0, 4500, SH_BU_3, IS_HARD180YR);
    b.mapobj(0x1000, 0, 0, 4000, SH_BU_8, IS_HARD180YR);
    b.maploop("level3_5.ceiltown", 3);

    // MAP3_5.ASM:153-162 — fall_walker section
    b.mapobj(0x0000, -0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x0000, 0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x1000, -0x1200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.cspecial(0x0000, 0x0200, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    b.mapobj(0x1000, 0x1500, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, -0x0600, 0, 5000, SH_BU_8, IS_HARD180YR);
    b.mapobj(0x1000, 0x0600, 0, 5000, SH_BU_8, IS_HARD180YR);
    b.pathcspecial(0x0000, 0, 0, 5400, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);

    // MAP3_5.ASM:164-176 — twin_lazer section
    b.mapobj(0x2000, 0, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.mapobj(0x0000, 0, -120, 3800, SH_ITEM_7, IS_ITEM7);
    b.setalvarb(AL_SBYTE1, 1);
    b.mapobj(0x0000, -0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x0000, 0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.pathcspecial(0x0000, 0, 0, 5400, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);
    b.mapobj(0x3000, 0, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.mapobj(0x0000, 0x1200, -600, 7000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x0000, -0x1200, -600, 8000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);

    // MAP3_5.ASM:177-184 — .volcs0 loop (small volcanoes, 2 iterations)
    b.label("level3_5.volcs0");
    b.mapobj(
        0x0500,
        -0x0300,
        -600,
        3000,
        SH_SVOLCANO_PROXY,
        IS_FIREPILLAR,
    );
    b.pathobj(0x0500, 0, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    b.mapobj(
        0x0500,
        -0x0200,
        -600,
        3000,
        SH_SVOLCANO_PROXY,
        IS_FIREPILLAR,
    );
    b.mapobj(0x0500, 0x0200, -600, 3000, SH_SVOLCANO_PROXY, IS_FIREPILLAR);
    b.maploop("level3_5.volcs0", 2);
    b.mapobj(0x2000, 0x0800, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, 0x1000, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapwait(0x3000);

    // MAP3_5.ASM:187-189 — big_volcano
    b.mapobj(0x0000, -0x0080, -50, 4200, SH_ITEM_6, IS_ITEM6);
    b.setalvarb(AL_SBYTE1, 1);
    b.mapobj(0x3000, -0x0300, 0, 4000, SH_VOLCANO_PROXY, IS_VOLCANO);

    // MAP3_5.ASM:192-204 — missile pairs + inverted houdai
    b.mapobj(0x0000, -0x0200, 0, 3000, SH_MISS_1_1, IS_NOCOLL);
    b.setvarobj(WM_MAPVAR1);
    b.mapobj(0x0000, -0x0200, -20, 3000, SH_MISS_1_2, IS_WOODS);
    b.setalvarptrw(AL_PTR, WM_MAPVAR1);
    b.setalvarb(AL_ROTX, -(DEG90));
    b.mapwait(0x1000);
    b.mapobj(0x0000, 0, 0, 3000, SH_MISS_1_1, IS_NOCOLL);
    b.setvarobj(WM_MAPVAR1);
    b.mapobj(0x0000, 0, -20, 3000, SH_MISS_1_2, IS_WOODS);
    b.setalvarptrw(AL_PTR, WM_MAPVAR1);
    b.setalvarb(AL_ROTX, -(DEG90));
    b.pathcspecial(
        0x2000,
        -0x0300,
        -600,
        3300,
        SH_HOU_5,
        PATH_ID_E_TANK,
        10,
        10,
    );
    b.mapobj(0x1000, 0x0700, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);

    // MAP3_5.ASM:206-216 — .volcs2 loop (volcanoes + rocks, 2 iterations)
    b.label("level3_5.volcs2");
    b.mapobj(
        0x0500,
        -0x0400,
        -600,
        3000,
        SH_SVOLCANO_PROXY,
        IS_FIREPILLAR,
    );
    b.pathobj(0x0200, 0x0200, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    b.mapobj(0x0200, 0, -600, 3000, SH_SVOLCANO_PROXY, IS_FIREPILLAR);
    b.mapobj(0x0400, 0x1200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(
        0x0500,
        -0x0200,
        -600,
        3000,
        SH_SVOLCANO_PROXY,
        IS_FIREPILLAR,
    );
    b.mapobj(0x0200, -0x1000, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0500, 0x0400, -600, 3000, SH_SVOLCANO_PROXY, IS_FIREPILLAR);
    b.maploop("level3_5.volcs2", 2);
    b.mapobj(0x0000, 0x0400, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, -0x0400, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapwait(0x1000);

    // MAP3_5.ASM:218-220 — gate
    b.mapobj(0x0000, 0, -150, 4000, SH_GATE_0, IS_GATE);
    b.pathobj(0x1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);
    b.mapwait(0x2000);
    b.mapobj(0x0000, 0x0800, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);

    // MAP3_5.ASM:223-241 — .woodmiss loop (missile pairs, 4 iterations)
    b.label("level3_5.woodmiss");
    b.mapobj(0x0000, -0x0200, 0, 3000, SH_MISS_1_1, IS_NOCOLL);
    b.setvarobj(WM_MAPVAR1);
    b.mapobj(0x0000, -0x0200, -20, 3000, SH_MISS_1_2, IS_WOODS);
    b.setalvarptrw(AL_PTR, WM_MAPVAR1);
    b.setalvarb(AL_ROTX, -(DEG90));
    b.mapobj(0x0000, 0x0200, 0, 3000, SH_MISS_1_1, IS_NOCOLL);
    b.setvarobj(WM_MAPVAR1);
    b.mapobj(0x0000, 0x0200, -20, 3000, SH_MISS_1_2, IS_WOODS);
    b.setalvarptrw(AL_PTR, WM_MAPVAR1);
    b.setalvarb(AL_ROTX, -(DEG90));
    b.mapwait(0x0800);
    b.mapobj(0x0000, 0, -600, 3500, SH_MISS_1_1, IS_NOCOLL);
    b.setvarobj(WM_MAPVAR1);
    b.setalvarb(AL_ROTX, 127);
    b.mapobj(0x0000, 0, -580, 3500, SH_MISS_1_2, IS_WOODS);
    b.setalvarptrw(AL_PTR, WM_MAPVAR1);
    b.setalvarb(AL_ROTX, DEG90);
    b.mapwait(0x0800);
    b.maploop("level3_5.woodmiss", 4);
    b.mapobj(0x1000, 0x0700, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);

    // MAP3_5.ASM:244-256 — friend chase6 + tanks + houdai
    b.pathobj(
        0x0000,
        -0x0750,
        -400,
        0,
        SH_FRIENDSHIP_4,
        PATH_ID_CHASE6_1,
        10,
        10,
    );
    b.pathobj(
        0x0400,
        -0x0750,
        -400,
        0,
        SH_ZACO_A,
        PATH_ID_CHASE6_2,
        10,
        10,
    );
    b.pathcspecial(
        0x0000,
        0x0150,
        -600,
        3000,
        SH_TANK_1,
        PATH_ID_E_TANK,
        10,
        10,
    );
    b.setalvarb(AL_ROTZ, DEG180);
    b.pathcspecial(
        0x0000,
        -0x0150,
        -600,
        3000,
        SH_TANK_1,
        PATH_ID_E_TANK,
        10,
        10,
    );
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x1000, -0x0800, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x2000, 0, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x1000, -0x0800, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, 0x0600, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.cspecial(0x1000, 0, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    b.mapobj(0x1000, -0x0900, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);

    // MAP3_5.ASM:259-291 — fire_balls: houdai gauntlet + inverted cannons
    b.mapobj(0x2000, 0x0800, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.pathcspecial(
        0x1000,
        -0x0300,
        -600,
        3300,
        SH_HOU_5,
        PATH_ID_E_TANK,
        10,
        10,
    );
    b.pathcspecial(
        0x1000,
        -0x0100,
        -600,
        3300,
        SH_HOU_5,
        PATH_ID_E_TANK,
        10,
        10,
    );
    b.mapobj(0x0000, -0x0800, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x0000, 0x0700, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.cspecial(0x1000, 0, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    b.cspecial(0x1000, -0x0200, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    b.cspecial(0x1000, 0x0200, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    b.pathcspecial(
        0x1000,
        -0x0300,
        -600,
        3300,
        SH_HOU_5,
        PATH_ID_E_TANK,
        10,
        10,
    );
    b.pathcspecial(0x1000, 0x0300, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    b.cspecial(0x1000, -0x0400, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    b.cspecial(0x1000, 0x0400, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    b.cspecial(0x1000, 0, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    b.cspecial(0x1000, -0x0500, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    b.cspecial(0x1000, 0x0500, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    b.mapobj(0x2000, 0, 0, 4000, SH_VOLCANO_PROXY, IS_VOLCANO);

    b.pathcspecial(0x1000, 0, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    b.pathcspecial(
        0x0000,
        -0x0400,
        -600,
        3300,
        SH_HOU_5,
        PATH_ID_E_TANK,
        10,
        10,
    );
    b.pathcspecial(0x1000, 0x0400, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    b.pathcspecial(
        0x0000,
        -0x0200,
        -600,
        3300,
        SH_HOU_5,
        PATH_ID_E_TANK,
        10,
        10,
    );
    b.pathcspecial(0x0000, 0x0200, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    b.pathcspecial(0x0000, 0, -600, 3000, SH_TANK_1, PATH_ID_E_TANK, 10, 10);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x0400, -0x0500, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0800, 0x0500, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, -0x0300, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapobj(0x2000, 0x0200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, -0x0800, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setalvarb(AL_ROTZ, DEG180);

    // MAP3_5.ASM:294-308 — boss section. `fadeoutbgm` includes the normal
    // MEDPSPEED * 30 wait. The following 2,000-unit wait in the source is
    // inside `IFNE MSU1` and is absent from the retail build (MSU1 = 0).
    b.setbgm(BGM_FADEOUT);
    b.mapwait(MEDPSPEED * 30);
    b.setbgm(BGM_BOSS1);

    // boss_2_2 spawn (0<<boss2_scale = 0)
    b.mapobj(0x0000, 0, 0, 4000, SH_BOSS_2_2_PROXY, IS_BOSS2);

    // mapwaitboss
    let mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("level3_5.bosswait.loop");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, "level3_5.bosswait.cont");
    b.mapgoto("level3_5.bosswait.loop");
    b.label("level3_5.bosswait.cont");
    let mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let mapwaitboss_cleanup_ptr = b.mapcode65816_inline();

    // post-boss: rocks + markboss boss35
    b.mapobj(0x0000, 0x1000, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    b.mapobj(0x0000, -0x1000, 0, 8000, SH_RO_6_PROXY, IS_HARD180YR);
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);
    b.mapwait(0x0500 + 15 * MEDPSPEED);
    b.maprts();

    // CL_UNDER.ASM — clear demo (under type) appended as subroutine.
    append_cl_under_submap(&mut b);

    b.resolve();

    // C: bails to s_empty_level if the skillfly skip label is missing.
    assert!(
        b.lookup_label("level3_5.skillfly_bonus_skip").is_some(),
        "level3_5 skillfly bonus skip label missing"
    );

    let (data, labels) = b.finish();
    // C `register_level3_5_inline_callbacks()` registration-call order
    // (3-5 registers its own mapwaitboss trio, then the bonus guard).
    finish_level(
        data,
        labels,
        vec![
            (mapwaitboss_trigse_ptr, "level3_5_mapwaitboss_trigse"),
            (mapwaitboss_cantdie_ptr, "level3_5_mapwaitboss_cantdie"),
            (mapwaitboss_cleanup_ptr, "level3_5_mapwaitboss_cleanup"),
            (skillfly_bonus_guard_ptr, "level3_5_skillfly_bonus_guard"),
        ],
    )
}
