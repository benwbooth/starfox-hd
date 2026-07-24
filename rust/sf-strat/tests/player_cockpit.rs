//! ROM cockpit enter/exit SET_PLAYER* (PSTRATS.ASM).

use sf_core::player_view::PlayerViewMode;
use sf_game::vars::{OUTVIEWDIST, PSF_NOCTRL, PSF_NOFIRE, PSTF_NOVDISTC};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    make_all_med_pspeed, player_into_cock2_init, player_into_cock_strat, player_out_of_cock_strat,
    player_sv as sv, set_player_into_cock, set_player_out_of_cock, COCKPIT_EXIT_FRAMES,
};

#[test]
fn make_all_med_pspeed_sets_speeds() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    make_all_med_pspeed(&mut g, p);
    assert_eq!(g.objs.aliens[p as usize].vel, 65);
    assert_eq!(g.vars.pviewvelz, 65);
    assert_eq!(g.vars.sv_u8(sv::PLAYER_TOSPEED), 65);
    assert_eq!(g.vars.playervel_z, 65);
}

#[test]
fn into_cock_init_and_chase() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.objs.aliens[p as usize].worldx = 40;
    g.objs.aliens[p as usize].worldy = 0;
    g.vars.set_sv_i16(sv::OUTDIST, 8);

    set_player_into_cock(&mut g, p);
    assert!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE) != 0);
    assert!(g.vars.pstratflags & PSTF_NOVDISTC != 0);

    // One strat tick: chase toward center (two rept steps), then space strat.
    let done = player_into_cock_strat(&mut g, p);
    assert!(!done);
    // outdist 8 → 4 → 2 across the two rept chases.
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 2);
    // worldx was chased 40→20→10 before space strat; space strat may move further.
    assert!(g.objs.aliens[p as usize].worldx.abs() <= 20);
}

#[test]
fn into_cock2_places_at_inviewdist() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.set_sv_i16(sv::PVIEWPOSZ, 1000);
    player_into_cock2_init(&mut g, p);
    assert_eq!(g.objs.aliens[p as usize].worldz, 1060);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 20);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 0);
}

#[test]
fn registered_into_cock_handoff_releases_control() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.internal_playpt = p as i16;
    g.vars.set_sv_i16(sv::OUTDIST, 0);

    set_player_into_cock(&mut g, p);
    let into = g.objs.aliens[p as usize]
        .stratptr
        .expect("into-cock callback");
    g.call_strat(into, p);
    let phase2 = g.objs.aliens[p as usize]
        .stratptr
        .expect("phase-2 callback");
    assert_ne!(phase2, into);

    for _ in 0..21 {
        let tick = g.objs.aliens[p as usize].stratptr.expect("player callback");
        g.call_strat(tick, p);
    }

    assert_eq!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);
    assert_eq!(g.vars.player_view_mode, PlayerViewMode::Cockpit);
    assert_ne!(g.objs.aliens[p as usize].stratptr, Some(phase2));
}

#[test]
fn out_of_cock_init_and_countdown() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.player_view_mode = PlayerViewMode::Cockpit;

    set_player_out_of_cock(&mut g, p);
    assert_eq!(g.vars.viewdist, OUTVIEWDIST);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), OUTVIEWDIST);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), COCKPIT_EXIT_FRAMES - 1);
    assert_eq!(g.vars.pstratflags & PSTF_NOVDISTC, 0);
    assert_eq!(g.vars.player_view_mode, PlayerViewMode::LeavingCockpit);

    let done = player_out_of_cock_strat(&mut g, p);
    assert!(!done);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), COCKPIT_EXIT_FRAMES - 2);
}

#[test]
fn out_of_cock_spawns_retail_cockpit_mesh() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    set_player_out_of_cock(&mut g, p);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 19);

    assert!(!player_out_of_cock_strat(&mut g, p));
    assert!(g.objs.aliens.iter().any(|a| a.active && a.shape == 322));
}
