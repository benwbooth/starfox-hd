//! MAP_ID_3_3 — Fortuna (Level 3, Route 3).
//!
//! C oracle: `src/map/levels.c` `build_level3_3_wrapper_slice()` +
//! `register_level3_3_inline_callbacks()`.
//! ASM: LEVEL3_3.ASM / MAP3_3A.ASM / MAP3_3B.ASM / CL_GND.ASM.

use super::common::*;
use super::finish_level;
use super::Route3Level;
use crate::builder::MapBuilder;

pub(crate) fn build() -> Route3Level {
    let mut b = MapBuilder::new();

    // LEVEL3_3.ASM wrapper: mapjsr map3_3a, 4 flower mapobjs, mapjsr cl_ground, mapend.
    b.mapjsr("level3_3.map3_3a");

    // 4 flower mapobjs after map3_3a returns
    b.mapobj(0, 800, 0, 8000, SH_FLOWER_1, IS_HARD180YR);
    b.mapobj(0, -1000, 0, 10000, SH_FLOWER_1, IS_HARD180YR);
    b.mapobj(0, 1000, 0, 12000, SH_FLOWER_1, IS_HARD180YR);
    b.mapobj(0, 1200, 0, 12000, SH_FLOWER_1, IS_HARD180YR);

    b.mapjsr("cl_ground");
    b.mapend(1);

    // MAP3_3A.ASM — Fortuna Part A subroutine.
    b.label("level3_3.map3_3a");
    b.mapwait(2500);

    // Lines 6-8: three e_flower path objects
    b.pathobj(1000, 0, 0, 2500, SH_NULLSHAPE, PATH_ID_E_FLOWER, 10, 8);
    b.pathobj(1000, -200, 0, 2500, SH_NULLSHAPE, PATH_ID_E_FLOWER, 10, 8);
    b.pathobj(1000, 200, 0, 2500, SH_NULLSHAPE, PATH_ID_E_FLOWER, 10, 8);

    // Lines 10-12: three tree1 objects
    b.mapobj(1000, -200, 0, 2500, SH_STALK, IS_TREE1);
    b.mapobj(1000, 200, 0, 2500, SH_STALK, IS_TREE1);
    b.mapobj(1000, 0, 0, 2500, SH_STALK, IS_TREE1);

    // Lines 14-16: flower mapobjs
    b.mapobj(0x0400, -300, 0, 4000, SH_FLOWER_2, IS_HARD180YR);
    b.mapobj(0x0900, 100, 0, 4000, SH_FLOWER_1, IS_HARD180YR);
    b.mapobj(1000, -800, 0, 4000, SH_FLOWER_1, IS_HARD180YR);

    // Lines 17-18: bee pathcspecials
    b.pathcspecial(0x0800, 300, -150, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);
    b.pathcspecial(0x0400, -400, -170, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);

    // Lines 19-20: more flowers
    b.mapobj(1400, -1000, 0, 4000, SH_FLOWER_1, IS_HARD180YR);
    b.mapobj(2400, -800, 0, 4000, SH_FLOWER_1, IS_HARD180YR);

    // Lines 21-22: friend chase6 pair
    b.pathobj(0, -750, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    b.pathcspecial(0, -720, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);

    // Lines 23-27: tomset paths, flowers, bees
    b.pathobj(3000, 400, -40, 4000, SH_STALK, PATH_ID_TOMSET, 10, 10);
    b.mapobj(2000, 100, 0, 4000, SH_FLOWER_2, IS_HARD180YR);
    b.pathobj(2000, -300, -40, 4000, SH_STALK, PATH_ID_TOMSET, 10, 10);
    b.pathcspecial(0x0600, 100, -120, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);
    b.pathcspecial(1400, -100, -120, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);

    // Lines 28-31: more flowers
    b.mapobj(500, 200, 0, 4000, SH_FLOWER_2, IS_HARD180YR);
    b.mapobj(500, -200, 0, 4000, SH_FLOWER_2, IS_HARD180YR);
    b.mapobj(500, -900, 0, 4000, SH_FLOWER_2, IS_HARD180YR);
    b.mapobj(1000, 900, 0, 4000, SH_FLOWER_2, IS_HARD180YR);

    // Line 32: tomhaha path
    b.pathobj(3000, 0, -1000, 4000, SH_NULLSHAPE, PATH_ID_TOMHAHA, 10, 10);

    // Lines 33-36: flowers
    b.mapobj(1000, 400, 0, 4000, SH_FLOWER_1, IS_HARD180YR);
    b.mapobj(1000, -500, 0, 4000, SH_FLOWER_1, IS_HARD180YR);
    b.mapobj(1000, 100, 0, 4000, SH_FLOWER_2, IS_HARD180YR);
    b.mapobj(2000, -200, 0, 4000, SH_FLOWER_2, IS_HARD180YR);

    // Lines 38-39: trees
    b.mapobj(300, 300, 0, 1500, SH_STALK, IS_TREE1);
    b.mapobj(0, 0, 0, 1500, SH_STALK, IS_TREE1);

    // Line 40: cspecial bom_wing
    b.cspecial(0, 0, 0, 4000, SH_BOM_WING, IS_BOMWING);

    // Lines 41-43: ponpon + bees
    b.pathspecial(500, -400, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);
    b.pathcspecial(400, 100, -120, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);
    b.pathcspecial(400, -100, -100, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);

    // Lines 44-45: ponpon + bee
    b.pathspecial(1000, 400, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);
    b.pathcspecial(400, 100, -120, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);

    // Lines 46-52: trees (tree1 and tree2)
    b.mapobj(3500, 200, 0, 1500, SH_STALK, IS_TREE1);
    b.mapobj(900, -200, 0, 1300, SH_STALK, IS_TREE1);
    b.mapobj(900, 0, 0, 1300, SH_STALK, IS_TREE1);
    b.mapobj(900, 200, 0, 1300, SH_STALK, IS_TREE1);
    b.mapobj(1200, -300, 0, 1300, SH_STALK, IS_TREE1);
    b.mapobj(500, 300, 0, 1800, SH_STALK, IS_TREE2);
    b.mapobj(2500, 0, 0, 1800, SH_STALK, IS_TREE2);

    // Line 54: gate
    b.mapobj(1000, 0, -100, 2000, SH_GATE_0, IS_GATE);

    // Line 56: e_gate path
    b.pathobj(0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 59-60: .pdead2 — dead-check loop
    b.label("level3_3.pdead2");
    b.mapwait(1000);
    let pdead2_capture = b.mapcode65816_inline();

    // Line 61: mapfadetosea
    b.mapfadetosea();

    // Lines 62-64: transition to water phase
    b.mapwait(600);
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_ONWATER_L);
    b.mapwait(1000);

    // Lines 66-68: three flyfish pathcspecials
    b.pathcspecial(
        1000,
        200,
        0,
        4000,
        SH_F_FISH_PROXY,
        PATH_ID_E_FLYFISH,
        10,
        10,
    );
    b.pathcspecial(5000, 0, 0, 4000, SH_F_FISH_PROXY, PATH_ID_E_FLYFISH, 10, 10);
    b.pathcspecial(
        1000,
        -200,
        0,
        4000,
        SH_F_FISH_PROXY,
        PATH_ID_E_FLYFISH,
        10,
        10,
    );

    // Lines 70-72: torpedo spawners
    b.mapobj(500, 0, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(500, -300, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(2000, 300, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);

    // Lines 74-79: kamome + torpedoes
    b.pathcspecial(1000, 1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    b.mapobj(0, 0, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);
    b.pathcspecial(1000, -1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    b.mapobj(0, -300, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);
    b.pathcspecial(2000, 1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    b.mapobj(2000, 300, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);

    // Lines 81-82: seadragon friend chase7 pair
    b.pathobj(0, 0, -400, -150, SH_FRIENDSHIP_4, PATH_ID_CHASE7_1, 10, 10);
    b.pathcspecial(4000, 0, -400, -150, SH_ZACO_A, PATH_ID_CHASE7_2, 10, 10);

    // Lines 83-85: kamome trio
    b.pathcspecial(1000, -1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    b.pathcspecial(1000, 1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    b.pathcspecial(2000, -1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);

    // Line 86: nessie 6000,-400,0000,5000,deg180,40
    b.nessie(6000, -400, 0, 5000, 128, 40);

    // Lines 87-88: item_6 + setalvar sbyte1
    b.mapobj(0, 100, -50, 4700, SH_ITEM_6, IS_ITEM6);
    b.setalvarb(AL_SBYTE1, 1);

    // Line 89: nessie 3000,-400,0000,5000,deg45,40
    b.nessie(3000, -400, 0, 5000, 32, 40);

    // Line 90: mapmother — mother_snakes pattern (children spawn inert
    // until seadragon_istrat is ported; STRATEGY_SEADRAGON is reserved).
    b.mapmother(
        4000,
        0,
        0,
        3000,
        SH_MOTHER1,
        STRATEGY_MOTHER2,
        crate::mothers::mother_maps().mother_snakes,
    );

    // Lines 91-92: seadragon2 snakes
    b.mapobj(3000, 300, 0, 4000, SH_SNAKE_1, IS_SEADRAGON2);
    b.mapobj(2500, -400, 0, 4000, SH_SNAKE_1, IS_SEADRAGON2);

    // Line 93: nessie 3000,-200,0000,5000,deg22,10
    b.nessie(3000, -200, 0, 5000, 16, 10);

    // Line 94: maprem mother1
    b.mapremove(SH_MOTHER1);

    // Lines 95-98: snakes, up1man, more snakes + nessie
    b.mapobj(3000, 150, 0, 4000, SH_SNAKE_1, IS_SEADRAGON2);
    b.mapobj(2500, 0, -140, 4000, SH_NULLSHAPE, IS_UP1MAN);
    b.mapobj(2000, 0, 0, 4000, SH_SNAKE_1, IS_SEADRAGON2);
    b.nessie(2000, -200, 0, 5000, 32, 60);

    // Lines 99-103: kamome pair + friend chase6 + kamome
    b.pathcspecial(1500, -1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    b.pathcspecial(1500, 1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    b.pathobj(0, -750, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    b.pathcspecial(0, -720, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);
    b.pathcspecial(2500, -1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);

    // Lines 105-107: three flyfish
    b.pathcspecial(
        1000,
        200,
        0,
        4000,
        SH_F_FISH_PROXY,
        PATH_ID_E_FLYFISH,
        10,
        10,
    );
    b.pathcspecial(1000, 0, 0, 4000, SH_F_FISH_PROXY, PATH_ID_E_FLYFISH, 10, 10);
    b.pathcspecial(
        3000,
        -200,
        0,
        4000,
        SH_F_FISH_PROXY,
        PATH_ID_E_FLYFISH,
        10,
        10,
    );

    // Lines 108-110: torpedo spawners
    b.mapobj(1000, 0, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(1000, -300, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(4000, 300, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);

    // Lines 112-113: underwater gate
    b.mapobj(1000, 200, -200, 2000, SH_GATE_0, IS_GATE);
    b.pathobj(0, 3000, -100, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 115-116: .pdead — dead-check loop
    b.label("level3_3.pdead");
    b.mapwait(1000);
    let pdead_capture = b.mapcode65816_inline();

    // Lines 117-120: mapfadetoground + onplanet transition
    b.mapfadetoground();
    b.mapwait(500);
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_ONPLANET_L);
    b.mapwait(4000);

    // Lines 122-126: second ground phase opening — trees + ponpon
    b.mapobj(500, 150, 0, 1500, SH_STALK, IS_TREE1);
    b.pathspecial(0, -300, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);
    b.mapobj(500, 300, 0, 1500, SH_STALK, IS_TREE1);
    b.mapobj(500, -300, 0, 1500, SH_STALK, IS_TREE1);
    b.pathspecial(0, 300, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);

    // Lines 128-129: skillfly init + set
    b.skillfly_init();
    b.skillfly_set_default(-300, -50, 2300);

    // Line 130: roottree 0800,-300,0000,2400,-deg45,30
    b.roottree(0x0800, -300, 0, 2400, -32, 30);

    // Lines 131-132: tree2 objects
    b.mapobj(500, 100, 0, 2500, SH_STALK, IS_TREE2);
    b.mapobj(500, -200, 0, 1800, SH_STALK, IS_TREE2);

    // Line 133: ponpon
    b.pathspecial(0, 200, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);

    // Line 134: tree2
    b.mapobj(500, 0, 0, 2500, SH_STALK, IS_TREE2);

    // Lines 135-136: skillfly_bonus item_5 + setalvar
    let skillfly_bonus0_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, -250, -80, 1500, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.label("level3_3.skillfly_bonus_0_skip");

    // Lines 137-140: tree2 + ponpon + tree1 + tree2
    b.mapobj(500, 350, 0, 1800, SH_STALK, IS_TREE2);
    b.pathspecial(0, -300, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);
    b.mapobj(500, -150, 0, 1800, SH_STALK, IS_TREE1);
    b.mapobj(500, 300, 0, 1800, SH_STALK, IS_TREE2);

    // Line 141: roottree 1500,-200,0000,2400,deg11,40
    b.roottree(1500, -200, 0, 2400, 8, 40);

    // Lines 142-143: tree2 + roottree
    b.mapobj(500, 300, 0, 1800, SH_STALK, IS_TREE2);
    b.roottree(1500, 0, 0, 2400, 16, 10);

    // Lines 144-146: tree2 + item_7 + setalvar
    b.mapobj(500, -100, 0, 1800, SH_STALK, IS_TREE2);
    b.mapobj(0, 200, -50, 2200, SH_ITEM_7, IS_ITEM7);
    b.setalvarb(AL_SBYTE1, 1);

    // Line 147: roottree 1500,0200,0000,2400,-deg45,30
    b.roottree(1500, 200, 0, 2400, -32, 30);

    // Lines 148-149: ponpon + tree2
    b.pathspecial(0, 0, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);
    b.mapobj(500, -100, 0, 1800, SH_STALK, IS_TREE2);

    // Lines 150-151: roottree + tree2
    b.roottree(1500, -200, 0, 3000, 8, 10);
    b.mapobj(500, 300, 0, 1800, SH_STALK, IS_TREE2);

    // Lines 152-155: roottree + tree1 + tree2 + roottree pair
    b.roottree(1500, -200, 0, 3000, 8, 10);
    b.mapobj(500, 100, 0, 1500, SH_STALK, IS_TREE1);
    b.mapobj(500, -100, 0, 2200, SH_STALK, IS_TREE2);
    b.roottree(0, 0, 0, 3000, 8, 10);

    // Lines 156-158: roottree + tree2 pair
    b.roottree(1500, 200, 0, 3000, 8, 40);
    b.mapobj(500, -200, 0, 1800, SH_STALK, IS_TREE2);
    b.mapobj(3500, 300, 0, 1800, SH_STALK, IS_TREE2);

    // Line 160: dragonmsg path
    b.pathobj(0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_DRAGONMSG, 10, 10);

    // Lines 165-168: boss section — fadeoutbgm, setbgm boss, chicken spawn
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_BOSS1);
    b.mapobj(0, -100, 0, 4000, SH_BOSS_D_1, IS_CHICKEN);
    b.setalvarb(AL_ROTY, 128); // deg180

    // Lines 169-170: mapwaitboss + markboss boss33
    b.mapwait(100);
    let mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("level3_3.bosswait.loop");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, "level3_3.bosswait.cont");
    b.mapgoto("level3_3.bosswait.loop");
    b.label("level3_3.bosswait.cont");
    let mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let mapwaitboss_cleanup_ptr = b.mapcode65816_inline();

    // markboss boss33
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);

    // Line 172: mapwait 1000
    b.mapwait(1000);

    // Line 174: maprts
    b.maprts();

    // ================================================================
    // MAP3_3B.ASM — Fortuna Part B (boss sea-monster torpedo gauntlet)
    // Standalone callable subroutine (INCMAP'd in MAPLIST.ASM).
    // ================================================================
    b.label("level3_3.map3_3b");

    // Lines 4-17: torpedo spawners (alternating left/right)
    b.mapobj(3000, -400, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(3000, 400, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(1000, -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(1000, 200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(500, -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(500, 200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(300, -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(300, 200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(200, -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(200, 200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(200, -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(200, 200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(200, -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(3000, 200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);

    // Line 19: sea monster
    b.mapobj(3000, 0, 0, 400, SH_SEA_0_0, IS_SEAMON);

    // Lines 21-27: sea monster V-formation (z = 3000-2000 .. 3300-2000)
    b.mapobj(50, 0, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(0, -100, 0, 1100, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(50, 100, 0, 1100, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(0, -200, 0, 1200, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(50, 200, 0, 1200, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(0, -300, 0, 1300, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(8000, 300, 0, 1300, SH_SEA_0_0, IS_SEAMON);

    // Lines 29-33: more torpedo spawners
    b.mapobj(500, -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(500, 200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(500, -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(500, 200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);

    // Lines 36-42: descending sea monster arc (left to right)
    b.mapobj(300, -300, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(250, -200, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(200, -100, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(150, 0, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(100, 100, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(50, 200, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(8000, 300, 0, 1000, SH_SEA_0_0, IS_SEAMON);

    // Lines 44-50: ascending sea monster arc (right to left)
    b.mapobj(300, 300, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(250, 200, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(200, 100, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(150, 0, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(100, -100, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(50, -200, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    b.mapobj(8000, -300, 0, 1000, SH_SEA_0_0, IS_SEAMON);

    // Line 52: maprts
    b.maprts();

    // CL_GND.ASM — clear demo (ground type) appended as subroutine.
    append_cl_ground_submap(&mut b);

    b.resolve();

    // C zeroes these when missing; all three labels are emitted above.
    assert!(
        b.lookup_label("level3_3.skillfly_bonus_0_skip").is_some(),
        "level3_3 skillfly bonus skip label missing"
    );
    let (data, labels) = b.finish();
    // C `register_level3_3_inline_callbacks()` registration-call order.
    finish_level(
        data,
        labels,
        vec![
            (skillfly_bonus0_guard_ptr, "level3_3_skillfly_bonus0_guard"),
            // Inline callbacks are keyed by the byte immediately after the
            // CODE65816 opcode, not by the loop label before its mapwait.
            (pdead2_capture, "level3_3_pdead2_check"),
            (pdead_capture, "level3_3_pdead_check"),
            (mapwaitboss_trigse_ptr, "level1_1_mapwaitboss_trigse"),
            (mapwaitboss_cantdie_ptr, "level1_1_mapwaitboss_cantdie"),
            (mapwaitboss_cleanup_ptr, "level1_1_mapwaitboss_cleanup"),
        ],
    )
}
