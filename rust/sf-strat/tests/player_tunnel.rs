//! ROM tunnel / tunnel-exit SET_PLAYER* (PSTRATS.ASM).

use sf_game::alien::ASF_SHADOW;
use sf_game::vars::{GF_VIEWROT, PSF3_INTUNNEL, PSF_NOCTRL, PSF_NOFIRE, PSTF_NOTDIE};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_sv as sv, set_player_in_ltexit, set_player_in_ltunnel, set_player_in_mtexit,
    set_player_in_mtunnel, set_player_in_stexit, set_player_in_stunnel,
};

const PSF_NOYCTRL: u8 = 128;
const PSF2_NOSPARK: u8 = 4;

#[test]
fn ltunnel_enables_ctrl_and_intunnel_bounds() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.pshipflags = PSF_NOCTRL | PSF_NOFIRE;
    g.vars.gameflags |= GF_VIEWROT;

    set_player_in_ltunnel(&mut g, p);

    assert_eq!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);
    assert_eq!(g.vars.sv_i16(sv::VIEWCY), -60);
    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -120);
    assert_eq!(g.vars.sv_i16(sv::MAXPMOVEX), 120);
    assert!(g.vars.pshipflags3 & PSF3_INTUNNEL != 0);
    assert!(g.objs.aliens[p as usize].sflags & ASF_SHADOW != 0);
    assert_eq!(g.vars.gameflags & GF_VIEWROT, 0);
    assert_eq!(g.vars.pshipflags2 & PSF2_NOSPARK, 0);
}

#[test]
fn mtunnel_and_stunnel_narrower_x() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");

    set_player_in_mtunnel(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -90);
    assert_eq!(g.vars.sv_i16(sv::MAXPMOVEX), 90);

    set_player_in_stunnel(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -60);
    assert_eq!(g.vars.sv_i16(sv::MAXPMOVEX), 60);
}

#[test]
fn texit_sets_noyctrl_clears_intunnel() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    set_player_in_ltunnel(&mut g, p);
    assert!(g.vars.pshipflags3 & PSF3_INTUNNEL != 0);

    set_player_in_stexit(&mut g, p);
    assert!(g.vars.pshipflags & PSF_NOYCTRL != 0);
    assert_eq!(g.vars.pshipflags3 & PSF3_INTUNNEL, 0);
    assert!(g.vars.gameflags & GF_VIEWROT != 0);
    assert!(g.vars.pshipflags2 & PSF2_NOSPARK != 0);
    assert!(g.vars.pstratflags & PSTF_NOTDIE != 0);
    assert_eq!(g.objs.aliens[p as usize].sflags & ASF_SHADOW, 0);
    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -35);

    set_player_in_mtexit(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -50);

    set_player_in_ltexit(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -70);
}
