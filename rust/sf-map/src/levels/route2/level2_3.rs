//! MAP_ID_2_3 — Titania (Level 2, Route 2).
//!
//! C oracle: `src/map/levels.c` `build_level2_3a_slice()` +
//! `register_level2_3_inline_callbacks()`.
//! ASM sources: MAP2_3A.ASM, MAP2_3C.ASM (boss room), MAP2_3B.ASM (boss
//! section, inlined), LEVEL2_3.ASM transitions, CL_BRIDG.ASM.
//!
//! Runtime-only C side effects NOT mirrored here (they touch game state,
//! not the bytecode blob): `RAM16(WM_PLAYERPOSX) = 0` before the build and
//! the `g_player_posx` sync / `g_ebyte3` / `g_maptrigger` resets performed
//! by `register_level2_3_inline_callbacks()`.

use super::rc::*;
use super::submaps;
use super::Route2Level;
use crate::builder::MapBuilder;

/// C `build_level2_3a_slice()`.
pub fn build() -> Route2Level {
    let mut b = MapBuilder::new();

    // BGS.ASM `bg_2_3a_1`: `pstrat playeronplanet,a,ab`. The old literal
    // builder started at MAP2_3A and silently omitted this init-bg side
    // effect, leaving a freshly spawned player without the planet Y bounds.
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_ONPLANET_L);

    // 2-3-1
    b.setvarb(WM_INFOG, 1);
    b.mapwait(2000);
    // -----------------------------------------------------------------------
    b.pathobj(0, 0x0400, -120, 2500, SH_R_BUT_2, PATH_ID_PINITA_B, 10, 10);
    b.pathobj(0, -0x0400, -120, 2500, SH_R_BUT_2, PATH_ID_PINITA_B, 10, 10);
    b.pathobj(2000, 0, -120, 2500, SH_R_BUT_2, PATH_ID_PINITA_A, 10, 10);
    b.mapobj(0, -0x0600, 0, 2000, SH_BRO_4, IS_ROCKHARD);
    b.mapobj(0x0500, 0x0600, 0, 2000, SH_BRO_5, IS_ROCKHARD);
    b.maphardrot(0, -150, -75, 2000, SH_CLISLA_M, 0, 8, 0);
    b.pathobj(
        0x0500,
        0x0050,
        -75,
        2000,
        SH_CLISLA_S,
        PATH_ID_L_CLISLA,
        10,
        10,
    );
    b.mapobj(0, -0x0550, 0, 2000, SH_BRO_0, IS_ROCKHARD);
    b.mapobj(0x1000, 0x0350, 0, 2000, SH_BRO_5, IS_ROCKHARD);
    b.mapobj(0x0500, -200, 0, 2000, SH_HOU_5, IS_HOUDAI5F);

    b.mapobj(0, -0x0700, 0, 2000, SH_BRO_0, IS_ROCKHARD);
    b.mapobj(0, 0x0150, 0, 2000, SH_BRO_5, IS_ROCKHARD);
    b.pathcspecial(
        0x1000,
        0x0150,
        0,
        2600,
        SH_WALKER_0,
        PATH_ID_E_WALK_1,
        10,
        10,
    );
    b.mapobj(0, -0x0600, 0, 2000, SH_BRO_2, IS_ROCKHARD);
    b.mapobj(0x1000, 0x0500, 0, 2000, SH_BRO_1, IS_ROCKHARD);
    b.mapobj(0, -0x0400, 0, 2000, SH_BRO_4, IS_ROCKHARD);
    b.mapobj(0x1000, 0x0550, 0, 2000, SH_BRO_1, IS_ROCKHARD);

    b.mapobj(0, -0x0650, 0, 2000, SH_BRO_0, IS_ROCKHARD);
    b.mapobj(0, 0x0650, 0, 2000, SH_BRO_1, IS_ROCKHARD);
    b.mapobj(0, 0, 0, 2000, SH_BRO_6, IS_ROCKHARD);
    b.mapobj(0, -160, -190, 2500, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.mapwait(200);
    b.pathspecial(0x1000, 0, 0, 2350, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);
    b.maphardrot(0, 0, -75, 2000, SH_CLISLA_M, 0, -8, 0);
    b.pathobj(0, -200, -75, 2000, SH_CLISLA_S, PATH_ID_R_CLISLA, 10, 10);
    b.mapobj(0, -0x0500, 0, 2000, SH_BRO_2, IS_ROCKHARD);
    b.mapobj(0x1000, 0x0500, 0, 2000, SH_BRO_3, IS_ROCKHARD);
    b.mapobj(0, -0x0500, 0, 2000, SH_BRO_4, IS_ROCKHARD);
    b.mapobj(0, 0x0500, 0, 2000, SH_BRO_5, IS_ROCKHARD);
    b.mapobj(0x1000, 0, 0, 2000, SH_HOU_5, IS_HOUDAI5F);
    b.mapobj(0, 250, 0, 2000, SH_BRO_6, IS_HARD180YR);
    b.mapobj(0x0500, -250, 0, 2000, SH_BRO_6, IS_HARD180YR);
    b.pathobj(0x1500, 0, -120, 2500, SH_R_BUT_2, PATH_ID_PINITA_B, 10, 10);

    // 2-3-2
    b.mapobj(0, -300, 0, 2000, SH_BRO_6, IS_HARD180YR);
    b.pathspecial(0x1000, -300, 0, 2500, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);
    b.mapobj(0, 0x0600, 0, 2000, SH_BRO_6, IS_HARD180YR);
    b.pathspecial(
        0x1000,
        0x0600,
        0,
        2500,
        SH_WALKER_0,
        PATH_ID_E_WALK_1,
        10,
        10,
    );

    b.pathobj(0, -750, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    b.pathcspecial(0, -720, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);

    // misstank
    b.cspecial(0, -1000, 0, 3000, SH_M_TANK, IS_MISSTANK);
    b.setalvarb(AL_ROTY, -64); // -deg90
    b.addalvarptrw(AL_WORLDX, WM_PLAYERPOSX);

    b.pathobj(
        0x0700,
        300,
        -200,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(
        0x0700,
        -200,
        -45,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(
        0x0700,
        0x0100,
        -30,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(0, 250, -120, 2500, SH_R_BUT_2, PATH_ID_PINITA_B, 10, 10);
    b.pathobj(
        0x0700,
        -400,
        -100,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );

    b.cspecial(0, 1000, 0, 3000, SH_M_TANK, IS_MISSTANK);
    b.setalvarb(AL_ROTY, 64); // deg90
    b.addalvarptrw(AL_WORLDX, WM_PLAYERPOSX);

    b.pathobj(
        0x0700,
        -300,
        -200,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(
        0x0700,
        400,
        -100,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(
        0x0700,
        -100,
        -30,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(
        0x0700,
        200,
        -45,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );

    b.cspecial(0, -1000, 0, 3000, SH_M_TANK, IS_MISSTANK);
    b.setalvarb(AL_ROTY, -64); // -deg90
    b.addalvarptrw(AL_WORLDX, WM_PLAYERPOSX);

    b.pathobj(0, 0x0100, -120, 2500, SH_R_BUT_2, PATH_ID_PINITA_B, 10, 10);

    // .fogagain
    b.label("level2_3.fogagain");
    b.pathobj(
        0x0700,
        -300,
        -200,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(
        0x0700,
        400,
        -100,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(
        0x0700,
        -100,
        -30,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(3000, 200, -45, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    b.pathobj(0x1000, 0, -170, 3000, SH_WALK_4_0, PATH_ID_E_KANI_0, 10, 10);

    b.pathobj(
        0x0700,
        -300,
        -200,
        3000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(
        0x0700,
        400,
        -100,
        3000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(
        0x0700,
        -100,
        -30,
        3000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(
        0x1000,
        200,
        -45,
        3000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );

    // base
    b.mapobj(0, 0x0350, 0, 3000, SH_HOU_5, IS_HOUDAI5F);
    b.mapobj(0x1400, -350, 0, 3000, SH_HOU_5, IS_HOUDAI5F);
    b.mapobj(0, 0, -50, 4200, SH_ITEM_7, IS_ITEM7);
    b.setalvarb(AL_SBYTE1, 1);
    b.mapobj(0x0500, 0, 0, 4000, SH_BASE_0, IS_BASE1);

    b.pathspecial(0, 0x0500, 0, 4400, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    b.mapobj(0, 0x0500, 0, 4000, SH_BASE_0, IS_BASE1);
    // skillfly_init / skillfly_set are commented out in the ASM
    // ASM bare numerals are decimal: `0500`, not hexadecimal `$0500`.
    b.pathobj(0, 500, -100, 4030, SH_CORE_1_1, PATH_ID_TENKI_ON, 10, 10);
    b.pathobj(500, 500, 0, 4030, SH_RADER_1, PATH_ID_TENKI_DM, 10, 10);

    b.pathspecial(0, -0x0500, 0, 4400, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    b.pathobj(0, 0, -120, 4000, SH_R_BUT_2, PATH_ID_PINITA_B, 10, 10);
    b.mapobj(3500, -0x0500, 0, 4000, SH_BASE_0, IS_BASE1);

    // eguchi2fly_goto .fogout
    let level2_3_fog_guard_ptr = b.mapcode65816_inline();
    b.mapgoto("level2_3.fogout");
    b.label("level2_3.fog_guard_continue");
    b.mapwait(500);
    b.mapgoto("level2_3.fogagain");

    // .fogout
    b.label("level2_3.fogout");

    // Post-fog transition: SETVAR.N FADEPAL,33 / setvar palfrom..pallen / INFOG=0 /
    // MAPCODE_JSL BG_1_4B_1 / start_65816 dotsflag+planetstars end_65816
    b.setvarb(WM_FADEPAL, 33);
    b.setvarb(WM_PALFROM, 0);
    b.setvarb(WM_PALTO, 0);
    b.setvarb(WM_PALLEN, 32);
    b.setvarb(WM_INFOG, 0);
    b.mapcodejsl_builtin(MAP_CB_BG_1_4B_1_L);
    let level2_3_setvar_inline_ptr = b.mapcode65816_inline();

    b.pathspecial(0, -2100, -200, 3500, SH_ZACO_A, PATH_ID_EGU4, 10, 10);
    b.pathcspecial(4000, -2300, -100, 2500, SH_ZACO_5, PATH_ID_EGU4, 10, 10);

    b.pathcspecial(0, -150, 0, 5000, SH_HELI, PATH_ID_E_HELI, 10, 10);
    b.pathcspecial(4200, 150, 0, 5800, SH_HELI, PATH_ID_E_HELI, 10, 10);

    b.pathobj(0, 0, -400, -150, SH_FRIENDSHIP_4, PATH_ID_CHASE7_1, 10, 10);
    b.pathcspecial(0x1600, 0, -400, -150, SH_ZACO_A, PATH_ID_CHASE7_2, 10, 10);
    b.pathobj(0, 260, -120, 3000, SH_R_BUT_2, PATH_ID_PINITA_B, 10, 10);
    b.pathobj(3400, -260, -120, 3000, SH_R_BUT_2, PATH_ID_PINITA_B, 10, 10);
    b.pathobj(
        0x0500,
        200,
        -120,
        3000,
        SH_R_BUT_2,
        PATH_ID_PINITA_B,
        10,
        10,
    );
    b.mapobj(0, -200, -150, 3200, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.pathobj(2100, -200, -120, 3000, SH_R_BUT_2, PATH_ID_PINITA_A, 10, 10);

    b.maphardrot(0, 300, -75, 4000, SH_CLISLA_M, 0, -8, 0);
    b.pathobj(
        3000,
        0x0500,
        -75,
        4000,
        SH_CLISLA_S,
        PATH_ID_L_CLISLA,
        10,
        10,
    );
    b.pathobj(
        0x1500,
        200,
        -170,
        4500,
        SH_WALK_4_0,
        PATH_ID_E_KANI_0,
        10,
        10,
    );
    b.maphardrot(0, -300, -75, 4000, SH_CLISLA_M, 0, -8, 0);
    b.pathobj(
        4000,
        -0x0500,
        -75,
        4000,
        SH_CLISLA_S,
        PATH_ID_R_CLISLA,
        10,
        10,
    );
    b.pathobj(
        0x1000,
        -200,
        -170,
        4500,
        SH_WALK_4_0,
        PATH_ID_E_KANI_0,
        10,
        10,
    );
    b.pathobj(
        0x0400,
        -300,
        -200,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(
        0x0400,
        400,
        -100,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );

    b.pathobj(
        0x0400,
        -100,
        -30,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );
    b.pathobj(
        0x0400,
        200,
        -45,
        2000,
        SH_CLISLA_S,
        PATH_ID_MINI_CLI,
        10,
        10,
    );

    b.mapobj(0x1000, -600, 0, 5000, SH_CLISLA_L, IS_HARD180YR);

    // skillfly_init + skillfly_set
    b.skillfly_init();
    b.skillfly_set_default(0, -150, 3000);
    b.pathobj(0x1000, 0, -120, 3000, SH_R_BUT_2, PATH_ID_PINITA_A, 10, 10);
    b.pathobj(
        0x1000,
        260,
        -120,
        3000,
        SH_R_BUT_2,
        PATH_ID_PINITA_B,
        10,
        10,
    );
    b.skillfly_set_default(-300, -120, 3000);
    b.pathobj(
        0x1500,
        -300,
        -120,
        3000,
        SH_R_BUT_2,
        PATH_ID_PINITA_B,
        10,
        10,
    );
    b.pathobj(
        0x1500,
        0x0100,
        -120,
        3000,
        SH_R_BUT_2,
        PATH_ID_PINITA_A,
        10,
        10,
    );

    // skillfly_bonus
    let level2_3_skillfly_bonus_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, 0x0100, -120, 1700, SH_GATE_0, IS_GATE);
    b.label("level2_3.skillfly_bonus_0_skip");

    // misstank pair
    b.cspecial(0, 1000, 0, 3000, SH_M_TANK, IS_MISSTANK);
    b.setalvarb(AL_ROTY, 64); // deg90
    b.addalvarptrw(AL_WORLDX, WM_PLAYERPOSX);
    b.cspecial(0, -1000, 0, 3000, SH_M_TANK, IS_MISSTANK);
    b.setalvarb(AL_ROTY, -64); // -deg90
    b.addalvarptrw(AL_WORLDX, WM_PLAYERPOSX);

    b.mapwait(2500);

    b.special(0x0400, 0x0550, 0, 4000, SH_S_TANK_0, IS_TANK3);
    b.pathcspecial(0x0400, 0, 0, 4000, SH_TANK_1, PATH_ID_E_TANK, 10, 10);
    b.special(6000, -0x0550, 0, 4000, SH_S_TANK_0, IS_TANK3);
    b.pathobj(0, -750, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    b.pathcspecial(4000, -720, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);
    b.cspecial(0x0500, 300, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    b.cspecial(4500, -300, 0, 4000, SH_HOU_5, IS_HOUDAI5F);

    b.pathobj(18000, 0, -170, 5000, SH_NULLSHAPE, PATH_ID_KANIHAHA, 10, 10);

    b.mapwait(10000);

    // kichi base entrance sequence
    {
        let mut kichi2_pos: i32 = BIGBASEZ + KICHI0_DOOR; // 9360
        b.mapobj(0, 0, 0, kichi2_pos, SH_K_DOOR, IS_KDOOR);
        kichi2_pos += KICHI2_LEN / 2; // 9556
        b.mapobj(0, 0, 0, kichi2_pos, SH_KICHI_3, IS_KICHI2);
        kichi2_pos += KICHI2_LEN; // 9948
        b.mapobj(0, 0, 0, kichi2_pos, SH_KICHI_3, IS_KICHI2);
        kichi2_pos += KICHI2_LEN; // 10340
        b.mapobj(0, 0, 0, kichi2_pos, SH_KICHI_3, IS_KICHI2);
        kichi2_pos += KICHI2_LEN / 2; // 10536
        b.mapobj(0, 0, 0, kichi2_pos, SH_K_DOOR, IS_KDOOR2);
        // kichi_0 (massivebase): placed at kichi2_pos - kichi2_len - kichi2_len/2 - medpspeed*20
        {
            let massive_wait: i32 = kichi2_pos - KICHI2_LEN - KICHI2_LEN / 2 - MEDPSPEED * 20;
            b.mapobj(massive_wait, 0, 0, BIGBASEZ, SH_KICHI_0, IS_MASSIVEBASE);
        }
    }

    b.setbgm(BGM_FADEOUT);
    b.mapwait(MEDPSPEED * 20);

    // LEVEL2_3.ASM transition: setbg 2_3c / initbg / setrestart
    b.setbg(BG_2_3C);
    b.initbg();
    b.mapcodejsl_builtin(MAP_CB_SETRESTART_L);

    // ============================================================
    // MAP2_3C.ASM — Titania Part C (boss room)
    // ============================================================

    // setrestart (MAP2_3C's own restart checkpoint)
    b.mapcodejsl_builtin(MAP_CB_SETRESTART_L);
    // mapobj 0000,0000,-060,3000,boss_g_0,bossg_istrat
    b.mapobj(0, 0, -0x60, 3000, SH_BOSS_G_0, IS_BOSSG);
    // start_65816 / trigse $0b / end_65816
    let level2_3c_trigse_ptr = b.mapcode65816_inline();
    // mapwait 1
    b.mapwait(1);
    // setbgm 5 (BGM_BOSS1)
    b.setbgm(BGM_BOSS1);
    // mapwait 5000
    b.mapwait(5000);
    // incmap airlock1 (airlock_pos = 4000)
    {
        let mut airlock_pos: i32 = 4000;
        // mapobj 0000,0000,0000,airlock_pos,k_door,kdoor_istrat
        b.mapobj(0, 0, 0, airlock_pos, SH_K_DOOR, IS_KDOOR);
        // airlock_pos = airlock_pos + kichi2_len/2
        airlock_pos += KICHI2_LEN / 2;
        // mapobj 0000,0000,0000,airlock_pos,kichi_3,kichi2_istrat
        b.mapobj(0, 0, 0, airlock_pos, SH_KICHI_3, IS_KICHI2);
        // airlock_pos = airlock_pos + kichi2_len
        airlock_pos += KICHI2_LEN;
        // mapobj 0000,0000,0000,airlock_pos,kichi_3,kichi2_istrat
        b.mapobj(0, 0, 0, airlock_pos, SH_KICHI_3, IS_KICHI2);
        // airlock_pos = airlock_pos + kichi2_len
        airlock_pos += KICHI2_LEN;
        // mapobj 0000,0000,0000,airlock_pos,kichi_3,kichi2_istrat
        b.mapobj(0, 0, 0, airlock_pos, SH_KICHI_3, IS_KICHI2);
        // airlock_pos = airlock_pos + kichi2_len
        airlock_pos += KICHI2_LEN;
        // mapobj 0000,0000,0000,airlock_pos,kichi_3,kichi2_istrat
        b.mapobj(0, 0, 0, airlock_pos, SH_KICHI_3, IS_KICHI2);
        // airlock_pos = airlock_pos + kichi2_len/2
        airlock_pos += KICHI2_LEN / 2;
        // mapobj 0000,0000,0000,airlock_pos,k_door,kdoor2_istrat
        b.mapobj(0, 0, 0, airlock_pos, SH_K_DOOR, IS_KDOOR2);
        // mapwait airlock_pos - kichi2_len*2
        b.mapwait(airlock_pos - KICHI2_LEN * 2);
    }
    // maprts — end of MAP2_3C (inlined, so just continue)

    // LEVEL2_3.ASM transition: setbg 2_3b / mapwait kichi2_len*2 / initbg
    b.setbg(BG_2_3B);
    b.mapwait(KICHI2_LEN * 2);
    b.initbg();

    // ============================================================
    // MAP2_3B.ASM — Titania Part B (boss section)
    // ============================================================
    // Inlined here rather than as a subroutine; the original LEVEL2_3.ASM
    // calls map2_3a, map2_3c, then map2_3b via mapjsr.

    b.mapwait(2000);

    // .waitabit
    b.label("level2_3b.waitabit");
    b.mapwait(100);

    // Inline 65816: maptrigger check
    let level2_3b_trigger_ptr = b.mapcode65816_inline();

    // setvar gsvar_byte1, 5
    b.setvarb(WM_GSVAR_BYTE1, 5);
    // 5 bossSeamon objects
    b.mapobj(500, -200, 0, 3300, SH_SEA_0_0, STRATEGY_BOSS_SEAMON);
    b.mapobj(500, 0, 0, 3000, SH_SEA_0_0, STRATEGY_BOSS_SEAMON);
    b.mapobj(500, 200, 0, 3300, SH_SEA_0_0, STRATEGY_BOSS_SEAMON);
    b.mapobj(500, 0x0400, 0, 3500, SH_SEA_0_0, STRATEGY_BOSS_SEAMON);
    b.mapobj(500, 0x0400, 0, 3500, SH_SEA_0_0, STRATEGY_BOSS_SEAMON);
    // 4 torpedo spawners
    b.mapobj(500, -0x0600, 0, 1200, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(500, 0x0600, 0, 1200, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(500, -0x0400, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    b.mapobj(500, 0x0400, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);

    // .seatest — wait for all seamons destroyed (gsvar_byte1 == 0)
    b.label("level2_3b.seatest");
    b.mapwait(1);
    let level2_3b_seatest_ptr = b.mapcode65816_inline();

    // loop back to .waitabit
    b.mapgoto("level2_3b.waitabit");

    // .carryon — boss phase
    b.label("level2_3b.carryon");

    // mapwaitboss nosound — no trigse, no bgm fadeout/boss music
    b.mapwait(100);
    // chkbossdead loop
    b.label("level2_3b.bosswait.loop");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, "level2_3b.bosswait.cont");
    b.mapgoto("level2_3b.bosswait.loop");
    b.label("level2_3b.bosswait.cont");
    // cantdie + cleanup inline blocks
    let level2_3b_mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let level2_3b_mapwaitboss_cleanup_ptr = b.mapcode65816_inline();

    // markboss boss23
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);

    // IFEQ 1 block is disabled (conditional assembly = false), skipped.

    b.maprts();

    // LEVEL2_3.ASM wrapper tail: mapwait 1000, mapjsr cl_bridge, mapend.
    b.mapwait(1000);
    b.mapjsr("cl_bridge");
    b.mapend(1);

    // CL_BRIDG.ASM — clear demo (bridge type) appended as subroutine.
    submaps::append_cl_bridge_submap(&mut b);

    b.resolve();

    // C label-ptr lookups (fall back to 0 in C; must exist here).
    assert!(b.lookup_label("level2_3.skillfly_bonus_0_skip").is_some());
    assert!(b.lookup_label("level2_3.fog_guard_continue").is_some());
    assert!(b.lookup_label("level2_3b.carryon").is_some());
    assert!(b.lookup_label("level2_3b.waitabit").is_some());
    assert!(b.lookup_label("level2_3b.seatest").is_some());

    let (data, labels) = b.finish();

    // C `register_level2_3_inline_callbacks()` — registration-call order.
    // NOTE: in C the native BG_1_4B_1_L registration happens between the
    // setvar_inline and 2_3c trigse inline registrations; the wrapper keeps
    // native and inline lists separate but each preserves its call order.
    Route2Level::new(
        data,
        labels,
        vec![(MAP_CB_BG_1_4B_1_L, "level2_3_bg_1_4b_1")],
        vec![
            (
                level2_3_skillfly_bonus_guard_ptr,
                "level2_3_skillfly_bonus_guard",
            ),
            (level2_3_fog_guard_ptr, "level2_3_fog_guard"),
            (level2_3_setvar_inline_ptr, "level2_3_setvar_inline"),
            (level2_3c_trigse_ptr, "level1_1_mapwaitboss_trigse"),
            (level2_3b_trigger_ptr, "level2_3b_trigger_check"),
            (level2_3b_seatest_ptr, "level2_3b_seatest_check"),
            (
                level2_3b_mapwaitboss_cantdie_ptr,
                "level1_1_mapwaitboss_cantdie",
            ),
            (
                level2_3b_mapwaitboss_cleanup_ptr,
                "level1_1_mapwaitboss_cleanup",
            ),
        ],
    )
}
