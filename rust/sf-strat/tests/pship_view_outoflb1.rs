//! Tick 86: pshipoutoflb1 + viewoutoflb1.

use sf_game::alien::ASF_COLLDISABLE;
use sf_game::vars::GF_STRATDONE1;
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_sv as sv, pshipoutoflb1_istrat, pshipoutoflb1_strat, viewoutoflb1_istrat,
    viewoutoflb1_strat,
};

const MED_PSPEED: i16 = 65;
const MAX_PSPEED: i16 = 85;
const DEG90: u8 = 64;
const DEG45: u8 = 32;
const VIEWTYPE_FPOS: u8 = 2;
const VIEWTYPE_TOOBJ: u8 = 1;
const WM_GAMEFLAGS2: u16 = 0x155C;
const GF2_STRATFLAG1: u8 = 1;
const ASF2_SFLAG3: u8 = 0x40;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn pshipoutoflb1_init_climb_and_lineup() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldy = -500; // still "lower" than -1000
    g.vars.gameflags |= GF_STRATDONE1;

    pshipoutoflb1_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[idx as usize].vel, MED_PSPEED as u8);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 0u8.wrapping_sub(DEG90));
    assert_eq!(g.vars.sv_i16(sv::VIEWTOOBJ), idx as i16);
    assert_eq!(g.vars.gameflags & GF_STRATDONE1, 0);

    // Below -1000 threshold: only rotz spin
    let rz0 = g.objs.aliens[idx as usize].rotz;
    pshipoutoflb1_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotz, rz0.wrapping_sub(4));
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 0);

    // Climb: worldy < -1000; pitch already at -deg45 → nextstate
    g.objs.aliens[idx as usize].worldy = -1200;
    g.objs.aliens[idx as usize].rotx = 0u8.wrapping_sub(DEG45);
    pshipoutoflb1_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 22); // 23 set then same-frame decbne
    assert!(g.objs.aliens[idx as usize].vel > MED_PSPEED as u8); // speedto max

    // Lineup: force achase complete
    g.objs.aliens[idx as usize].rotx = 0;
    g.objs.aliens[idx as usize].rotz = 0;
    g.objs.aliens[idx as usize].sbyte1 = 5;
    pshipoutoflb1_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 2);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 69); // 70 then same-frame decbne
}

#[test]
fn pshipoutoflb1_friends_and_boost_done() {
    const SH_LAST_B_0: u16 = 212;
    const SH_LAST_B_3: u16 = 214;
    let mut g = Game::new();
    g.vars.frog_hp = 3;
    g.vars.bunny_hp = 3;
    g.vars.falcon_hp = 3;
    let idx = spawn(&mut g);
    // Base pieces to remove
    let b0 = spawn(&mut g);
    g.objs.aliens[b0 as usize].shape = SH_LAST_B_0;
    let b3 = spawn(&mut g);
    g.objs.aliens[b3 as usize].shape = SH_LAST_B_3;

    pshipoutoflb1_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].stratstate = 3;
    let before = g.objs.active_indices().len();
    pshipoutoflb1_strat(&mut g, idx);
    // The two freed slots may be reused immediately for the three wingmen;
    // verify the base meshes disappeared rather than requiring those numeric
    // pool slots to remain inactive.
    assert!(!g
        .objs
        .aliens
        .iter()
        .any(|al| al.active && al.shape == SH_LAST_B_0));
    assert!(!g
        .objs
        .aliens
        .iter()
        .any(|al| al.active && al.shape == SH_LAST_B_3));
    assert!(g.objs.active_indices().len() > before - 2); // friends spawned
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 4);

    // Boost done
    g.objs.aliens[idx as usize].stratstate = 5;
    g.objs.aliens[idx as usize].sbyte1 = 1;
    pshipoutoflb1_strat(&mut g, idx);
    assert_ne!(g.vars.gameflags & GF_STRATDONE1, 0);
    assert_eq!(g.vars.sv_u8(sv::VIEWTYPE), VIEWTYPE_FPOS);
}

#[test]
fn viewoutoflb1_tracks_pship_state() {
    let mut g = Game::new();
    let ship = spawn(&mut g);
    g.objs.aliens[ship as usize].stratstate = 0;
    g.vars.set_sv_i16(sv::VIEWTOOBJ, ship as i16);
    g.vars.set_sv_i16(sv::OUTDIST, 120);
    g.vars.write_ext8(WM_GAMEFLAGS2, GF2_STRATFLAG1);

    let cam = spawn(&mut g);
    viewoutoflb1_istrat(&mut g, cam);
    assert_ne!(g.objs.aliens[cam as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[cam as usize].sword1, 120);
    assert_eq!(g.objs.aliens[cam as usize].rotx, 0u8.wrapping_sub(DEG45));
    assert_eq!(g.vars.sv_u8(sv::VIEWTYPE), VIEWTYPE_FPOS | VIEWTYPE_TOOBJ);
    assert_eq!(g.vars.read_ext8(WM_GAMEFLAGS2) & GF2_STRATFLAG1, 0);

    // State 0 ship → speedto max-5
    g.objs.aliens[cam as usize].vel = 0;
    viewoutoflb1_strat(&mut g, cam);
    assert!(g.objs.aliens[cam as usize].vel > 0);
    assert!(g.objs.aliens[cam as usize].vel <= (MAX_PSPEED - 5) as u8);

    // State 4 ship → sflag3 + stop
    g.objs.aliens[ship as usize].stratstate = 4;
    viewoutoflb1_strat(&mut g, cam);
    assert_ne!(g.objs.aliens[cam as usize].sflags2 & ASF2_SFLAG3, 0);
    assert_eq!(g.objs.aliens[cam as usize].vel, 0);

    // viewpos copied
    assert_eq!(
        g.vars.sv_i16(sv::VIEWPOSX),
        g.objs.aliens[cam as usize].worldx
    );
}
