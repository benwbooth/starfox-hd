//! Shared submaps used by the route2 level wrappers.
//!
//! DUPLICATE: consolidate. `append_map1_1a_submap` is a route2-local copy of
//! the one in `levels/level1_1.rs`, and the `cl_*` clear demos mirror the C
//! `append_cl_*_submap` helpers in `src/map/levels.c` that are shared across
//! route lanes. They live here so route lanes never edit shared files.

use super::rc::*;
use crate::builder::MapBuilder;

/// C `append_map1_1a_submap()` — MAP1_1A.ASM shared launch corridor.
pub fn append_map1_1a_submap(b: &mut MapBuilder) {
    b.label("map1_1a");
    b.mapobj(0, 0, 0, 250, SH_OP_0, IS_GND);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250, SH_OP_1, IS_NOCOLL);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (100 << 3), SH_OP_0, IS_GND);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (100 << 3), SH_OP_1, IS_NOCOLL);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (200 << 3), SH_OP_0, IS_GND);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (200 << 3), SH_OP_1, IS_NOCOLL);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);

    b.mapobj(0, -40, 0, -200, SH_IMYSHIP_4, IS_SHIPINTRO);
    b.setalvarw(AL_SWORD1, -70);
    b.setalvarb(AL_SBYTE1, 60);
    b.mapobj(0, 40, 0, -200, SH_IMYSHIP_4, IS_SHIPINTRO);
    b.setalvarw(AL_SWORD1, -70);
    b.setalvarb(AL_SBYTE1, 50);
    b.mapobj(0, 0, 0, -300, SH_IMYSHIP_4, IS_SHIPINTRO);
    b.setalvarw(AL_SWORD1, -100);
    b.setalvarb(AL_SBYTE1, -1);

    b.label("map1_1a.here2");
    b.mapwait((100 << 3) - MEDPSPEED);
    b.mapobj(0, 0, 0, 250 + (200 << 3), SH_OP_0, IS_GND);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (200 << 3), SH_OP_1, IS_NOCOLL);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.maploop("map1_1a.here2", 8);

    b.label("map1_1a.here3");
    b.mapwait((100 << 3) - MEDPSPEED);
    b.mapobj(0, 0, 0, 250 + (200 << 3), SH_OP_2, IS_GND);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapif_builtin(MAP_CB_CHKSTRATDONE1, "map1_1a.fin");
    b.mapgoto("map1_1a.here3");
    b.label("map1_1a.fin");
    b.maprts();
}

/// C `append_cl_earth_submap()` — CL_EARTH.ASM clear demo.
pub fn append_cl_earth_submap(b: &mut MapBuilder) {
    b.label("cl_earth");
    b.mapmother(0, 0, 0, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    b.mapplayeroutview();
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_CLEAR_EARTH_L);
    b.setbgm(BGM_FANFARE);
    b.mapwait(3300);

    b.setvarb(WM_STAGECLEAR, 1);
    b.sendmsg(1);
    b.mapwait(CL_GND_FRIENDWAIT);

    b.mapif_builtin(MAP_CB_FROG_ALIVE, "cl_earth.frog_alive");
    b.mapgoto("cl_earth.nf");
    b.label("cl_earth.frog_alive");
    b.mapobj(CL_GND_FRIENDWAIT, 2000, -50, 50, SH_MYSHIP_4, IS_CLSHIPEARTHB);
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_FROG);
    b.label("cl_earth.nf");

    b.mapif_builtin(MAP_CB_BUNNY_ALIVE, "cl_earth.bunny_alive");
    b.mapgoto("cl_earth.nb");
    b.label("cl_earth.bunny_alive");
    b.mapobj(CL_GND_FRIENDWAIT, -2000, -50, 50, SH_MYSHIP_4, IS_CLSHIPEARTHA);
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_BUNNY);
    b.label("cl_earth.nb");

    b.mapif_builtin(MAP_CB_COCK_ALIVE, "cl_earth.cock_alive");
    b.mapgoto("cl_earth.nc");
    b.label("cl_earth.cock_alive");
    b.mapobj(CL_GND_FRIENDWAIT, 0, 1000, -700, SH_MYSHIP_4, IS_CLSHIPEARTHC);
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_COCK);
    b.label("cl_earth.nc");

    b.mapwait(5000);
    b.label("cl_earth.sdloop");
    b.mapif_builtin(MAP_CB_CHKSTAGEDONE, "cl_earth.sdcont");
    b.mapgoto("cl_earth.sdloop");
    b.label("cl_earth.sdcont");
    b.setvarb(WM_CLB2, 0);
    b.setvarb(WM_STAGECLEAR, 0);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_PRINTLEVELFIN);
    b.label("cl_earth.eswait");
    b.mapwait(1);
    b.maploop("cl_earth.eswait", 100);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_WIPEOUT);
    b.setbgm(BGM_FADEOUT);
    b.mapwait(90 * MEDPSPEED);
    b.setvarb(WM_CLB2, 1);
    b.maprts();
}

/// C `append_cl_dive_submap()` — CL_DIVE.ASM clear demo.
pub fn append_cl_dive_submap(b: &mut MapBuilder) {
    b.label("cl_dive");
    b.mapplayeroutview();
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_FANFARE);
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_DIVE_L);
    b.mapwait(2800);

    b.setvarb(WM_STAGECLEAR, 1);
    b.sendmsg(1);
    b.mapwait(CL_GND_FRIENDWAIT);

    b.mapif_builtin(MAP_CB_FROG_ALIVE, "cl_dive.frog_alive");
    b.mapgoto("cl_dive.nf");
    b.label("cl_dive.frog_alive");
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_FROG);
    b.mapobj(CL_GND_FRIENDWAIT, 200, SPACE_VIEWCY, 50, SH_MYSHIP_4, IS_CLSHIPDIVEB);
    b.label("cl_dive.nf");

    b.mapif_builtin(MAP_CB_BUNNY_ALIVE, "cl_dive.bunny_alive");
    b.mapgoto("cl_dive.nb");
    b.label("cl_dive.bunny_alive");
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_BUNNY);
    b.mapobj(CL_GND_FRIENDWAIT, -200, SPACE_VIEWCY, 50, SH_MYSHIP_4, IS_CLSHIPDIVEA);
    b.label("cl_dive.nb");

    b.mapif_builtin(MAP_CB_COCK_ALIVE, "cl_dive.cock_alive");
    b.mapgoto("cl_dive.nc");
    b.label("cl_dive.cock_alive");
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_COCK);
    b.mapobj(CL_GND_FRIENDWAIT, 0, SPACE_VIEWCY - 40, -50, SH_MYSHIP_4, IS_CLSHIPDIVEC);
    b.label("cl_dive.nc");

    b.mapwait(5000);
    b.setvarb(WM_CLB2, 0);
    b.setvarb(WM_STAGECLEAR, 0);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_PRINTLEVELFIN);
    b.label("cl_dive.eswait");
    b.mapwait(1);
    b.maploop("cl_dive.eswait", 100);

    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_CL_DIVE_CLEAR_ENGINESND);
    b.qfadedown();
    b.waitfade();
    b.setvarb(WM_CLB2, 1);
    b.maprts();
}

/// C `append_cl_bridge_submap()` — CL_BRIDG.ASM clear demo.
pub fn append_cl_bridge_submap(b: &mut MapBuilder) {
    b.label("cl_bridge");
    b.mapplayeroutview();
    b.setbgm(BGM_FADEOUT);
    b.mapwait(2200);

    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_CLEAR_BRIDGE_L);
    b.setbgm(BGM_FANFARE);
    b.mapwait(2900);

    b.setvarb(WM_STAGECLEAR, 1);
    b.sendmsg(1);
    b.mapwait(CL_GND_FRIENDWAIT);

    b.mapif_builtin(MAP_CB_FROG_ALIVE, "cl_bridge.frog_alive");
    b.mapgoto("cl_bridge.nf");
    b.label("cl_bridge.frog_alive");
    b.mapobj(CL_GND_FRIENDWAIT, -1000, -300, 50, SH_MYSHIP_4, IS_CLSHIPBRIDGEB);
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_FROG);
    b.label("cl_bridge.nf");

    b.mapif_builtin(MAP_CB_BUNNY_ALIVE, "cl_bridge.bunny_alive");
    b.mapgoto("cl_bridge.nb");
    b.label("cl_bridge.bunny_alive");
    b.mapobj(CL_GND_FRIENDWAIT, 1000, -300, 50, SH_MYSHIP_4, IS_CLSHIPBRIDGEA);
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_BUNNY);
    b.label("cl_bridge.nb");

    b.mapif_builtin(MAP_CB_COCK_ALIVE, "cl_bridge.cock_alive");
    b.mapgoto("cl_bridge.nc");
    b.label("cl_bridge.cock_alive");
    b.mapobj(CL_GND_FRIENDWAIT, 0, 0, -2000, SH_MYSHIP_4, IS_CLSHIPBRIDGEC);
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_COCK);
    b.label("cl_bridge.nc");

    b.mapwait(5000);
    b.setvarb(WM_CLB2, 0);
    b.setvarb(WM_STAGECLEAR, 0);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_PRINTLEVELFIN);
    b.label("cl_bridge.eswait");
    b.mapwait(1);
    b.maploop("cl_bridge.eswait", 100);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_WIPEOUT);
    b.setbgm(BGM_FADEOUT);
    b.mapwait(32 * MEDPSPEED);
    b.maprts();
}

/// C `append_cl_turn_submap()` — CL_TURN.ASM clear demo (with clfish sub).
pub fn append_cl_turn_submap(b: &mut MapBuilder) {
    b.label("cl_turn");
    b.mapplayeroutview();
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_FANFARE);
    b.mapwait(1800);

    b.mapjsr("cl_turn.clfish");

    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_CLEAR_TURN_L);
    b.mapwait(1000);

    b.setvarb(WM_STAGECLEAR, 1);
    b.sendmsg(1);
    b.mapwait(CL_GND_FRIENDWAIT);

    b.mapif_builtin(MAP_CB_FROG_ALIVE, "cl_turn.frog_alive");
    b.mapgoto("cl_turn.nf");
    b.label("cl_turn.frog_alive");
    b.mapobj(CL_GND_FRIENDWAIT, 700, SPACE_VIEWCY, 50, SH_MYSHIP_4, IS_CLSHIPTURNB);
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_FROG);
    b.label("cl_turn.nf");

    b.mapif_builtin(MAP_CB_BUNNY_ALIVE, "cl_turn.bunny_alive");
    b.mapgoto("cl_turn.nb");
    b.label("cl_turn.bunny_alive");
    b.mapobj(CL_GND_FRIENDWAIT, -500, SPACE_VIEWCY, 50, SH_MYSHIP_4, IS_CLSHIPTURNA);
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_BUNNY);
    b.label("cl_turn.nb");

    b.mapif_builtin(MAP_CB_COCK_ALIVE, "cl_turn.cock_alive");
    b.mapgoto("cl_turn.nc");
    b.label("cl_turn.cock_alive");
    b.mapobj(CL_GND_FRIENDWAIT, 0, SPACE_VIEWCY + 400, -3000, SH_MYSHIP_4, IS_CLSHIPTURNC);
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_COCK);
    b.label("cl_turn.nc");

    b.mapwait(4000);
    b.mapjsr("cl_turn.clfish");
    b.mapwait(4000);

    b.setvarb(WM_CLB2, 0);
    b.setvarb(WM_STAGECLEAR, 0);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_PRINTLEVELFIN);
    b.mapwait(9000);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_WIPEOUT);
    b.setbgm(BGM_FADEOUT);
    b.mapwait(32 * MEDPSPEED);
    b.maprts();

    // clfish subroutine: map_sfish 0,0000,100,1000,9
    b.label("cl_turn.clfish");
    b.map_sfish(0, 0, 100, 1000, 9);
    b.maprts();
}

/// Inline script ptrs captured while appending the TRUCKER.ASM submap.
pub struct TruckerPtrs {
    pub biker_check: u16,
    pub trigger: u16,
}

/// C `append_trucker_submap()` — TRUCKER.ASM Mad Trucker boss subroutine.
pub fn append_trucker_submap(b: &mut MapBuilder) -> TruckerPtrs {
    b.label("level2_6.trucker");

    // Lines 2-3: initial biker pair
    b.mapobj(0x1000, -0x400, -60, 1000, SH_AIR_1_PROXY, STRAT_ADDR_MADBIKER);
    b.mapobj(0x1000, -0x300, -60, 0x0300, SH_AIR_1_PROXY, STRAT_ADDR_MADBIKER);

    // Lines 4-6: .mad loop — wall/boulder obstacles x6
    // (C source used octal literals -060 == -48 for the y coordinate.)
    b.label("level2_6.trucker.mad");
    b.mapobj(0x1000, -0x050, -0o60, 4000, SH_WALL_4_PROXY, IS_HARD180YR);
    b.mapobj(0x1000, -0x200, -0o60, 4000, SH_BOU_1B_PROXY, IS_HARD180YR);
    b.maploop("level2_6.trucker.mad", 6);

    // Lines 8-9: more bikers
    b.mapobj(0, -50, -60, -0x200, SH_AIR_1_PROXY, STRAT_ADDR_MADBIKER);
    b.mapobj(0x100, 50, -10, -0x400, SH_AIR_1_PROXY, STRAT_ADDR_MADBIKER);

    // Lines 11-23: .loop — wait for all bikers destroyed
    b.label("level2_6.trucker.loop");
    let biker_check = b.mapcode65816_inline();
    b.mapwait(100);
    b.mapgoto("level2_6.trucker.loop");

    // Lines 24-30: .carryon — boss entrance
    b.label("level2_6.trucker.carryon");
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_BOSS1);
    // trigse $0b (boss approach sound)
    b.mapwait(3000);

    // Line 31: boss spawn
    b.mapobj(0, -0x200, -70, -0x300, SH_BOSS_9_5_PROXY, STRAT_ADDR_MADTRUCKER);

    // Line 32: mapwait 1
    b.mapwait(1);

    // Lines 33-49: .loop2 — maptrigger check loop
    b.label("level2_6.trucker.loop2");
    let trigger = b.mapcode65816_inline();
    b.mapwait(500);
    b.mapgoto("level2_6.trucker.loop2");

    // Lines 50-52: .rightblockbit — dispatch to rightblock subroutine
    b.label("level2_6.trucker.rightblockbit");
    b.mapjsr("level2_6.trucker.rightblock");
    b.mapgoto("level2_6.trucker.loop2");

    // Lines 56-62: .rightblock subroutine — road obstacle spawns
    b.label("level2_6.trucker.rightblock");
    b.mapobj(0, 60, 0, 1600, SH_LINE_2_PROXY, STRAT_ADDR_ROADLINE);
    b.mapobj(0, 40, 0, 2400, SH_LINE_2_PROXY, STRAT_ADDR_ROADLINE);
    b.mapobj(0, 20, 0, 3100, SH_LINE_2_PROXY, STRAT_ADDR_ROADLINE);
    b.mapobj(0, 0, 0, 3400, SH_LINE_2_PROXY, STRAT_ADDR_ROADLINE);
    b.mapobj(0, 90, -60, 3600, SH_BOU_1B_PROXY, IS_HARD180YR);
    b.maprts();

    // Lines 65-73: .continue — boss defeated
    b.label("level2_6.trucker.continue");
    // Original 65816 block: lda #0 / sta.l m_bossmaxHP
    b.setvarw(WM_BOSSMAXHP, 0);
    b.setbgm(BGM_FADEOUT);
    b.maprts();

    TruckerPtrs { biker_check, trigger }
}
