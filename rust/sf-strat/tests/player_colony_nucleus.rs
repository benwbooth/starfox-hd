//! ROM colony / nucleus SET_PLAYER* (PSTRATS.ASM).

use sf_game::alien::ASF_SHADOW;
use sf_game::vars::{GF_VIEWROT, PSF3_INTUNNEL, PSF_NOCTRL, PSF_NOFIRE};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_clear_colony_strat, player_sv as sv, set_player_clear_colony, set_player_in_colony,
    set_player_in_nucleus, set_player_washent,
};

const PSTF_FIRSTFRAMELCOL: u8 = 16;

#[test]
fn colony_fly_mode_bounds_and_macro() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.pshipflags = PSF_NOCTRL | PSF_NOFIRE;
    g.vars.gameflags |= GF_VIEWROT;

    set_player_in_colony(&mut g, p);

    assert_eq!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);
    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -170);
    assert_eq!(g.vars.sv_i16(sv::MAXPMOVEX), 120);
    assert_eq!(g.vars.sv_i16(sv::MINMMOVEX), -5000);
    assert_eq!(g.vars.sv_i16(sv::MISSBTOPLEFT), -140);
    assert!(g.vars.pstratflags & PSTF_FIRSTFRAMELCOL != 0);
    assert!(g.vars.pshipflags3 & PSF3_INTUNNEL != 0);
    assert!(g.objs.aliens[p as usize].sflags & ASF_SHADOW != 0);
    assert_eq!(g.vars.gameflags & GF_VIEWROT, 0);
    assert!(g.objs.aliens[p as usize].stratptr.is_some());
    assert!(g.objs.aliens[p as usize].collstratptr.is_some());
    assert!(g.objs.aliens[p as usize].expstratptr.is_some());
}

#[test]
fn nucleus_fly_mode_scaled_bounds() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.objs.aliens[p as usize].worldy = 100;

    set_player_in_nucleus(&mut g, p);

    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -880);
    assert_eq!(g.vars.sv_i16(sv::MAXPMOVEX), 880);
    assert_eq!(g.vars.sv_i16(sv::MINMMOVEX), -1880);
    assert_eq!(g.vars.sv_i16(sv::MAXMMOVEX), 1880);
    assert_eq!(g.objs.aliens[p as usize].worldy, -60);
    assert_eq!(g.vars.pshipflags3 & PSF3_INTUNNEL, 0);
    assert!(g.objs.aliens[p as usize].sflags & ASF_SHADOW != 0);
    assert!(g.objs.aliens[p as usize].stratptr.is_some());
    assert!(g.objs.aliens[p as usize].collstratptr.is_some());
    assert!(g.objs.aliens[p as usize].expstratptr.is_some());
}

#[test]
fn clear_colony_and_washent_dup_and_advance() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.objs.aliens[p as usize].worldz = 1000;
    g.objs.aliens[p as usize].shape = 2;

    let dup = set_player_clear_colony(&mut g, p).expect("dup");
    assert!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE) != 0);
    assert_ne!(dup, p);
    assert!(g.objs.aliens[dup as usize].active);

    player_clear_colony_strat(&mut g, p);
    assert_eq!(g.objs.aliens[p as usize].worldz, 1000 + 65);

    let p2 = g.objs.alloc().expect("slot2");
    g.objs.aliens[p2 as usize].worldz = 0;
    let _ = set_player_washent(&mut g, p2).expect("washent dup");
    player_clear_colony_strat(&mut g, p2);
    assert_eq!(g.objs.aliens[p2 as usize].worldz, 65);
}
