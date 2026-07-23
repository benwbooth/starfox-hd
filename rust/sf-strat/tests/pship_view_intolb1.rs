//! Tick 88: pshipintolb1 + viewintolb1.

use sf_game::alien::{ASF_COLLDISABLE, ASF_INVISIBLE};
use sf_game::vars::{GF_NOZREMOVE, PSTF_INSEQ};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_into_lb1a_strat, player_sv as sv, pshipintolb1_istrat, pshipintolb1_strat,
    set_player_into_lb1, viewintolb1_istrat, viewintolb1_strat,
};

const MED_PSPEED: i16 = 65;
const MAX_PSPEED: i16 = 85;
const DEG90: u8 = 64;
const DEG180: u8 = 128;
const INVIEWDIST: i16 = 60;
const LTUNNEL_VIEWCY: i16 = -60;
const WM_GAMEFLAGS2: u16 = 0x155C;
const GF2_STRATFLAG1: u8 = 1;
const WM_MAPVAR1: u16 = 0x0320;
const WM_FLOATVAR1: u16 = 0x1569;
const ASF2_SFLAG1: u8 = 0x10;
const VIEWTYPE_NORM: u8 = 0;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn pshipintolb1_climb_then_roll() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    pshipintolb1_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[idx as usize].vel, MED_PSPEED as u8);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, DEG90 / 2);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 0);

    let rx0 = g.objs.aliens[idx as usize].rotx;
    pshipintolb1_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotx, rx0.wrapping_sub(2));
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, DEG90 / 2 - 1);
    assert!(g.objs.aliens[idx as usize].vel <= MED_PSPEED as u8);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 0);

    // Force state 0→1 (fall-through runs state 1 same frame → decbne 64→63)
    g.objs.aliens[idx as usize].sbyte1 = 1;
    pshipintolb1_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, DEG180 / 2 - 1);

    // State 1: open door flag at sbyte1==10, then roll
    g.objs.aliens[idx as usize].sbyte1 = 10;
    let rz0 = g.objs.aliens[idx as usize].rotz;
    pshipintolb1_strat(&mut g, idx);
    assert_ne!(g.vars.read_ext8(WM_GAMEFLAGS2) & GF2_STRATFLAG1, 0);
    assert_eq!(g.objs.aliens[idx as usize].rotz, rz0.wrapping_add(8));
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 9);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);

    // Force boost transition (state 2 runs same frame → sbyte1 40→39)
    g.objs.aliens[idx as usize].sbyte1 = 1;
    pshipintolb1_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 2);
    assert_eq!(g.objs.aliens[idx as usize].vel, MAX_PSPEED as u8);
    assert_eq!(g.vars.sv_i16(sv::BOOSTOBJ), idx as i16);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 39);
}

#[test]
fn pshipintolb1_handoff_to_ltunnel() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    assert_eq!(player, 0);
    g.vars.internal_playpt = 0;
    g.objs.aliens[0].sflags |= ASF_INVISIBLE;
    g.objs.aliens[0].worldz = 1000;
    g.vars.gameflags |= GF_NOZREMOVE;
    g.vars.write_ext8(WM_FLOATVAR1, 99);

    let gate = spawn(&mut g);
    g.objs.aliens[gate as usize].worldx = 40;
    g.objs.aliens[gate as usize].worldz = 2000;
    g.vars.write_ext16(WM_MAPVAR1, gate + 1);

    let ship = spawn(&mut g);
    g.objs.aliens[ship as usize].worldx = 0;
    g.objs.aliens[ship as usize].worldz = 1000;
    g.objs.aliens[ship as usize].rotz = 40;
    pshipintolb1_istrat(&mut g, ship);
    g.objs.aliens[ship as usize].stratstate = 2;
    g.objs.aliens[ship as usize].sbyte1 = 1;

    // One tick: chase + decbne → state 4 → handoff same frame
    pshipintolb1_strat(&mut g, ship);
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(g.objs.aliens[0].sflags & ASF_INVISIBLE, 0);
    assert_eq!(g.vars.sv_i16(sv::VIEWTOOBJ), 0);
    assert_eq!(g.vars.sv_u8(sv::VIEWTYPE), VIEWTYPE_NORM);
    assert_eq!(g.vars.pviewvelz, MAX_PSPEED);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), INVIEWDIST);
    assert_eq!(g.vars.viewdist, INVIEWDIST);
    assert_eq!(g.objs.aliens[0].vel, MAX_PSPEED as u8);
    assert_eq!(g.objs.aliens[0].sbyte2, 1);
    assert_eq!(g.objs.aliens[0].worldy, LTUNNEL_VIEWCY - 5);
    assert_eq!(g.objs.aliens[0].worldx, 0);
    assert_eq!(g.vars.player_posz, 1000);
    assert_eq!(
        g.vars.sv_i16(sv::PVIEWPOSZ),
        1000i16.wrapping_sub(MAX_PSPEED)
    );
    assert_eq!(g.vars.sv_i16(sv::PVIEWPOSY), LTUNNEL_VIEWCY);
    assert_eq!(g.vars.gameflags & GF_NOZREMOVE, 0);
    assert_eq!(g.vars.read_ext8(WM_FLOATVAR1), 0);
    // Speedto min during climb; handoff uses max — also speed chased toward gate
    assert_ne!(g.objs.aliens[ship as usize].worldz, 1000);
}

#[test]
fn player_reaches_map_target_and_builds_the_lb1_cutscene() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    g.vars.internal_playpt = player as i16;
    g.objs.aliens[player as usize].worldz = 0;
    g.vars.set_sv_i16(sv::VIEWCY, -50);

    let target = spawn(&mut g);
    g.objs.aliens[target as usize].worldz = 1700;
    // SETVAROBJ stores index+1; s_set_objtobevar decodes it.
    g.vars.write_ext16(WM_MAPVAR1, target + 1);

    set_player_into_lb1(&mut g, player);
    player_into_lb1a_strat(&mut g, player);

    assert_ne!(g.vars.pstratflags & PSTF_INSEQ, 0);
    assert_ne!(g.vars.gameflags & GF_NOZREMOVE, 0);
    assert_ne!(g.objs.aliens[player as usize].sflags & ASF_INVISIBLE, 0);
    let ship = g.vars.sv_i16(sv::VIEWTOOBJ);
    assert!(ship >= 0 && ship as u16 != player);
    assert_eq!(g.objs.aliens[ship as usize].sbyte1, DEG90 / 2);
    assert_eq!(g.objs.aliens[player as usize].worldz, MED_PSPEED);
}

#[test]
fn viewintolb1_offsets_follow_pship_state() {
    let mut g = Game::new();
    let ship = spawn(&mut g);
    g.objs.aliens[ship as usize].worldx = 10;
    g.objs.aliens[ship as usize].worldy = -50;
    g.objs.aliens[ship as usize].worldz = 500;
    g.objs.aliens[ship as usize].stratstate = 0;
    g.vars.set_sv_i16(sv::VIEWTOOBJ, ship as i16);
    g.vars.set_sv_i16(sv::OUTDIST, 80);

    let cam = spawn(&mut g);
    // Force deterministic sflag1 off by clearing after init
    viewintolb1_istrat(&mut g, cam);
    g.objs.aliens[cam as usize].sflags2 &= !ASF2_SFLAG1;
    assert_eq!(g.objs.aliens[cam as usize].sword1, 80);
    assert_eq!(g.objs.aliens[cam as usize].sbyte1, DEG90 / 3);
    assert_eq!(g.objs.aliens[cam as usize].vel, MED_PSPEED as u8);

    viewintolb1_strat(&mut g, cam);
    assert_eq!(g.objs.aliens[cam as usize].sword1, 84);
    assert_eq!(g.objs.aliens[cam as usize].ptr as i16, 2);
    assert_eq!(g.objs.aliens[cam as usize].worldz, 500 - 84);
    assert_eq!(g.objs.aliens[cam as usize].worldx, 10 + 2);
    assert_eq!(g.vars.sv_i16(sv::VIEWPOSZ), 500 - 84);
    assert_eq!(g.vars.sv_i16(sv::BGSSCROLLZ), -50); // worldy + sword2(0)

    // State 1: chase sword1→5, sword2−=8
    g.objs.aliens[ship as usize].stratstate = 1;
    viewintolb1_strat(&mut g, cam);
    assert_eq!(g.objs.aliens[cam as usize].sword2, -8);
    assert!(g.objs.aliens[cam as usize].sword1 < 84);

    // State 2: chase sword2→−inviewdist, ptr→0
    g.objs.aliens[ship as usize].stratstate = 2;
    g.objs.aliens[cam as usize].ptr = 20;
    viewintolb1_strat(&mut g, cam);
    assert!((g.objs.aliens[cam as usize].ptr as i16).abs() < 20);
    assert!(g.objs.aliens[cam as usize].sword2 < -8);

    // State 4: remove cam
    g.objs.aliens[ship as usize].stratstate = 4;
    viewintolb1_strat(&mut g, cam);
    assert_eq!(g.objs.aldead, 1);
}
