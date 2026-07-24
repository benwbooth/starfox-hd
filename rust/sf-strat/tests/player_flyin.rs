//! Tick 99: player fly-in / straight / speed / on-cont / cred / divegnd.

use sf_core::player_view::PlayerViewMode;
use sf_game::alien::{ASF_COLLDISABLE, ASF_INVISIBLE};
use sf_game::vars::{PSF3_ENGINESND, PSF_NOCTRL, PSF_NOFIRE, PSTF_NOVDISTC};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_colony_flyin_istrat, player_colony_flyin_strat, player_cred_istrat, player_cred_strat,
    player_divegnd_istrat, player_inside_space_flyin_istrat, player_inside_space_flyin_strat,
    player_ltunnel_flyin_istrat, player_ltunnel_flyin_strat, player_on_cont_istrat,
    player_planet_flyin_istrat, player_planet_flyin_strat, player_space_flyin_istrat,
    player_space_flyin_strat, player_speedstop_istrat, player_speedup_istrat,
    player_straight_strat, player_sv as sv, set_player_cred,
};

const MED_PSPEED: i16 = 65;
const MAX_PSPEED: i16 = 85;
const INVIEWDIST: i16 = 60;
const SPACE_VIEWCY: i16 = -60;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn space_flyin_chases_then_hands_off() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.set_sv_i16(sv::VIEWCY, SPACE_VIEWCY);
    player_space_flyin_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -400);
    assert_ne!(g.vars.pstratflags & PSTF_NOVDISTC, 0);
    assert_eq!(g.objs.aliens[idx as usize].vel, MED_PSPEED as u8);

    // Force done
    g.objs.aliens[idx as usize].worldy = SPACE_VIEWCY;
    let od0 = g.vars.sv_i16(sv::OUTDIST);
    player_space_flyin_strat(&mut g, idx);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), od0.wrapping_add(3));
    assert_eq!(g.vars.pstratflags & PSTF_NOVDISTC, 0);
}

#[test]
fn inside_space_flyin_sets_inviewdist() {
    let mut g = Game::new();
    sf_strat::table::register_all(&mut g);
    let idx = spawn(&mut g);
    g.vars.set_sv_i16(sv::VIEWCY, SPACE_VIEWCY);
    player_inside_space_flyin_istrat(&mut g, idx);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), INVIEWDIST);
    assert_eq!(g.vars.viewdist, INVIEWDIST);
    let flyin_tick = g.objs.aliens[idx as usize]
        .stratptr
        .expect("inside fly-in callback");
    g.objs.aliens[idx as usize].worldy = SPACE_VIEWCY;
    player_inside_space_flyin_strat(&mut g, idx);
    assert_eq!(g.vars.player_view_mode, PlayerViewMode::EnteringCockpit);
    assert_ne!(
        g.objs.aliens[idx as usize].stratptr,
        Some(flyin_tick),
        "fly-in completion must install the cockpit transition"
    );
}

#[test]
fn planet_ltunnel_colony_flyin_inits() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.set_sv_i16(sv::VIEWCY, -50);
    player_planet_flyin_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -400);
    g.objs.aliens[idx as usize].worldy = -50;
    player_planet_flyin_strat(&mut g, idx);
    assert_eq!(g.vars.pstratflags & PSTF_NOVDISTC, 0);

    player_ltunnel_flyin_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -120);
    assert_eq!(g.objs.aliens[idx as usize].vel, MAX_PSPEED as u8);
    g.vars.set_sv_i16(sv::VIEWCY, -60);
    g.objs.aliens[idx as usize].worldy = -60;
    player_ltunnel_flyin_strat(&mut g, idx);

    player_colony_flyin_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vel, MAX_PSPEED as u8);
    g.objs.aliens[idx as usize].worldy = -60;
    player_colony_flyin_strat(&mut g, idx);
}

#[test]
fn straight_locks_center_cruise() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.set_sv_i16(sv::VIEWCY, -50);
    g.vars.set_sv_i16(sv::OUTVX, 100);
    g.vars.set_sv_u8(sv::ARROWS, 0xFF);
    player_straight_strat(&mut g, idx);
    assert_eq!(g.vars.sv_u8(sv::ARROWS), 0);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 0);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -50);
    assert_eq!(g.objs.aliens[idx as usize].vel, MED_PSPEED as u8);
    assert_eq!(g.vars.pviewvelz, MED_PSPEED);
}

#[test]
fn speedup_and_speedstop() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    let trigger = spawn(&mut g);
    g.vars.internal_playpt = player as i16;
    player_speedup_istrat(&mut g, trigger);
    assert_eq!(g.objs.aliens[player as usize].vel, MAX_PSPEED as u8);
    assert_eq!(g.objs.aliens[player as usize].sbyte2, 20);
    assert_eq!(
        g.vars.pviewvelz,
        ((MAX_PSPEED - MED_PSPEED) / 2) + MED_PSPEED
    );
    assert_eq!(g.objs.aldead, 1);

    g.objs.aldead = 0;
    player_speedstop_istrat(&mut g, trigger);
    assert_eq!(g.vars.sv_u8(sv::PLAYER_TOSPEED), 0);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn on_cont_and_cred() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    player_on_cont_istrat(&mut g, idx);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 200);
    assert_eq!(g.vars.viewdist, 200);
    assert_eq!(g.vars.pshipflags3 & PSF3_ENGINESND, 0);

    set_player_cred(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_INVISIBLE, 0);
    assert_ne!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);
    player_cred_istrat(&mut g, idx);
    player_cred_strat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
}

#[test]
fn divegnd_istrat_arms_noop_strat() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.set_sv_i8(sv::STAYBLACK, -1);
    player_divegnd_istrat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    assert_eq!(g.objs.aliens[idx as usize].worldx, 0);
}
