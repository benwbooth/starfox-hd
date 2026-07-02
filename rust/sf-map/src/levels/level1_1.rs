//! MAP_ID_1_1 — Corneria (Level 1, Route 1).
//!
//! C oracle: `src/map/levels.c` `build_level1_1_opening_slice()`,
//! `append_map1_1a_submap()` and `register_level1_1_inline_callbacks()`.
//!
//! ASM sources transcribed (via the C port):
//! - `LEVEL1_1.ASM` — level wrapper: scramble intro, base exit, jsr chain.
//! - `MAP1_1A.ASM`  — shared launch-corridor submap (`map1_1a` label).
//! - `MAP1_1B.ASM` / `1-1.ASM` — the main Corneria run + attack-carrier
//!   boss block (`level1_1.map1_1b` label).
//! - `CL_GND.ASM`   — ground clear demo (`cl_ground` label; inlined in the
//!   LEVEL1_1 tail exactly like the C build does).

use super::{BuiltLevel, InlineCallback, NativeCallback};
use crate::builder::MapBuilder;
use crate::consts::*;

/// C `build_level1_1_opening_slice()` + `register_level1_1_inline_callbacks()`.
pub fn build() -> BuiltLevel {
    let mut b = MapBuilder::new();

    // Literal LEVEL1_1.ASM slice through MAP1_1B.ASM, including the first
    // attack-carrier boss handoff. Opens with the scramble/launch intro:
    // `initlevel 1_1i` runs `pstrat playeropening` (started from boot),
    // then the wrapper jsrs into the shared MAP1_1A submap (appended below).
    b.mapwait(100);
    b.mapjsr("map1_1a");
    b.qfadedown();
    b.waitfade();
    b.mapcodejsl_builtin(cb::INITBLACK_L);
    b.mapwait(1);
    b.setbg(BG_1_1C);
    b.initbg();
    b.mapwait(MEDPSPEED * 2);
    b.qfadeup();
    let keep_player_strat_ptr = b.mapcode65816_inline();
    b.mapif_builtin(cb::IS_PLAYER_DEAD, "level1_1.after_exitbase_setup");
    b.mapcodejsl_builtin(cb::SET_PLAYER_EXITBASE_L);
    b.label("level1_1.after_exitbase_setup");

    b.mapobj(0, 0, 0, 0, sh::MYBASE_1, is::NOCOLL);
    b.mapobj(0, 0, 0, 0, sh::MYBASE_0, is::NOCOLL);

    b.mapobj(0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200, sh::MYSHIP_4, is::FRIENDEXITBASE);
    b.setalvarb(al::SBYTE1, 17);

    b.mapobj(0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200, sh::MYSHIP_4, is::FRIENDEXITBASE);
    b.setalvarb(al::SBYTE1, 17 + (1000 / PEXITBASE_SPEED));

    b.pathobj(0, 3000, 3000, 3000, sh::NULLSHAPE, path::MATEMSG, 10, 10);
    b.pathobj(0, 100, -90, 1400, sh::FRIENDSHIP_4, path::FALCO_LV1, 10, 10);
    b.pathobj(0, -80, -140, 1200, sh::FRIENDSHIP_4, path::FROG_LV1, 10, 10);

    b.mapobj(0, -600, 0, 2000, sh::BU_1, is::HARD180YR);
    b.mapobj(0, 600, 0, 2000, sh::BU_1, is::HARD180YR);
    b.mapobj(0, -800, 0, 3500, sh::BU_1, is::HARD180YR);
    b.mapobj(0, 800, 0, 3500, sh::BU_1, is::HARD180YR);

    b.label("level1_1.buloop");
    b.mapobj(0, -1000, 0, 5000, sh::BU_1, is::HARD180YR);
    b.mapobj(1500, 1000, 0, 5000, sh::BU_1, is::HARD180YR);
    b.maploop("level1_1.buloop", 3);

    b.cspecial(0, -500, -300, 0, sh::ZACO_5, is::ZACO1L);
    b.mapobj(0, -1100, 0, 5000, sh::BU_1, is::HARD180YR);
    b.mapobj(1500, 1100, 0, 5000, sh::BU_1, is::HARD180YR);
    b.mapobj(0, -1200, 0, 5000, sh::BU_1, is::HARD180YR);
    b.mapobj(0, 1200, 0, 5000, sh::BU_1, is::HARD180YR);

    b.pathobj(0, 0, -400, -100, sh::FRIENDSHIP_4, path::FROG1_1, 10, 10);
    b.mapcodejsl_builtin(cb::SETRESTART_L);
    b.mapcodejsl_builtin(cb::SET_PLAYER_ONPLANET_L);
    b.mapjsr("level1_1.map1_1b");

    b.mapobj(500, 1000, 0, 8000, sh::BU_6, is::HARD180YR);
    b.mapobj(1000, -800, 0, 10000, sh::BU_5, is::HARD180YR);
    b.mapobj(1000, -1200, 0, 12000, sh::BU_4, is::HARD180YR);
    b.mapjsr("cl_ground");
    b.emit8(op::END);

    // CL_GND.ASM shared clear-demo submap used by the LEVEL1_1 tail.
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
    b.mapobj(CL_GND_FRIENDWAIT, 500, -50, 50, sh::MYSHIP_4, is::CLSHIPGNDB);
    b.label("cl_ground.nf");

    b.mapif_builtin(cb::BUNNY_ALIVE, "cl_ground.bunny_alive");
    b.mapgoto("cl_ground.nb");
    b.label("cl_ground.bunny_alive");
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_BUNNY);
    b.mapobj(CL_GND_FRIENDWAIT, -500, -50, 50, sh::MYSHIP_4, is::CLSHIPGNDA);
    b.label("cl_ground.nb");

    b.mapif_builtin(cb::COCK_ALIVE, "cl_ground.cock_alive");
    b.mapgoto("cl_ground.nc");
    b.label("cl_ground.cock_alive");
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_COCK);
    b.mapobj(CL_GND_FRIENDWAIT, 0, -500, -300, sh::MYSHIP_4, is::CLSHIPGNDC);
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

    b.label("level1_1.map1_1b");

    // MAP1_1B.ASM -> INCMAP <1-1>, first bounded chunk from 1-1.ASM:7-15.
    b.pathobj(1000, 3000, 0, 1000, sh::NULLSHAPE, path::E_GATE, 10, 10);

    b.cspecial(0, -700, -500, 0, sh::ZACO_5, is::ZACO1L);
    b.mapobj(0, -1200, 0, 5000, sh::BU_1, is::HARD180YR);
    b.mapobj(1500, 1200, 0, 5000, sh::BU_1, is::HARD180YR);
    b.mapobj(0, -1200, 0, 5000, sh::BU_1, is::HARD180YR);
    b.mapobj(1500, 1200, 0, 5000, sh::BU_1, is::HARD180YR);

    b.mapobj(0, 0, 0, 4000, sh::ARCH_0, is::HARD);
    b.skillfly_init();
    b.skillfly_set(0, -60, 4000, 100);
    b.mapobj(0, -1200, 0, 5000, sh::BU_1, is::HARD180YR);
    b.mapobj(1500, 1200, 0, 5000, sh::BU_1, is::HARD180YR);

    b.mapobj(0, 200, 0, 4000, sh::ARCH_0, is::HARD);
    b.skillfly_set(200, -60, 4000, 100);
    b.mapobj(0, -1200, 0, 5000, sh::BU_1, is::HARD180YR);
    b.mapobj(1500, 1200, 0, 5000, sh::BU_1, is::HARD180YR);

    b.skillfly_set(-200, -60, 4000, 100);
    b.mapobj(800, -200, 0, 4000, sh::ARCH_0, is::HARD);

    b.special(200, 400, -400, 0, sh::ZACO_A, is::ZACO1R);
    b.mapobj(0, 350, -30 << 2, 4000, sh::RADER_0, is::RADER0);
    b.mapobj(1000, 350, 0, 4000, sh::RADER_1, is::RADER1);

    b.cspecial(1500, 400, -400, -250, sh::ZACO_5, is::ZACO1R);
    b.skillfly_set(0, -60, 4000, 100);
    b.mapobj(500, 0, 0, 4000, sh::ARCH_0, is::HARD);

    b.mapobj(0, -600, 0, 5000, sh::BU_0, is::HARD180YR);
    b.mapobj(1500, 600, 0, 5000, sh::BU_0, is::HARD180YR);
    b.mapobj(2000, 0, 0, 4500, sh::BIG_GATE, is::HARD);

    let skillfly_bonus_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, 0, -50, 2500, sh::ITEM_7, is::ITEM7);
    b.setalvarb(al::SBYTE1, 1);
    b.label("level1_1.map1_1b.skillfly_bonus_0_skip");

    b.pathcspecial(500, 200, -30, 3200, sh::BOM_WING, path::PONPON, 2, 8);
    b.mapobj(0, -600, 0, 5000, sh::BU_0, is::HARD180YR);
    b.mapobj(3000, 600, 0, 5000, sh::BU_0, is::HARD180YR);
    b.pathobj(0, -500, 0, 4000, sh::TOW_0, path::TOW_0, 10, 10);
    b.pathobj(1500, 500, 0, 4000, sh::TOW_0, path::TOW_0, 10, 10);
    b.special(0, 0, -1000, 2300, sh::ZACO_A, is::ZACOS);
    b.cspecial(0, -200, -1300, 2300, sh::ZACO_6, is::ZACOS);
    b.cspecial(3500, 200, -1300, 2300, sh::ZACO_6, is::ZACOS);
    b.mapobj(0, 800, 0, 4000, sh::TOWER_2, is::TOWER0);
    b.mapobj(3000, -800, 0, 4000, sh::TOWER_2, is::TOWER0);
    b.cspecial(0, -800, -300, 3000, sh::KAMIKAZE, is::ZACO4);
    b.pathobj(0, 0, 0, 4000, sh::TOW_0, path::TOW_0, 10, 10);
    b.mapobj(0, 1200, 0, 4000, sh::TOWER_2, is::TOWER0);
    b.mapobj(600, -1200, 0, 4000, sh::TOWER_2, is::TOWER0);
    b.cspecial(0, 800, -250, 3000, sh::KAMIKAZE, is::ZACO4);
    b.mapobj(0, 400, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapobj(3500, -400, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.pathobj(0, 1200, 0, 3500, sh::NULLSHAPE, path::ROBOTSWITHLOG, 6, 4);
    b.setalvarb(al::ROTY, 64);
    b.mapwait(0x0800);

    b.mapobj(0, 200, 0, 5000, sh::BU_8, is::HARD180YR);
    b.mapobj(2400, -200, 0, 5000, sh::BU_8, is::HARD180YR);
    b.pathobj(0, 750, -100, 0, sh::FRIENDSHIP_4, path::CHASE8_1, 10, 10);
    b.pathcspecial(0, 3800, -3600, 4260, sh::ZACO_A, path::CHASE8_2, 10, 10);
    b.mapobj(0, 800, 0, 5000, sh::BU_6, is::HARD180YR);
    b.pathcspecial(0, 0, 0, 5000, sh::WALKER_2, path::KORORI, 6, 4);
    b.mapobj(0, 200, -50, 5000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(0, -200, -50, 5000, sh::R_BU_1, is::HARD180YR);
    b.pathcspecial(2000, 750, -100, 0, sh::ZACO_A, path::CHASE8_3, 10, 10);
    b.mapobj(0, 200, -50, 5000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(0, -200, -50, 5000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(2000, -800, 0, 5000, sh::BU_2, is::HARD180YR);
    b.mapobj(0, 200, -50, 5000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(0, -200, -50, 5000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(0, 200, -50, 5000, sh::R_BU_1, is::HARD180YR);
    b.pathobj(0, -400, 0, 5000, sh::ROBOT_0, path::ROBOT, 6, 4);
    b.setalvarb(al::ROTY, -DEG45 - DEG22);
    b.mapobj(1000, -200, -50, 5000, sh::R_BU_1, is::HARD180YR);
    b.mapobj(0, 800, 0, 5000, sh::BU_5, is::HARD);
    b.setalvarb(al::ROTY, DEG90);
    b.mapwait(0x2000);

    b.mapexploderobot();
    b.mapobj(0, 820, 0, 4500, sh::BU_1, is::HARD180YR);
    b.mapobj(1400, -1200, 0, 4000, sh::BU_2, is::HARD180YR);
    b.cspecial(0, 300, -30, 4000, sh::BOM_WING, is::BOMWING);
    b.mapobj(0, -820, 0, 4500, sh::BU_1, is::HARD180YR);
    b.mapobj(2000, 820, 0, 4500, sh::BU_1, is::HARD180YR);
    b.mapobj(0, -900, 0, 5000, sh::BU_0, is::HARD180YR);
    b.mapobj(2800, 900, 0, 5000, sh::BU_0, is::HARD180YR);

    b.mapobj(0, -1000, 0, 4500, sh::BU_1, is::HARD180YR);
    b.mapobj(0, 1000, 0, 4500, sh::BU_6, is::HARD180YR);
    b.mapobj(0, -800, 0, 5000, sh::BU_2, is::HARD180YR);
    b.mapobj(0, 500, 0, 5000, sh::BU_5, is::HARD);
    b.setalvarb(al::ROTY, DEG90);
    b.mapobj(2000, -350, 0, 5000, sh::BU_4, is::HARD180YR);
    b.mapobj(0, -400, 0, 4000, sh::BU_4, is::HARD180YR);
    b.mapobj(0, 400, 0, 4000, sh::BU_4, is::HARD);
    b.setalvarb(al::ROTY, DEG90);
    b.mapwait(1400);
    b.mapobj(1200, 0, 0, 4000, sh::BU_7, is::HARD180YR);
    b.mapobj(0, -1000, 0, 4000, sh::BU_6, is::HARD180YR);
    b.mapobj(600, 1000, 0, 4000, sh::BU_6, is::HARD180YR);
    b.mapobj(0, -450, 0, 4000, sh::BU_6, is::HARD180YR);
    b.mapobj(700, 450, 0, 4000, sh::BU_6, is::HARD180YR);

    b.pathcspecial(1000, -1800, -600, 2000, sh::ZACO_5, path::PATROL, 10, 10);
    b.mapobj(1200, 100, 0, 4000, sh::BU_7, is::HARD180YR);
    b.mapobj(0, -1000, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(600, 1000, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(0, -400, 0, 4000, sh::BU_4, is::HARD180YR);
    b.mapobj(600, 450, 0, 4000, sh::BU_6, is::HARD180YR);
    b.mapobj(600, -400, 0, 4000, sh::BU_4, is::HARD180YR);
    b.mapobj(0, 450, 0, 4000, sh::BU_6, is::HARD180YR);
    b.mapobj(1400, -400, 0, 4000, sh::BU_4, is::HARD180YR);
    b.mapobj(1000, 0, 0, 4000, sh::BU_7, is::HARD180YR);
    b.mapobj(0, -900, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(600, 900, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(0, -400, 0, 4000, sh::BU_4, is::HARD180YR);
    b.mapobj(1000, 400, 0, 4000, sh::BU_5, is::HARD90YR);
    b.mapobj(0, 440, -230, 4050, sh::ITEM_5, is::ITEM5);
    b.setalvarb(al::SBYTE1, 1);
    b.mapobj(0, 400, 0, 4000, sh::BU_5, is::HARD90YR);
    b.mapobj(800, -400, 0, 4000, sh::BU_4, is::HARD180YR);
    b.pathcspecial(400, -1500, -700, 2000, sh::ZACO_5, path::PATROL, 10, 10);
    b.mapobj(0, 1000, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(500, -1000, 0, 4000, sh::BU_0, is::HARD180YR);
    b.mapobj(0, 350, 0, 4000, sh::BU_4, is::HARD);
    b.setalvarb(al::ROTY, DEG90);
    b.pathcspecial(0, 0, 0, 5000, sh::WALKER_2, path::KORORI, 6, 4);
    b.mapobj(800, -350, 0, 4000, sh::BU_4, is::HARD180YR);
    b.mapobj(0, 350, 0, 4000, sh::BU_4, is::HARD);
    b.setalvarb(al::ROTY, DEG90);
    b.mapobj(0, -350, 0, 4000, sh::BU_4, is::HARD180YR);
    b.pathcspecial(500, 2000, -500, 2000, sh::ZACO_5, path::PATROL, 10, 10);
    b.mapwait(800);
    b.pathobj(0, 1300, 0, 3800, sh::NULLSHAPE, path::ROBOTWITHLOG2, 6, 4);
    b.setalvarb(al::ROTY, DEG90);
    b.mapwait(500);
    b.pathcspecial(0, 0, 0, 5000, sh::WALKER_2, path::KORORI, 6, 4);
    b.pathcspecial(2000, -200, -50, 3200, sh::BOM_WING, path::PONPON, 2, 8);
    b.mapnobj(1000, 0, -100, 4000, sh::GATE_0, STRAT_ADDR_GATE3);

    b.mapobj(0, -900, 0, 5000, sh::BU_0, is::HARD180YR);
    b.mapobj(1000, 900, 0, 5000, sh::BU_0, is::HARD180YR);
    b.mapobj(800, 350, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapobj(800, -350, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapobj(0, -900, 0, 5000, sh::BU_0, is::HARD180YR);
    b.mapobj(0, 900, 0, 5000, sh::BU_0, is::HARD180YR);
    b.mapobj(800, 300, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapobj(800, -250, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapobj(0, -900, 0, 5000, sh::BU_0, is::HARD180YR);
    b.mapobj(0, 900, 0, 5000, sh::BU_0, is::HARD180YR);
    b.mapobj(800, 250, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapobj(100, -200, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapobj(1500, 200, 0, 4000, sh::PILLAR3, is::PILLAR3);

    b.cspecial(100, 400, -600, -200, sh::ZACO_5, is::ZACO1R);
    b.cspecial(800, -400, -800, -200, sh::ZACO_5, is::ZACO1L);

    b.pathobj(0, -1000, 0, 3500, sh::NULLSHAPE, path::ROBOTWITHLOG, 6, 4);
    b.setalvarb(al::ROTY, -DEG45 - DEG22);

    b.mapobj(0, -1000, 0, 6000, sh::BU_5, is::HARD180YR);
    b.mapobj(0, 1000, 0, 6000, sh::BU_4, is::HARD);
    b.setalvarb(al::ROTY, 64);
    b.mapwait(1000);
    b.pathobj(0, -750, -400, 0, sh::FRIENDSHIP_4, path::CHASE6_1, 10, 10);
    b.pathcspecial(2000, -720, -400, 0, sh::ZACO_A, path::CHASE6_2, 10, 10);
    b.mapobj(1000, -1000, 0, 6000, sh::BU_5, is::HARD180YR);
    b.mapobj(0, 1300, 0, 6000, sh::BU_5, is::HARD);
    b.setalvarb(al::ROTY, 64);
    b.mapwait(2000);
    b.pathcspecial(0, 200, -50, 3200, sh::BOM_WING, path::PONPON, 2, 8);

    b.pathobj(0, 800, 0, 3500, sh::NULLSHAPE, path::ROBOTWITHLOG, 6, 4);
    b.setalvarb(al::ROTY, DEG45 + DEG22);
    b.mapobj(1000, -1000, 0, 6000, sh::BU_4, is::HARD180YR);
    b.mapobj(0, 1300, 0, 6000, sh::BU_4, is::HARD);
    b.setalvarb(al::ROTY, 64);

    b.mapobj(0, 0, -150, 4000, sh::GATE_0, is::GATE);
    b.pathobj(1000, 3000, 0, 1000, sh::NULLSHAPE, path::E_GATE, 10, 10);
    b.pathcspecial(1000, -250, -1800, 0, sh::CARRIER, path::E_UFO, 10, 10);
    b.mapobj(0, 1300, 0, 6000, sh::BU_2, is::HARD180YR);
    b.mapobj(2000, -1300, 0, 6000, sh::BU_2, is::HARD180YR);
    b.special(400, -400, -200, -200, sh::ZACO_A, is::ZACO1L);
    b.mapobj(0, -1300, 0, 7000, sh::BU_5, is::HARD180YR);
    b.mapobj(0, 1300, 0, 7000, sh::BU_4, is::HARD);
    b.setalvarb(al::ROTY, 120);
    b.mapwait(3000);
    b.mapobj(0, 1300, 0, 7000, sh::BU_6, is::HARD180YR);
    b.mapobj(3000, -1300, 0, 7000, sh::BU_6, is::HARD180YR);
    b.mapobj(0, 1300, 0, 6000, sh::BU_4, is::HARD);
    b.setalvarb(al::ROTY, 120);
    b.mapobj(4000, -1300, 0, 6000, sh::BU_5, is::HARD180YR);
    b.pathobj(0, -350, 0, 4000, sh::ROBOT_0, path::ROBOT, 6, 4);
    b.setalvarb(al::ROTY, -DEG45);
    b.mapwait(4000);

    // MAP1_1B.ASM boss block.
    b.setbgm(BGM_FADEOUT);
    b.mapwait(MEDPSPEED * 30);
    b.setbgm(BGM_BOSS1);
    b.mapobj(0, 0, -(70 << BOSS7_SCALE), -200, sh::BOSS_7_1, is::BOSS7);

    let mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("level1_1.map1_1b.bosswait.loop");
    b.mapif_builtin(cb::CHKBOSSDEAD, "level1_1.map1_1b.bosswait.cont");
    b.mapgoto("level1_1.map1_1b.bosswait.loop");
    b.label("level1_1.map1_1b.bosswait.cont");
    let mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let mapwaitboss_cleanup_ptr = b.mapcode65816_inline();
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(cb::MARKBOSS_L);

    b.maprts();

    append_map1_1a_submap(&mut b);

    b.resolve();

    // C: s_level1_1_skillfly_bonus_skip_ptr lookup (must exist).
    assert!(
        b.lookup_label("level1_1.map1_1b.skillfly_bonus_0_skip").is_some(),
        "level1_1 skillfly bonus skip label missing"
    );

    let (data, labels) = b.finish();

    // C `register_level1_1_inline_callbacks()` — registration-call order.
    let mut inline_callbacks = Vec::new();
    for (ptr, cbid) in [
        (keep_player_strat_ptr, InlineCallback::LevelScrambleKeepPlayerStrat),
        (skillfly_bonus_guard_ptr, InlineCallback::Level1_1SkillflyBonusGuard),
        (mapwaitboss_trigse_ptr, InlineCallback::Level1_1MapwaitbossTrigse),
        (mapwaitboss_cantdie_ptr, InlineCallback::Level1_1MapwaitbossCantdie),
        (mapwaitboss_cleanup_ptr, InlineCallback::Level1_1MapwaitbossCleanup),
    ] {
        if ptr != 0 {
            inline_callbacks.push((ptr, cbid));
        }
    }

    BuiltLevel {
        data,
        labels,
        native_callbacks: vec![
            (cb::CL_GROUND_PRINTLEVELFIN, NativeCallback::ClGroundPrintlevelfin),
            (cb::CL_GROUND_WIPEOUT, NativeCallback::ClGroundWipeout),
            (cb::CL_DIVE_CLEAR_ENGINESND, NativeCallback::ClDiveClearEnginesnd),
        ],
        inline_callbacks,
    }
}

/// C `append_map1_1a_submap()` — MAP1_1A.ASM: the shared launch-corridor
/// submap (base tunnel `op_0/op_1` segments, escorting Arwing intro ships,
/// looped corridor extension, CHKSTRATDONE1 exit).
fn append_map1_1a_submap(b: &mut MapBuilder) {
    b.label("map1_1a");
    b.mapobj(0, 0, 0, 250, sh::OP_0, is::GND);
    b.setalxvarb(alx::DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250, sh::OP_1, is::NOCOLL);
    b.setalxvarb(alx::DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (100 << 3), sh::OP_0, is::GND);
    b.setalxvarb(alx::DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (100 << 3), sh::OP_1, is::NOCOLL);
    b.setalxvarb(alx::DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (200 << 3), sh::OP_0, is::GND);
    b.setalxvarb(alx::DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (200 << 3), sh::OP_1, is::NOCOLL);
    b.setalxvarb(alx::DEPTHOFFSET, 1);

    b.mapobj(0, -40, 0, -200, sh::IMYSHIP_4, is::SHIPINTRO);
    b.setalvarw(al::SWORD1, -70);
    b.setalvarb(al::SBYTE1, 60);
    b.mapobj(0, 40, 0, -200, sh::IMYSHIP_4, is::SHIPINTRO);
    b.setalvarw(al::SWORD1, -70);
    b.setalvarb(al::SBYTE1, 50);
    b.mapobj(0, 0, 0, -300, sh::IMYSHIP_4, is::SHIPINTRO);
    b.setalvarw(al::SWORD1, -100);
    b.setalvarb(al::SBYTE1, -1);

    b.label("map1_1a.here2");
    b.mapwait((100 << 3) - MEDPSPEED);
    b.mapobj(0, 0, 0, 250 + (200 << 3), sh::OP_0, is::GND);
    b.setalxvarb(alx::DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (200 << 3), sh::OP_1, is::NOCOLL);
    b.setalxvarb(alx::DEPTHOFFSET, 1);
    b.maploop("map1_1a.here2", 8);

    b.label("map1_1a.here3");
    b.mapwait((100 << 3) - MEDPSPEED);
    b.mapobj(0, 0, 0, 250 + (200 << 3), sh::OP_2, is::GND);
    b.setalxvarb(alx::DEPTHOFFSET, 1);
    b.mapif_builtin(cb::CHKSTRATDONE1, "map1_1a.fin");
    b.mapgoto("map1_1a.here3");
    b.label("map1_1a.fin");
    b.maprts();
}
