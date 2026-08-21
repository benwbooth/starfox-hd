//! MAP_ID_2_1 — Asteroid route Corneria (Level 2, Route 2).
//!
//! C oracle: `src/map/levels.c` `build_level2_1_wrapper_slice()` +
//! `register_level2_1_inline_callbacks()`.
//! ASM sources: LEVEL2_1.ASM wrapper, MAP2_1B / 2-1.ASM body, shared
//! MAP1_1A.ASM launch corridor and CL_EARTH.ASM clear demo.

use super::rc::*;
use super::submaps;
use super::Route2Level;
use crate::builder::MapBuilder;
use crate::consts::{op, SCRAMBLE_WIPE_DISTANCE};

/// C `build_level2_1_wrapper_slice()`.
pub fn build() -> Route2Level {
    let mut b = MapBuilder::new();

    b.mapwait(100);
    b.mapjsr("map1_1a");
    b.qfadedown();
    b.waitfade();
    b.mapcodejsl_builtin(MAP_CB_INITBLACK_L);
    b.mapwait(1);
    b.setbg(BG_1_1C);
    b.initbg();
    b.mapcodejsl_builtin(MAP_CB_INITBLACK_L);
    b.mapwait(SCRAMBLE_WIPE_DISTANCE);
    b.mapwait(MEDPSPEED * 2);
    b.qfadeup();
    let level2_1_keep_player_strat_ptr = b.mapcode65816_inline();
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_EXITBASE_L);

    b.mapobj(0, 0, 0, 0, SH_MYBASE_1, IS_NOCOLL);
    b.mapobj(0, 0, 0, 0, SH_MYBASE_0, IS_NOCOLL);

    b.mapobj(
        0,
        -27 << MYBASE_SCALE,
        -39 << MYBASE_SCALE,
        -200,
        SH_MYSHIP_4,
        IS_FRIENDEXITBASE,
    );
    b.setalvarb(AL_SBYTE1, 17);
    b.mapobj(
        0,
        -27 << MYBASE_SCALE,
        -39 << MYBASE_SCALE,
        -200,
        SH_MYSHIP_4,
        IS_FRIENDEXITBASE,
    );
    b.setalvarb(AL_SBYTE1, 17 + (1000 / PEXITBASE_SPEED));

    b.pathobj(0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_MATEMSG, 10, 10);
    b.pathobj(
        0,
        100,
        -90,
        1400,
        SH_FRIENDSHIP_4,
        PATH_ID_FALCO_LV1,
        10,
        10,
    );
    b.pathobj(
        0,
        -80,
        -140,
        1200,
        SH_FRIENDSHIP_4,
        PATH_ID_FROG_LV1,
        10,
        10,
    );

    b.mapobj(0, -600, 0, 2000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, 600, 0, 2000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, -700, 0, 3500, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, 700, 0, 3500, SH_BU_1, IS_HARD180YR);

    b.label("level2_1.tower");
    b.mapobj(0, -1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(2000, 1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    b.maploop("level2_1.tower", 2);
    b.cspecial(0, -500, -300, 0, SH_ZACO_5, IS_ZACO1L);
    b.mapobj(0, -1200, 0, 5000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(2000, 1200, 0, 5000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(0, -1200, 0, 5000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(0, 1200, 0, 5000, SH_TOWER_2, IS_TOWER0);
    b.cspecial(0, 500, -300, 0, SH_ZACO_5, IS_ZACO1R);

    b.mapcodejsl_builtin(MAP_CB_SETRESTART_L);
    b.mapif_builtin(MAP_CB_IS_PLAYER_DEAD, "level2_1.after_onplanet_setup");
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_ONPLANET_L);
    b.label("level2_1.after_onplanet_setup");
    b.mapjsr("level2_1.map2_1b");
    b.emit8(op::END);

    b.label("level2_1.map2_1b");
    // MAP2_1B / 2-1.ASM literal body slice through the opening 2-1-3 block.
    b.mapcodejsl_builtin(MAP_CB_SETRESTART_L);
    b.pathobj(1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    b.cspecial(0, -700, -500, 0, SH_ZACO_5, IS_ZACO1L);
    b.mapobj(0, -1200, 0, 5200, SH_TOWER_2, IS_TOWER0);
    b.mapobj(2000, 1200, 0, 5200, SH_TOWER_2, IS_TOWER0);
    b.mapobj(0, -1200, 0, 5500, SH_TOWER_2, IS_TOWER0);
    b.mapobj(2000, 1200, 0, 5500, SH_TOWER_2, IS_TOWER0);

    b.cspecial(0, -200, -600, -500, SH_ZACO_5, IS_ZACO1L);
    b.pathobj(0, -500, 0, 5000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    b.pathobj(0, 500, 0, 5000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    b.mapobj(0, -600, 0, 4000, SH_HOUDAI_0, IS_HOUDAINS);
    b.cspecial(1000, -500, -200, 4000, SH_KAMIKAZE, IS_ZACO3);

    b.pathobj(0, -850, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    b.pathcspecial(0, -820, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);

    b.pathcspecial(400, 0, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    b.mapobj(0, -1000, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.mapobj(2000, 1000, 0, 5000, SH_BU_0, IS_HARD180YR);

    b.mapobj(0, -300, -110, 4000, SH_RADER_0, IS_RADER0);
    b.mapobj(0, -300, 0, 4000, SH_RADER_1, IS_RADER1);
    b.mapobj(0, 300, -110, 4000, SH_RADER_0, IS_RADER0);
    b.mapobj(1000, 300, 0, 4000, SH_RADER_1, IS_RADER1);
    b.skillfly_init();
    b.skillfly_set(0, -60, 4000, 100);
    b.mapobj(1000, 0, 0, 4000, SH_ARCH_0, IS_HARD);

    b.pathobj(0, -800, 0, 5000, SH_ROBOT_0, PATH_ID_ROBOT, 6, 4);
    b.setalvarb(AL_ROTY, -DEG45);
    b.mapobj(0, 1000, 0, 5000, SH_BU_5, IS_HARD);
    b.setalvarb(AL_ROTY, 96);
    b.mapwait(2000);
    b.mapobj(0, -1000, 0, 5000, SH_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 64);
    b.mapobj(0, -200, 0, 4000, SH_ARCH_0, IS_HARD);
    b.skillfly_set(-200, -60, 4000, 100);
    b.mapwait(2000);

    b.pathobj(0, 1600, 0, 4000, SH_NULLSHAPE, PATH_ID_ROBOTWITHLOG, 6, 4);
    b.setalvarb(AL_ROTY, DEG45 + DEG22);
    b.mapobj(1200, 800, 0, 5000, SH_BU_6, IS_HARD180YR);
    b.skillfly_set(-300, -60, 4000, 100);
    b.mapobj(1200, -300, 0, 4000, SH_ARCH_0, IS_HARD);
    b.mapobj(2000, -700, 0, 5000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, 700, 0, 5000, SH_BU_4, IS_HARD180YR);
    b.setalvarb(AL_ROTY, 96);
    b.skillfly_set(200, -60, 4000, 100);
    b.mapobj(2000, 200, 0, 4000, SH_ARCH_0, IS_HARD);

    b.mapobj(400, -800, 0, 4000, SH_BU_6, IS_HARD180YR);
    b.mapobj(400, 800, 0, 4000, SH_BU_6, IS_HARD180YR);
    b.special(0, -300, -1300, 1800, SH_CARRIER, IS_CARRIER);
    b.mapobj(0, -600, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(400, 600, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, -500, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(400, 500, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, -400, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(400, 400, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, 200, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(3000, -200, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.cspecial(0, 0, -400, 1000, SH_PARA_0, IS_PARA);
    b.cspecial(0, 100, -500, 1000, SH_PARA_0, IS_PARA);
    b.cspecial(500, -100, -500, 1000, SH_PARA_0, IS_PARA);

    b.mapobj(0, -400, 0, 2000, SH_BU_1, IS_HARD180YR);
    b.mapobj(500, 400, 0, 2000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, -600, 0, 4000, SH_BU_5, IS_HARD);
    b.setalvarb(AL_ROTY, 96);
    b.mapobj(0, 600, 0, 4000, SH_BU_4, IS_HARD);
    b.setalvarb(AL_ROTY, 96);
    b.mapwait(1000);
    b.mapobj(0, -600, 0, 4000, SH_BU_5, IS_HARD);
    b.setalvarb(AL_ROTY, 96);
    b.mapobj(0, 600, 0, 4000, SH_BU_4, IS_HARD);
    b.setalvarb(AL_ROTY, 96);
    b.mapwait(1000);
    b.mapobj(0, 400, 0, 4000, SH_BU_6, IS_HARD180YR);
    b.setalvarb(AL_ROTY, 96);
    b.mapobj(0, -400, 0, 4000, SH_BU_6, IS_HARD180YR);
    b.setalvarb(AL_ROTY, 96);
    b.mapwait(800);
    b.mapobj(1000, 0, 0, 4000, SH_BU_7, IS_HARD180YR);
    b.mapobj(0, 200, 0, 4000, SH_BU_7, IS_HARD180YR);
    b.mapobj(1000, -200, 0, 4000, SH_BU_7, IS_HARD180YR);
    b.mapobj(0, -700, 0, 3500, SH_BU_2, IS_HARD180YR);
    b.mapobj(0, 700, 0, 3500, SH_BU_2, IS_HARD180YR);
    b.mapobj(0, 400, 0, 4000, SH_BU_4, IS_HARD);
    b.setalvarb(AL_ROTY, 96);
    b.mapobj(0, -400, 0, 4000, SH_BU_4, IS_HARD);
    b.setalvarb(AL_ROTY, 96);
    b.mapwait(1000);
    b.mapobj(0, 400, 0, 4000, SH_BU_4, IS_HARD);
    b.setalvarb(AL_ROTY, 96);

    b.cspecial(0, -1800, -600, 4400, SH_ZACO_5, IS_ZACO0);
    b.mapobj(0, -400, 0, 4000, SH_BU_4, IS_HARD);
    b.setalvarb(AL_ROTY, 96);
    b.mapwait(2000);

    b.cspecial(100, 0, -1400, 2100, SH_ZACO_6, IS_ZACOS);
    b.special(0, -150, -1500, 2300, SH_ZACO_A, IS_ZACOS);
    b.special(100, 150, -1500, 2300, SH_ZACO_A, IS_ZACOS);
    b.cspecial(0, 300, -1700, 2600, SH_ZACO_6, IS_ZACOS);
    b.cspecial(2000, -300, -1700, 2600, SH_ZACO_6, IS_ZACOS);
    b.mapobj(0, -1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(2000, 1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(0, -1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(2000, 1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    b.pathobj(0, 300, 0, 4000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    b.pathobj(3000, -300, 0, 4000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    b.mapobj(0, 600, 0, 4000, SH_BU_4, IS_HARD180YR);
    b.mapobj(1000, -600, 0, 4000, SH_BU_5, IS_HARD180YR);
    b.mapobj(0, 300, 0, 4000, SH_BU_6, IS_HARD180YR);
    b.mapobj(400, -300, 0, 4000, SH_BU_6, IS_HARD180YR);

    b.skillfly_init();
    b.skillfly_set(0, -60, 4000, 20 << 2);
    b.mapobj(200, 0, 0, 4000, SH_ARCH_0, IS_HARD);
    b.pathobj(0, 0, -350, -150, SH_FRIENDSHIP_4, PATH_ID_CHASE7_1, 10, 10);
    b.pathcspecial(3000, 0, -350, -150, SH_ZACO_A, PATH_ID_CHASE7_2, 10, 10);
    b.pathcspecial(0, 150, -50, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);

    b.maphardrot(0, 400, 0, 4000, SH_TOWER_2, 0, 4, 0);
    b.maphardrot(800, -400, 0, 4000, SH_TOWER_2, 0, -4, 0);

    let level2_1_skillfly_bonus0_guard_ptr = b.mapcode65816_inline();
    b.mapnobj(0, 0, -80, 1500, SH_GATE_0, STRAT_ADDR_GATE3);
    b.label("level2_1.map2_1b.skillfly_bonus_0_skip");

    b.skillfly_set(0, -60, 4000, 20 << 2);
    b.mapobj(1500, 0, 0, 4000, SH_ARCH_0, IS_HARD);
    b.mapobj(0, 100, 0, 4000, SH_BU_8, IS_HARD180YR);
    b.mapobj(500, -100, 0, 4000, SH_BU_8, IS_HARD180YR);
    b.maphardrot(0, 480, 0, 4000, SH_TOWER_2, 0, 6, 0);
    b.maphardrot(1000, -480, 0, 4000, SH_TOWER_2, 0, -6, 0);
    b.skillfly_set(0, -60, 4000, 20 << 2);
    b.mapobj(1000, 0, 0, 4000, SH_ARCH_0, IS_HARD);

    b.skillfly_set(250, -60, 4000, 100);
    b.pathobj(0, -1500, 0, 4000, SH_NULLSHAPE, PATH_ID_ROBOTSWITHLOG, 6, 4);
    b.setalvarb(AL_ROTY, -DEG90);
    b.pathcspecial(0, 300, -50, 3500, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    b.cspecial(2200, -100, -30, 3500, SH_BOM_WING, IS_BOMWING);

    b.mapobj(0, 1400, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(1200, -1400, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, 1200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(1200, -1200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    let level2_1_skillfly_bonus1_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, 100, -80, 1500, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.label("level2_1.map2_1b.skillfly_bonus_1_skip");
    b.mapobj(0, 1000, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(1200, -1000, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, 800, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(1200, -800, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, 600, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(1400, -500, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, -900, 0, 5000, SH_BU_2, IS_HARD180YR);
    b.mapobj(0, 900, 0, 5000, SH_BU_2, IS_HARD180YR);
    b.mapobj(0, 400, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(1200, -400, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, -300, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(1000, 300, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, 250, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(1000, -250, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, 200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(900, -200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, -700, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.mapobj(0, 700, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.mapobj(0, 150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(800, -150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, 150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(700, -150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, 200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(600, -200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.pathcspecial(0, 300, -50, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);
    b.mapobj(0, 150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(1000, -150, 0, 4000, SH_PILLAR3, IS_PILLAR3);

    b.mapobj(0, 0, -150, 4000, SH_GATE_0, IS_GATE);
    b.pathobj(1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);
    b.mapwait(1000);
    b.mapobj(0, 240, 0, 4000, SH_BU_4, IS_HARD);
    b.setalvarb(AL_ROTY, 64);
    b.mapobj(0, -240, 0, 4000, SH_BU_4, IS_HARD180YR);
    b.mapobj(0, 0, -120, 4200, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.pathcspecial(1400, -1500, -700, 2000, SH_ZACO_5, PATH_ID_PATROL, 2, 10);
    // Finish the remaining 2-1.ASM tail, then fall through into MAP2_1B.ASM.
    b.pathcspecial(1000, 0, -50, 3500, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    b.mapwait(1000);
    b.special(0, 0, -1300, 2000, SH_ZACO_A, IS_ZACOS);
    b.cspecial(0, 200, -1500, 2200, SH_ZACO_6, IS_ZACOS);
    b.cspecial(100, -200, -1500, 2200, SH_ZACO_6, IS_ZACOS);
    b.cspecial(0, 300, -1700, 2400, SH_ZACO_6, IS_ZACOS);
    b.cspecial(2500, -300, -1700, 2400, SH_ZACO_6, IS_ZACOS);

    b.mapobj(0, 1400, 0, 4000, SH_BU_6, IS_HARD180YR);
    b.setalvarb(AL_ROTY, 96);
    b.mapobj(0, -1400, 0, 4000, SH_BU_6, IS_HARD180YR);
    b.setalvarb(AL_ROTY, 96);
    b.mapwait(2500);
    b.pathobj(0, 800, 0, 5000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    b.pathobj(4500, -800, 0, 5000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);

    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_BOSS1);
    b.mapobj(0, 0, -(70 << BOSS7_SCALE), -200, SH_BOSS_7_1, IS_BOSS7);
    let level2_1_mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("level2_1.map2_1b.bosswait.loop");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, "level2_1.map2_1b.bosswait.cont");
    b.mapgoto("level2_1.map2_1b.bosswait.loop");
    b.label("level2_1.map2_1b.bosswait.cont");
    let level2_1_mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let level2_1_mapwaitboss_cleanup_ptr = b.mapcode65816_inline();
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);
    b.maprts();

    submaps::append_cl_earth_submap(&mut b);
    submaps::append_map1_1a_submap(&mut b);

    b.resolve();

    // C: skillfly bonus skip-label lookups (build fails if missing).
    assert!(b
        .lookup_label("level2_1.map2_1b.skillfly_bonus_0_skip")
        .is_some());
    assert!(b
        .lookup_label("level2_1.map2_1b.skillfly_bonus_1_skip")
        .is_some());

    let (data, labels) = b.finish();

    // C `register_level2_1_inline_callbacks()` — registration-call order.
    Route2Level::new(
        data,
        labels,
        vec![],
        vec![
            (
                level2_1_keep_player_strat_ptr,
                "level_scramble_keep_player_strat",
            ),
            (
                level2_1_skillfly_bonus0_guard_ptr,
                "level2_1_skillfly_bonus0_guard",
            ),
            (
                level2_1_skillfly_bonus1_guard_ptr,
                "level2_1_skillfly_bonus1_guard",
            ),
            (
                level2_1_mapwaitboss_trigse_ptr,
                "level1_1_mapwaitboss_trigse",
            ),
            (
                level2_1_mapwaitboss_cantdie_ptr,
                "level1_1_mapwaitboss_cantdie",
            ),
            (
                level2_1_mapwaitboss_cleanup_ptr,
                "level1_1_mapwaitboss_cleanup",
            ),
        ],
    )
}
