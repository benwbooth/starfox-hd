//! MAP_ID_3_1 — Corneria (Level 3, Route 3).
//!
//! C oracle: `src/map/levels.c` `build_level3_1_wrapper_slice()` +
//! `register_level3_1_inline_callbacks()`.
//! ASM: LEVEL3_1.ASM / MAP1_1A.ASM / 3-1.ASM / CL_CHASE.ASM.

use super::common::*;
use super::finish_level;
use super::Route3Level;
use crate::builder::MapBuilder;

pub(crate) fn build() -> Route3Level {
    let mut b = MapBuilder::new();

    // LEVEL3_1.ASM through the first handoff into MAP3_1B.
    b.mapwait(100);
    b.mapjsr("map1_1a");
    b.qfadedown();
    b.waitfade();
    b.mapcodejsl_builtin(MAP_CB_INITBLACK_L);
    b.mapwait(1);
    b.setbg(BG_3_1C);
    b.initbg();
    b.mapwait(MEDPSPEED * 2);
    b.qfadeup();
    let keep_player_strat_ptr = b.mapcode65816_inline(); 
    b.mapif_builtin(MAP_CB_IS_PLAYER_DEAD, "level3_1.after_exitbase_setup");
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_EXITBASE_L);
    b.label("level3_1.after_exitbase_setup");

    b.mapobj(0, 0, 0, 0, SH_MYBASE_1, IS_NOCOLL);
    b.mapobj(0, 0, 0, 0, SH_MYBASE_0, IS_NOCOLL);

    b.mapobj(0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200, SH_MYSHIP_4, IS_FRIENDEXITBASE);
    b.setalvarb(AL_SBYTE1, 17);
    b.mapobj(0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200, SH_MYSHIP_4, IS_FRIENDEXITBASE);
    b.setalvarb(AL_SBYTE1, 17 + (1000 / PEXITBASE_SPEED));

    b.pathobj(0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_MATEMSG, 10, 10);
    b.pathobj(0, 100, -90, 1400, SH_FRIENDSHIP_4, PATH_ID_FALCO_LV1, 10, 10);
    b.pathobj(0, -80, -140, 1200, SH_FRIENDSHIP_4, PATH_ID_FROG_LV1, 10, 10);

    b.mapobj(0, -600, 0, 2000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, 600, 0, 2000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, -700, 0, 3500, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, 700, 0, 3500, SH_BU_1, IS_HARD180YR);

    b.mapobj(0, -900, 0, 5000, SH_BU_1, IS_HARD180YR);
    b.mapobj(2000, 900, 0, 5000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, -1100, 0, 4800, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, 1100, 0, 4800, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, -500, 0, 5000, SH_HOUDAI_0, IS_HOUDAINS);
    b.mapobj(2000, 500, 0, 5000, SH_HOUDAI_0, IS_HOUDAINS);

    b.cspecial(0, -500, -300, 0, SH_ZACO_5, IS_ZACO1L);
    b.mapobj(0, -1200, 0, 4800, SH_BU_1, IS_HARD180YR);
    b.mapobj(2000, 1200, 0, 4800, SH_BU_1, IS_HARD180YR);
    b.cspecial(0, 500, -300, 0, SH_ZACO_5, IS_ZACO1R);
    b.mapobj(0, -1200, 0, 4800, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, 1200, 0, 4800, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, -600, 0, 5000, SH_HOUDAI_0, IS_HOUDAINS);
    b.mapobj(0, 600, 0, 5000, SH_HOUDAI_0, IS_HOUDAI);

    b.mapcodejsl_builtin(MAP_CB_SETRESTART_L);
    b.mapif_builtin(MAP_CB_IS_PLAYER_DEAD, "level3_1.after_onplanet_setup");
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_ONPLANET_L);
    b.label("level3_1.after_onplanet_setup");
    b.mapjsr("level3_1.map3_1b");
    b.emit8(op::END);

    b.label("level3_1.map3_1b");
    // 3-1.ASM literal opening through the first 3-1-2 friend block.
    b.mapcodejsl_builtin(MAP_CB_SETRESTART_L);
    b.pathobj(1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    b.cspecial(0, -200, -700, -500, SH_ZACO_5, IS_ZACO1L);
    b.special(0, 200, -900, -500, SH_ZACO_A, IS_ZACO1R);
    b.label("level3_1.map3_1b.houdai");
    b.mapobj(0, -700, 0, 5000, SH_HOUDAI_0, IS_HOUDAINS);
    b.mapobj(0, 700, 0, 5000, SH_HOUDAI_0, IS_HOUDAINS);
    b.mapobj(0, -1200, 0, 4800, SH_BU_1, IS_HARD180YR);
    b.mapobj(1500, 1200, 0, 4800, SH_BU_1, IS_HARD180YR);
    b.maploop("level3_1.map3_1b.houdai", 2);

    b.skillfly_init();
    b.skillfly_set(0, -60, 4000, 100);
    b.mapobj(0, 0, 0, 4000, SH_ARCH_0, IS_HARD);
    b.mapobj(0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    b.mapobj(1500, 1200, 0, 5000, SH_BU_1, IS_HARD180YR);

    b.mapobj(0, 400, -110, 4000, SH_RADER_0, IS_RADER0);
    b.mapobj(0, 400, 0, 4000, SH_RADER_1, IS_RADER1);
    b.mapobj(0, -400, -110, 4000, SH_RADER_0, IS_RADER0);
    b.mapobj(1500, -400, 0, 4000, SH_RADER_1, IS_RADER1);

    b.mapobj(0, 0, -110, 4000, SH_RADER_0, IS_RADER0);
    b.mapobj(2000, 0, 0, 4000, SH_RADER_1, IS_RADER1);
    b.mapobj(0, -250, 0, 3200, SH_HOUDAI_0, IS_HOUDAI);
    b.mapobj(1200, 250, 0, 3200, SH_HOUDAI_0, IS_HOUDAI);
    b.cspecial(200, -600, -700, -200, SH_ZACO_5, IS_ZACO1L);
    b.special(200, 400, -700, -300, SH_ZACO_A, IS_ZACO1R);
    b.cspecial(1000, 600, -900, -500, SH_ZACO_5, IS_ZACO1R);
    b.skillfly_set(-300, -60, 4000, 100);
    b.mapobj(0, -300, 0, 4000, SH_ARCH_0, IS_HARD);
    b.setalvarb(AL_ROTY, DEG45);

    b.label("level3_1.map3_1b.bu_0");
    b.mapobj(0, -1200, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.mapobj(1000, 1200, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.maploop("level3_1.map3_1b.bu_0", 3);
    b.skillfly_set(300, -60, 4000, 100);
    b.mapobj(0, 300, 0, 4000, SH_ARCH_0, IS_HARD);
    b.setalvarb(AL_ROTY, -DEG45);

    b.pathcspecial(500, 0, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    b.mapobj(0, 900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(3000, -900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.pathobj(0, -350, 0, 5000, SH_ROBOT_0, PATH_ID_ROBOT, 6, 4);
    b.pathobj(0, 350, 0, 5000, SH_ROBOT_0, PATH_ID_ROBOT, 6, 4);
    b.mapobj(0, 900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(1000, -900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.skillfly_set(0, -60, 4000, 100);
    b.mapobj(3000, 0, 0, 4000, SH_ARCH_0, IS_HARD);

    b.pathobj(0, 0, -400, -150, SH_FRIENDSHIP_4, PATH_ID_CHASE7_1, 10, 10);
    b.pathcspecial(1000, 0, -400, -150, SH_ZACO_A, PATH_ID_CHASE7_2, 10, 10);

    b.mapobj(0, -400, 0, 5000, SH_BASE_1, IS_BASE_1);
    b.mapobj(0, 400, -50, 5200, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    let skillfly_bonus0_guard_ptr = b.mapcode65816_inline(); 
    b.mapobj(0, -400, -50, 5200, SH_ITEM_7, IS_ITEM7);
    b.setalvarb(AL_SBYTE1, 1);
    b.label("level3_1.map3_1b.skillfly_bonus_0_skip");
    b.mapobj(2000, 400, 0, 5000, SH_BASE_1, IS_BASE_1);
    b.mapobj(0, 900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(1500, -900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(0, 900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(1500, -900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(0, 900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(1500, -900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.pathcspecial(1500, 0, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    b.mapobj(0, 300, 0, 4000, SH_BU_8, IS_HARD180YR);
    b.mapobj(1500, -300, 0, 4000, SH_BU_8, IS_HARD180YR);
    b.mapobj(0, 1000, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(1000, -1000, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.special(0, -200, -1300, 2300, SH_ZACO_A, IS_ZACOS);
    b.cspecial(1500, 200, -1300, 2300, SH_ZACO_6, IS_ZACOS);
    b.mapobj(0, 300, 0, 4000, SH_BU_8, IS_HARD180YR);
    b.mapobj(1500, -300, 0, 4000, SH_BU_8, IS_HARD180YR);
    b.pathcspecial(3000, 0, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    b.mapobj(0, 1000, 0, 4000, SH_TOWER_2, IS_TOWER0);
    b.mapobj(1200, -1000, 0, 4000, SH_TOWER_2, IS_TOWER0);

    b.cspecial(300, -600, -300, 5000, SH_KAMIKAZE, IS_ZACO4);
    b.mapobj(0, 300, 0, 4000, SH_BU_8, IS_HARD180YR);
    b.mapobj(0, -300, 0, 4000, SH_BU_8, IS_HARD180YR);
    b.cspecial(1000, 700, -300, 5000, SH_KAMIKAZE, IS_ZACO4);
    b.mapobj(500, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(500, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, -600, 0, 4000, SH_BU_0, IS_HARD180YR);
    b.mapobj(0, 600, 0, 4000, SH_BU_0, IS_HARD180YR);
    b.mapobj(500, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, -600, 0, 4000, SH_BU_0, IS_HARD180YR);
    b.mapobj(0, 600, 0, 4000, SH_BU_0, IS_HARD180YR);
    b.mapobj(500, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, -150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(500, 150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, -300, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(3000, 300, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.cspecial(0, 200, -1300, 2000, SH_ZACO_6, IS_ZACOS);
    b.cspecial(0, -100, -1500, 2300, SH_ZACO_6, IS_ZACOS);
    b.cspecial(0, 500, -1500, 2300, SH_ZACO_6, IS_ZACOS);
    b.special(0, 50, -1700, 2500, SH_ZACO_A, IS_ZACOS);
    b.special(1000, 350, -1700, 2500, SH_ZACO_A, IS_ZACOS);
    b.label("level3_1.map3_1b.tow");
    b.pathobj(0, 800, 0, 4000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    b.pathobj(3000, -800, 0, 4000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    b.maploop("level3_1.map3_1b.tow", 2);

    b.mapobj(0, 380, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, -380, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, 180, 0, 5000, SH_BU_7, IS_HARD180YR);
    b.mapobj(2000, -180, 0, 5000, SH_BU_7, IS_HARD180YR);
    b.mapobj(0, 0, 0, 4000, SH_BU_7, IS_HARD180YR);
    b.mapobj(0, 480, 0, 4000, SH_BU_6, IS_HARD180YR);
    b.mapobj(1000, -480, 0, 4000, SH_BU_6, IS_HARD180YR);
    b.mapobj(0, 230, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, 230, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    b.cspecial(1000, -1500, -600, 4400, SH_ZACO_5, IS_ZACO0);
    b.mapobj(0, 400, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(1000, -400, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(0, 280, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(1000, -280, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.pathobj(0, -130, -150, -100, SH_FRIENDSHIP_4, PATH_ID_FALCON3_1, 10, 10);
    b.mapobj(0, 0, 0, 4000, SH_BU_7, IS_HARD180YR);
    b.mapobj(0, 400, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(1000, -400, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(0, 280, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(1000, -280, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(1000, 0, 0, 4000, SH_BU_7, IS_HARD180YR);
    b.mapobj(0, 340, 0, 4000, SH_BU_6, IS_HARD180YR);
    b.mapobj(2000, -340, 0, 4000, SH_BU_6, IS_HARD180YR);
    b.label("level3_1.map3_1b.torii");
    b.mapobj(300, 0, 0, 3000, SH_ARCH_0, IS_HARD);
    b.maploop("level3_1.map3_1b.torii", 5);
    b.mapobj(0, 0, -30, 2500, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.mapobj(400, 0, 0, 3000, SH_ARCH_0, IS_HARD);
    b.mapobj(0, 120, 0, 3000, SH_ARCH_0, IS_HARD);
    b.mapobj(300, -120, 0, 3000, SH_ARCH_0, IS_HARD);
    b.mapobj(0, 170, -30, 2800, SH_ITEM_5, IS_ITEM5);
    b.mapobj(0, 150, 0, 3000, SH_ARCH_0, IS_HARD);
    b.mapobj(300, -150, 0, 3000, SH_ARCH_0, IS_HARD);
    b.mapobj(0, 170, 0, 3000, SH_ARCH_0, IS_HARD);
    b.mapobj(300, -170, 0, 3000, SH_ARCH_0, IS_HARD);
    b.mapobj(0, 200, 0, 3000, SH_ARCH_0, IS_HARD);
    b.mapobj(100, -200, 0, 3000, SH_ARCH_0, IS_HARD);
    b.mapobj(0, -200, -100, 3000, SH_GATE_0, IS_GATE);
    b.pathobj(1000, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);
    b.pathobj(0, -300, 0, 5000, SH_NULLSHAPE, PATH_ID_ROBOTWITHLOG, 6, 4);
    b.setalxvarw(ALX_PWORD1, SH_RAW_BOSS_7_0);
    b.setalvarb(AL_ROTY, -DEG22);
    b.pathobj(3500, -200, 0, 5000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);

    b.pathobj(0, -1000, 0, 5000, SH_NULLSHAPE, PATH_ID_ROBOTWITHLOG, 6, 4);
    b.setalxvarw(ALX_PWORD1, SH_RAW_BOSS_7_3);
    b.setalvarb(AL_ROTY, -(DEG45 + DEG22));
    b.mapwait(1500);

    b.pathobj(0, -1400, 0, 5000, SH_NULLSHAPE, PATH_ID_ROBOTSWITHLOG, 6, 4);
    b.setalxvarw(ALX_PWORD1, SH_RAW_BOSS_7_3);
    b.setalvarb(AL_ROTY, -(DEG45 + DEG22));
    b.mapwait(3000);

    b.mapobj(1500, 0, 0, 4000, SH_BIG_GATE, IS_HARD);
    b.mapobj(0, 360, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(0, -360, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.label("level3_1.map3_1b.bupillar");
    b.mapobj(0, 180, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, -180, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(1000, 180, 0, 4400, SH_PILLAR3, IS_PILLAR3);
    b.maploop("level3_1.map3_1b.bupillar", 2);
    b.mapobj(0, 0, -50, 4000, SH_R_BU_1, IS_HARD180YR);
    b.mapobj(0, 180, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, -180, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(1000, -180, 0, 4400, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(0, 180, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, -180, 0, 4000, SH_BU_1, IS_HARD180YR);
    b.mapobj(0, 180, 0, 4400, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(1000, -180, 0, 4400, SH_PILLAR3, IS_PILLAR3);
    b.mapobj(1800, 0, 0, 5000, SH_BIG_GATE, IS_HARD);
    b.pathobj(0, -1400, 0, 5000, SH_NULLSHAPE, PATH_ID_ROBOTWITHLOG, 6, 4);
    b.setalxvarw(ALX_PWORD1, SH_RAW_BOSS_7_1);
    b.setalvarb(AL_ROTY, -(DEG45 + DEG22));

    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_BOSS1);
    b.mapobj(0, 3000, 0, 375 << BOSSA_SCALE, SH_BOSS_A_2, IS_BOSSA);
    let mapwaitboss_trigse_ptr = b.mapcode65816_inline(); 
    b.label("level3_1.map3_1b.bosswait.loop");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, "level3_1.map3_1b.bosswait.cont");
    b.mapgoto("level3_1.map3_1b.bosswait.loop");
    b.label("level3_1.map3_1b.bosswait.cont");
    let mapwaitboss_cantdie_ptr = b.mapcode65816_inline(); 
    let mapwaitboss_cleanup_ptr = b.mapcode65816_inline(); 
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);
    b.maprts();

    append_cl_chase_submap(&mut b);
    append_map1_1a_submap(&mut b);

    b.resolve();

    // C: bails to s_empty_level if the skillfly skip label is missing.
    assert!(
        b.lookup_label("level3_1.map3_1b.skillfly_bonus_0_skip").is_some(),
        "level3_1 skillfly bonus skip label missing"
    );

    let (data, labels) = b.finish();
    // C `register_level3_1_inline_callbacks()` registration-call order.
    finish_level(
        data,
        labels,
        vec![
            (keep_player_strat_ptr, "level_scramble_keep_player_strat"),
            (skillfly_bonus0_guard_ptr, "level3_1_skillfly_bonus0_guard"),
            (mapwaitboss_trigse_ptr, "level1_1_mapwaitboss_trigse"),
            (mapwaitboss_cantdie_ptr, "level1_1_mapwaitboss_cantdie"),
            (mapwaitboss_cleanup_ptr, "level1_1_mapwaitboss_cleanup"),
        ],
    )
}
