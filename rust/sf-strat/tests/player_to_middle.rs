//! ROM `playertomiddle1/4_srou_l` + `set_playertoCslow_l` (PCSTRATS.ASM).

use sf_game::vars::{OUTVIEWDIST, PSF_NOCTRL, PSF_NOFIRE};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_sv as sv, player_to_middle1, player_to_middle4, set_player_to_cslow,
};

#[test]
fn middle1_halves_offset_toward_viewcy() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.set_sv_i16(sv::VIEWCY, -60);
    g.objs.aliens[p as usize].worldx = 100;
    g.objs.aliens[p as usize].worldy = -20; // toward -60: delta -40 → adiv2 → -20 step

    player_to_middle1(&mut g, p);
    // shift 1: current + (target-current)>>1
    assert_eq!(g.objs.aliens[p as usize].worldx, 50);
    assert_eq!(g.objs.aliens[p as usize].worldy, -40);
}

#[test]
fn middle4_quarter_step() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.set_sv_i16(sv::VIEWCY, 0);
    g.objs.aliens[p as usize].worldx = 80;
    g.objs.aliens[p as usize].worldy = 80;

    player_to_middle4(&mut g, p);
    // >>4 of -80 = -5 → 80-5=75? Wait: current + (0-80)>>4 = 80 + (-5) = 75
    assert_eq!(g.objs.aliens[p as usize].worldx, 75);
    assert_eq!(g.objs.aliens[p as usize].worldy, 75);
}

#[test]
fn to_cslow_disables_ctrl_and_sets_viewdist() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.set_sv_i16(sv::VIEWCY, 0);
    g.objs.aliens[p as usize].worldx = 16;
    g.objs.aliens[p as usize].worldy = 16;

    set_player_to_cslow(&mut g, p);
    assert!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE) == PSF_NOCTRL | PSF_NOFIRE);
    assert_eq!(g.vars.viewdist, OUTVIEWDIST);
    // one middle4 step from 16 toward 0: 16 + (0-16)>>4 = 16-1 = 15
    assert_eq!(g.objs.aliens[p as usize].worldx, 15);
}
