//! ROM water / bridge / undergnd / space SET_PLAYER* (PSTRATS.ASM).

use sf_game::alien::ASF_SHADOW;
use sf_game::vars::{GF_VIEWROT, PFM_WOBBLE, PSF3_INTUNNEL, PSF_NOCTRL, PSF_NOFIRE};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_sv as sv, set_player_escape_nucleus, set_player_in_space, set_player_on_bridge,
    set_player_on_water, set_player_turn180, set_player_undergnd,
};

const PFM_WATER: u8 = 4;
const PFM_DIEYROT: u8 = 2;

#[test]
fn water_fly_mode() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.pshipflags = PSF_NOCTRL | PSF_NOFIRE;

    set_player_on_water(&mut g, p);
    assert_eq!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);
    assert_eq!(g.vars.sv_i16(sv::VIEWCY), -50);
    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -500);
    assert!(g.vars.playerflymode & PFM_WATER != 0);
    assert!(g.vars.gameflags & GF_VIEWROT != 0);
    assert_eq!(g.vars.pshipflags3 & PSF3_INTUNNEL, 0);
}

#[test]
fn bridge_fly_mode_missb() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    set_player_on_bridge(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -200);
    assert_eq!(g.vars.sv_i16(sv::MISSBTOPLEFT), -90);
    assert_eq!(g.vars.sv_i16(sv::MISSBTOPRIGHT), 90);
    assert!(g.vars.playerflymode & PFM_WATER != 0);
    assert!(g.objs.aliens[p as usize].sflags & ASF_SHADOW != 0);
}

#[test]
fn undergnd_and_space() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");

    set_player_undergnd(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -500);
    assert!(g.vars.playerflymode & PFM_WOBBLE != 0);
    assert!(g.objs.aliens[p as usize].sflags & ASF_SHADOW != 0);

    set_player_in_space(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::MINPMOVEX), -240);
    assert_eq!(g.vars.sv_i16(sv::MAXPMOVEX), 240);
    assert!(g.vars.playerflymode & PFM_DIEYROT != 0);
    assert!(g.vars.playerflymode & PFM_WOBBLE != 0);
    assert_ne!(
        g.objs.aliens[p as usize].sflags & ASF_SHADOW,
        0,
        "space macro preserves the source-untouched shadow state"
    );
    assert!(g.vars.gameflags & GF_VIEWROT != 0);
}

#[test]
fn turn180_and_escape_nucleus_arm() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    set_player_turn180(&mut g, p);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 74);

    // escape nucleus needs a live player object; just ensure it runs.
    set_player_escape_nucleus(&mut g, p);
    assert!(g.vars.pstratflags & sf_game::vars::PSTF_INSEQ != 0);
}
