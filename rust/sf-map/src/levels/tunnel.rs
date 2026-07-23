//! Shared tunnel-exit map fragments transcribed from MEXITMAP.ASM.

use crate::builder::MapBuilder;
use crate::consts::{
    al, cb, is, sh, wm, CL_GND_FRIENDWAIT, DEG180, DEG45, DEG90, MEDPSPEED, SPACE_VIEWCY,
};

const DEG135: i32 = 96;
const DEG225: i32 = 160;
const DEG315: i32 = 224;

const SH_EXITLIGHT: u16 = 1;
const SH_MWIREEXIT: u16 = 28;
const SH_MEXITFACE: u16 = 38;
const SH_EXIT_2: u16 = 72;
const SH_BMTUNNELFACE: u16 = 73;
const SH_MBLACKFACE: u16 = 74;
const SH_LWIREEXIT: u16 = 29;
const SH_BLTUNNELFACE: u16 = 30;
const SH_EXIT_1: u16 = 32;
const SH_LBLACKFACE: u16 = 33;
const SH_EXITFACE: u16 = 37;

const IS_EXITLIGHT3: u32 = 4;
const IS_EXITLIGHT4: u32 = 5;
const IS_EXITLIGHT5: u32 = 6;
const IS_EXITLIGHT6: u32 = 7;
const IS_EXIT: u32 = 12;
const IS_EXITOPEN: u32 = 13;
const IS_EXITOPENSND: u32 = 14;

const MTEXIT_MIN_X: i32 = -50;
const MTEXIT_MAX_X: i32 = 50;
const MTEXIT_MIN_Y: i32 = -95;
const MTEXIT_MAX_Y: i32 = -25;
const MTUNNEL_VIEW_CY: i32 = -60;
const TDIST: i32 = 4000;
const TELEN: i32 = 100;
const TLEN: i32 = 1000;

fn lights(b: &mut MapBuilder, z: i32, strat: u32) {
    for (x, y, rot) in [
        (MTEXIT_MAX_X, MTEXIT_MAX_Y, DEG45),
        (MTEXIT_MAX_X, MTEXIT_MIN_Y, DEG135),
        (MTEXIT_MIN_X, MTEXIT_MAX_Y, DEG315),
        (MTEXIT_MIN_X, MTEXIT_MIN_Y, DEG225),
    ] {
        b.mapobjzrot(0, x, y, z, SH_EXITLIGHT, strat, rot);
    }
}

fn large_lights(b: &mut MapBuilder, z: i32, strat: u32) {
    const LTEXIT_MIN_X: i32 = -70;
    const LTEXIT_MAX_X: i32 = 70;
    const LTEXIT_MIN_Y: i32 = -100;
    const LTEXIT_MAX_Y: i32 = -25;
    for (x, y, rot) in [
        (LTEXIT_MAX_X, LTEXIT_MAX_Y, DEG45),
        (LTEXIT_MAX_X, LTEXIT_MIN_Y, DEG135),
        (LTEXIT_MIN_X, LTEXIT_MAX_Y, DEG315),
        (LTEXIT_MIN_X, LTEXIT_MIN_Y, DEG225),
    ] {
        b.mapobjzrot(0, x, y, z, SH_EXITLIGHT, strat, rot);
    }
}

/// `Ltunnelexit` (LEXITMAP.ASM): complete large-tunnel exit geometry and
/// player handoff used by FINALMAP and DM_END.
pub(crate) fn append_ltunnel_exit(b: &mut MapBuilder, label_prefix: &str) {
    b.mapplayercantdie();

    large_lights(b, TLEN - 800 + TELEN + TDIST, IS_EXITLIGHT3);
    large_lights(b, TLEN - 600 + TELEN + TDIST, IS_EXITLIGHT4);
    large_lights(b, TLEN - 400 + TELEN + TDIST, IS_EXITLIGHT5);
    large_lights(b, TLEN - 200 + TELEN + TDIST, IS_EXITLIGHT6);
    large_lights(b, TLEN - 100 + TELEN + TDIST, is::NOCOLL);

    b.mapobj(0, 0, 0, TLEN + TELEN + TDIST - 1, SH_LWIREEXIT, is::NOCOLL);
    b.mapobj(0, 0, 60, TDIST + TELEN - 100, SH_BLTUNNELFACE, is::GND);
    b.mapobj(TDIST, 0, 0, TDIST, SH_EXIT_1, IS_EXIT);

    let after_mode = format!("{label_prefix}.after_inltexit");
    b.mapif_builtin(cb::IS_PLAYER_DEAD, &after_mode);
    b.mapcodejsl_builtin(cb::SET_PLAYER_INLTEXIT_L);
    b.label(&after_mode);

    b.mapobj(0, 0, -110, 10 + TLEN + TELEN, SH_EXITFACE, IS_EXITOPEN);
    b.setalvarw(al::SWORD1, 500);
    b.setalvarw(al::SWORD2, 0);
    b.setalvarb(al::SBYTE1, -10);
    b.mapobj(0, 0, 10, 10 + TLEN + TELEN, SH_EXITFACE, IS_EXITOPENSND);
    b.setalvarw(al::SWORD1, 500);
    b.setalvarw(al::SWORD2, 0);
    b.setalvarb(al::SBYTE1, 10);

    b.mapobj(0, 0, 0, TLEN + TELEN, SH_LBLACKFACE, is::NOCOLL);
    b.mapwait(100);
}

/// `mtunnelexit` (MEXITMAP.ASM): the complete medium-tunnel exit geometry,
/// player-mode transition, sliding doors, and terminal wait.
pub(crate) fn append_mtunnel_exit(b: &mut MapBuilder, label_prefix: &str) {
    b.mapplayercantdie();

    lights(b, TLEN - 800 + TELEN + TDIST, IS_EXITLIGHT3);
    lights(b, TLEN - 600 + TELEN + TDIST, IS_EXITLIGHT4);
    lights(b, TLEN - 400 + TELEN + TDIST, IS_EXITLIGHT5);
    lights(b, TLEN - 200 + TELEN + TDIST, IS_EXITLIGHT6);
    lights(b, TLEN - 100 + TELEN + TDIST, is::NOCOLL);

    b.mapobj(0, 0, 0, TLEN + TELEN + TDIST - 1, SH_MWIREEXIT, is::NOCOLL);
    b.mapobj(
        0,
        0,
        -MTUNNEL_VIEW_CY,
        TDIST + TELEN - 100,
        SH_BMTUNNELFACE,
        is::GND,
    );
    b.mapobj(TDIST, 0, 0, TDIST, SH_EXIT_2, IS_EXIT);

    // `mapplayermode InMTexit` skips the SET_PLAYER call if the player is
    // already dead, exactly as MAPMACS.INC's mapgotoifplayerdead wrapper.
    let after_mode = format!("{label_prefix}.after_inmtexit");
    b.mapif_builtin(cb::IS_PLAYER_DEAD, &after_mode);
    b.mapcodejsl_builtin(cb::SET_PLAYER_INMTEXIT_L);
    b.label(&after_mode);

    b.mapobj(0, 0, -110, 10 + TLEN + TELEN, SH_MEXITFACE, IS_EXITOPEN);
    b.setalvarw(al::SWORD1, 500);
    b.setalvarw(al::SWORD2, 0);
    b.setalvarb(al::SBYTE1, -10);
    b.mapobj(0, 0, 10, 10 + TLEN + TELEN, SH_MEXITFACE, IS_EXITOPENSND);
    b.setalvarw(al::SWORD1, 500);
    b.setalvarw(al::SWORD2, 0);
    b.setalvarb(al::SBYTE1, 10);

    b.mapobj(0, 0, 0, TLEN + TELEN, SH_MBLACKFACE, is::NOCOLL);
    b.mapwait(100);
}

/// `DM_LB1.ASM` — the last-base entrance used before FINALMAP on every
/// route. This is a complete map fragment: base/door geometry, the
/// `intoLB1` player cutscene, the `chkstratdone1` barrier, and the red-tunnel
/// background handoff.
pub(crate) fn append_dm_lb1(b: &mut MapBuilder, label_prefix: &str) {
    const BG_1_6B: i32 = 16;

    b.mapwait(MEDPSPEED * 44);
    b.setbgm(0x12);

    b.mapobj(0, 0, 0, 6000, sh::LAST_B_0, is::GND);
    b.setvarobj(wm::MAPVAR1);
    b.mapobj(0, 0, 0, 6000, sh::LAST_B_2, is::LASTB2);
    b.setalvarb(al::ROTY, DEG180);
    b.mapobj(0, 0, -(48 << 4), 6000, sh::LAST_B_3, is::LASTB3);
    b.mapobj(0, 0, (100 << 4) - 30, 6000 - 64, sh::DOOR_L, is::LASTB4);

    let after_mode = format!("{label_prefix}.after_intolb1");
    b.mapif_builtin(cb::IS_PLAYER_DEAD, &after_mode);
    b.mapcodejsl_builtin(cb::SET_PLAYER_INTOLB1_L);
    b.label(&after_mode);

    let wait = format!("{label_prefix}.wait");
    let cont = format!("{label_prefix}.cont");
    b.label(&wait);
    b.mapif_builtin(cb::CHKSTRATDONE1, &cont);
    b.mapwait(1);
    b.mapgoto(&wait);
    b.label(&cont);

    b.setbg(BG_1_6B);
    b.initbg();
}

/// `DM_END.ASM` — the complete post-Andross base escape and formation-flight
/// ending. The fragment ends in the same infinite wait as the ROM after
/// storing `LE_ENDOFGAME` in `levelfinished`.
pub(crate) fn append_dm_end(b: &mut MapBuilder, label_prefix: &str) {
    const BG_1_7A: i32 = 18;
    const BG_1_7B: i32 = 19;
    const BG_1_7C: i32 = 20;

    fn player_mode(b: &mut MapBuilder, cb_addr: u32, done: &str) {
        b.mapif_builtin(cb::IS_PLAYER_DEAD, done);
        b.mapcodejsl_builtin(cb_addr);
        b.label(done);
    }

    // Tunnel escape and inner last-base door sequence.
    player_mode(
        b,
        cb::SET_PLAYER_TOCSLOW_L,
        &format!("{label_prefix}.after_tocslow"),
    );
    append_ltunnel_exit(b, &format!("{label_prefix}.lexit"));
    b.mapwait(200);
    b.setbg(BG_1_7A);
    b.mapwait(MEDPSPEED * 3);
    b.initbg();

    b.mapobj(0, 0, SPACE_VIEWCY + 64, 2000, sh::DOOR_L, is::LSEQDOOR1);
    b.setalvarb(al::ROTX, 0);
    b.mapobj(
        0,
        0,
        SPACE_VIEWCY,
        (100 << 4) + 2000,
        sh::LAST_B_2,
        is::NOCOLL,
    );
    b.setalvarb(al::ROTX, DEG90);
    b.mapobj(
        0,
        0,
        SPACE_VIEWCY,
        ((148 << 4) - 30) + 2000,
        sh::LAST_B_3,
        is::NOCOLLANIM0,
    );
    b.setalvarb(al::ROTX, -DEG90);
    b.mapwait(1800 + 500);

    player_mode(
        b,
        cb::SET_PLAYER_OUTOFLB2A_L,
        &format!("{label_prefix}.after_outoflb2a"),
    );
    b.mapwait(500);
    b.setbg(BG_1_7B);
    b.initbg();
    b.mapcodejsl_builtin(cb::CLEARMAP_L);

    // Outside view of the ship escaping the exploding base.
    b.mapobj(0, 0, 0, 1000, sh::LAST_B_0, is::GND);
    b.setvarobj(wm::MAPVAR1);
    b.setalvarb(al::ROTY, DEG180);
    b.mapobj(0, 0, -(48 << 4), 1000, sh::LAST_B_3, is::LSEQDOOR2);
    b.mapobj(0, 0, 0, 1000, sh::MY_DEMOS, is::PSHIPOUTOFLB1);
    b.mapobj(0, -50, -1500, 1100, sh::NULLSHAPE, is::VIEWOUTOFLB1);

    b.mapwait(9500 - (MEDPSPEED * 6));
    b.sendmsg(1);

    // Surviving wingmen speak in frog/bunny/cock order.
    let frog_alive = format!("{label_prefix}.frog_alive_1");
    let frog_done = format!("{label_prefix}.frog_done_1");
    b.mapif_builtin(cb::FROG_ALIVE, &frog_alive);
    b.mapgoto(&frog_done);
    b.label(&frog_alive);
    b.mapwait(CL_GND_FRIENDWAIT);
    b.sendmsg(46);
    b.label(&frog_done);

    let bunny_alive = format!("{label_prefix}.bunny_alive_1");
    let bunny_done = format!("{label_prefix}.bunny_done_1");
    b.mapif_builtin(cb::BUNNY_ALIVE, &bunny_alive);
    b.mapgoto(&bunny_done);
    b.label(&bunny_alive);
    b.mapwait(CL_GND_FRIENDWAIT);
    b.sendmsg(26);
    b.label(&bunny_done);

    let cock_alive = format!("{label_prefix}.cock_alive_1");
    let cock_done = format!("{label_prefix}.cock_done_1");
    b.mapif_builtin(cb::COCK_ALIVE, &cock_alive);
    b.mapgoto(&cock_done);
    b.label(&cock_alive);
    b.mapwait(CL_GND_FRIENDWAIT);
    b.sendmsg(6);
    b.label(&cock_done);

    let first_wait = format!("{label_prefix}.first_wait");
    let first_done = format!("{label_prefix}.first_done");
    b.label(&first_wait);
    b.mapif_builtin(cb::CHKSTRATDONE1, &first_done);
    b.mapwait(1);
    b.mapgoto(&first_wait);
    b.label(&first_done);

    b.fadedown();
    b.waitfade();
    b.mapcodejsl_builtin(cb::CLEARMAP_L);
    b.setvarb24(wm::M_METERS, 0);
    b.mapcodejsl_builtin(cb::SETCHARMAPFROMMAP_L);

    // Final space formation and fly-past.
    b.setbg(BG_1_7C);
    b.initbg();
    b.mapobj(0, 0, SPACE_VIEWCY, 0, sh::MY_DEMO, is::PSHIPOUTOFLB3);
    b.setalvarb(al::SBYTE2, 45);
    b.mapobj(0, 0, SPACE_VIEWCY, 5000, sh::NULLSHAPE, is::VIEWOUTOFLB3);

    let no_cock = format!("{label_prefix}.no_cock");
    b.mapif_builtin(cb::COCK_ALIVE, &format!("{label_prefix}.cock_alive_2"));
    b.mapgoto(&no_cock);
    b.label(&format!("{label_prefix}.cock_alive_2"));
    b.mapobj(0, 0, SPACE_VIEWCY - 100, 0, sh::MY_DEMO, is::SHIPOUTOFLB3);
    b.setalvarw(al::SWORD1, 0);
    b.setalvarw(al::SWORD2, -100);
    b.setalvarw(al::PTR, 600);
    b.setalvarb(al::SBYTE2, 10);
    b.label(&no_cock);

    let no_frog = format!("{label_prefix}.no_frog");
    b.mapif_builtin(cb::FROG_ALIVE, &format!("{label_prefix}.frog_alive_2"));
    b.mapgoto(&no_frog);
    b.label(&format!("{label_prefix}.frog_alive_2"));
    b.mapobj(0, 70, SPACE_VIEWCY + 70, 200, sh::MY_DEMO, is::SHIPOUTOFLB3);
    b.setalvarb(al::ROTZ, DEG45);
    b.setalvarw(al::SWORD1, -100);
    b.setalvarw(al::SWORD2, 50);
    b.setalvarw(al::PTR, 400);
    b.setalvarb(al::SBYTE2, 20);
    b.label(&no_frog);

    let no_bunny = format!("{label_prefix}.no_bunny");
    b.mapif_builtin(cb::BUNNY_ALIVE, &format!("{label_prefix}.bunny_alive_2"));
    b.mapgoto(&no_bunny);
    b.label(&format!("{label_prefix}.bunny_alive_2"));
    b.mapobj(
        0,
        -70,
        SPACE_VIEWCY + 70,
        400,
        sh::MY_DEMO,
        is::SHIPOUTOFLB3,
    );
    b.setalvarb(al::ROTZ, -DEG45);
    b.setalvarw(al::SWORD1, 100);
    b.setalvarw(al::SWORD2, 75);
    b.setalvarw(al::PTR, 200);
    b.setalvarb(al::SBYTE2, 30);
    b.label(&no_bunny);

    b.mapwait(1000);
    b.fadeup();
    b.waitfade();

    let final_wait = format!("{label_prefix}.final_wait");
    let final_done = format!("{label_prefix}.final_done");
    b.label(&final_wait);
    b.mapif_builtin(cb::CHKSTRATDONE1, &final_done);
    b.mapwait(1);
    b.mapgoto(&final_wait);
    b.label(&final_done);
    b.setvarb(wm::LEVELFINISHED, 6);

    let forever = format!("{label_prefix}.forever");
    b.label(&forever);
    b.mapwait(30000);
    b.mapgoto(&forever);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::op;

    #[test]
    fn medium_exit_emits_all_twenty_rotated_lights_and_player_callback() {
        let mut light_builder = MapBuilder::new();
        for (z, strat) in [(4300, 4), (4500, 5), (4700, 6), (4900, 7), (5000, 10)] {
            lights(&mut light_builder, z, strat);
        }
        let (light_data, _) = light_builder.finish();
        assert_eq!(light_data.len(), 20 * 12);
        assert!(light_data
            .chunks_exact(12)
            .all(|entry| entry[0] == op::OBJZROT));

        let mut b = MapBuilder::new();
        append_mtunnel_exit(&mut b, "test.mexit");
        b.resolve();
        let (data, labels) = b.finish();
        assert!(labels
            .iter()
            .any(|(name, _)| name == "test.mexit.after_inmtexit"));
        assert!(data.windows(4).any(|bytes| {
            bytes
                == [
                    op::CODEJSL,
                    (cb::SET_PLAYER_INMTEXIT_L.wrapping_sub(1) & 0xff) as u8,
                    ((cb::SET_PLAYER_INMTEXIT_L.wrapping_sub(1) >> 8) & 0xff) as u8,
                    (cb::SET_PLAYER_INMTEXIT_L >> 16) as u8,
                ]
        }));
    }

    #[test]
    fn last_base_entrance_keeps_player_and_strategy_completion_gates() {
        let mut b = MapBuilder::new();
        append_dm_lb1(&mut b, "test.lb1");
        b.resolve();
        let (data, labels) = b.finish();

        for label in ["test.lb1.after_intolb1", "test.lb1.wait", "test.lb1.cont"] {
            assert!(
                labels.iter().any(|(name, _)| name == label),
                "missing {label}"
            );
        }
        assert!(data.windows(4).any(|bytes| {
            bytes
                == [
                    op::CODEJSL,
                    (cb::SET_PLAYER_INTOLB1_L.wrapping_sub(1) & 0xff) as u8,
                    ((cb::SET_PLAYER_INTOLB1_L.wrapping_sub(1) >> 8) & 0xff) as u8,
                    (cb::SET_PLAYER_INTOLB1_L >> 16) as u8,
                ]
        }));
    }
}
