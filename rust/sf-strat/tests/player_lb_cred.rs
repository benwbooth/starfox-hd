//! ROM LB-out / dive / cred / tunnel→planet SET_PLAYER* leaves.

use sf_game::alien::ASF_INVISIBLE;
use sf_game::vars::{
    GF_NOZREMOVE, GF_STRATDONE1, OUTVIEWDIST, PFM_WOBBLE, PSF_NOCTRL, PSF_NOFIRE, PSTF_INSEQ,
};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_out_of_lb2_strat, player_out_of_lb3_strat, player_sv as sv, set_player_cred,
    set_player_dive_gnd, set_player_into_lb1, set_player_out_of_lb1, set_player_out_of_lb2,
    set_player_out_of_lb2a, set_player_out_of_lb3, set_player_tunnel_to_on_planet, view_lb3_move,
};

#[test]
fn out_of_lb1_arms_ltunnel_seq() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.gameflags |= GF_STRATDONE1;
    g.vars.playerflymode |= PFM_WOBBLE;

    set_player_out_of_lb1(&mut g, p);
    assert!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE) != 0);
    assert!(g.vars.pstratflags & PSTF_INSEQ != 0);
    assert_eq!(g.vars.gameflags & GF_STRATDONE1, 0);
    assert_eq!(g.vars.playerflymode & PFM_WOBBLE, 0);
    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -120); // Ltunnel
}

#[test]
fn out_of_lb2_and_lb3() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.objs.aliens[p as usize].worldz = 100;

    set_player_out_of_lb2(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::OUTVX), -(64 * 256));
    assert!(g.objs.aliens[p as usize].sflags & ASF_INVISIBLE != 0);
    assert!(g.vars.gameflags & GF_NOZREMOVE != 0);
    assert_eq!(g.vars.sv_i16(sv::BG2YSCROLL), 200);

    player_out_of_lb2_strat(&mut g, p);
    assert_eq!(g.objs.aliens[p as usize].worldz, 165);

    set_player_out_of_lb3(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::BG2YSCROLL), 292);
    player_out_of_lb3_strat(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::BG2YSCROLL), 291);

    g.vars.set_sv_i16(sv::BG2YSCROLL, 172);
    view_lb3_move(&mut g);
    assert_eq!(g.vars.sv_i16(sv::BG2YSCROLL), 172); // already at stop
}

#[test]
fn lb2a_boost_and_cred_and_tunnel() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");

    set_player_out_of_lb2a(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::BOOSTOBJ), p as i16);
    assert_eq!(g.objs.aliens[p as usize].vel, 85);

    set_player_cred(&mut g, p);
    assert_eq!(g.vars.viewdist, OUTVIEWDIST);
    assert!(g.objs.aliens[p as usize].sflags & ASF_INVISIBLE != 0);
    assert_eq!(g.world.lastplayz, 0);

    set_player_tunnel_to_on_planet(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::VIEWCY), -60);

    set_player_into_lb1(&mut g, p);
    assert!(g.vars.pshipflags & PSF_NOCTRL != 0);

    g.vars.set_sv_i8(sv::STAYBLACK, 5);
    set_player_dive_gnd(&mut g, p);
    assert!(g.vars.pshipflags & PSF_NOCTRL != 0);
}
