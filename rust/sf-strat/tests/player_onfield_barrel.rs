//! Tick 124: playeronfield view + barrel-roll s_beqdec window.

use sf_core::pad;
use sf_game::alien::ASF_SHADOW;
use sf_game::vars::{GF_VIEWROT, PFM_WOBBLE, PSF_NOCTRL, PSF_NOFIRE};
use sf_game::Game;
use sf_strat::common::{strat_perc87, sv, StratRam};
use sf_strat::player::{
    player_on_field_strat, player_sv, set_player_on_field, strat_player, strat_spawn_player,
};

fn set_pad(g: &mut Game, pad: u16) {
    let prev = g.vars.pad1;
    g.vars.lastcont0 = (prev >> 8) as u8;
    g.vars.lastcontl0 = (prev & 0xFF) as u8;
    g.vars.pad1 = pad;
}

fn ready_player(g: &mut Game) -> u16 {
    let idx = strat_spawn_player(g).expect("player");
    g.vars.set_sv_u8(sv::STAYBLACK, (-1i8) as u8);
    g.vars.set_sv_u8(sv::DOINGWIPE, 0);
    g.vars.set_sv_u8(sv::PMOVELIMITAND, 0xFF);
    idx
}

#[test]
fn onfield_fly_mode_and_view() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.pshipflags = PSF_NOCTRL | PSF_NOFIRE;
    g.vars.gameflags |= GF_VIEWROT;

    set_player_on_field(&mut g, p);
    assert_eq!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);
    assert_eq!(g.vars.sv_i16(player_sv::VIEWCY), -60);
    assert_eq!(g.vars.sv_i16(player_sv::MINPMOVEX), -500);
    assert_eq!(g.vars.sv_i16(player_sv::MAXPMOVEX), 500);
    assert!(g.vars.playerflymode & PFM_WOBBLE != 0);
    assert_eq!(g.vars.gameflags & GF_VIEWROT, 0, "field clears gf_viewrot");
    assert!(g.objs.aliens[p as usize].sflags & ASF_SHADOW != 0);

    // View: perc87 X, fixed ViewCY Y (not player Y).
    g.objs.aliens[p as usize].worldx = 100;
    g.objs.aliens[p as usize].worldy = -40;
    player_on_field_strat(&mut g, p);
    assert_eq!(g.vars.sv_i16(player_sv::PVIEWPOSX), strat_perc87(100));
    assert_eq!(g.vars.sv_i16(player_sv::PVIEWPOSY), -60);
}

#[test]
fn barrel_roll_starts_while_delay_window_open() {
    // ROM: s_beqdec branches to .lragain when delay==0 (no start); start path
    // runs only when delay>0. Fresh shoulder on the frame after a hold+release
    // while delay still >0 must start the roll (audit Minor #9).
    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.set_sv_u8(sv::PLAYER_ROLLZVEL, 0);
    g.vars.set_sv_u8(sv::PLAYER_ROLLDELAY, 0);

    // Hold shoulder → reload delay to barrelrolldelay (3).
    set_pad(&mut g, pad::TRIGHT);
    strat_player(&mut g, idx);
    assert_eq!(g.vars.sv_u8(sv::PLAYER_ROLLDELAY), 3);
    assert_eq!(
        g.vars.sv_u8(sv::PLAYER_ROLLZVEL) as i8,
        0,
        "hold alone does not start"
    );

    // Release: delay ticks down (beqdec), no start.
    set_pad(&mut g, 0);
    strat_player(&mut g, idx);
    let delay_after_release = g.vars.sv_u8(sv::PLAYER_ROLLDELAY);
    assert!(
        delay_after_release > 0 && delay_after_release < 3,
        "window still open after one release frame, got {delay_after_release}"
    );

    // Fresh press while window open → start (ROM polarity: right → −32).
    set_pad(&mut g, pad::TRIGHT);
    strat_player(&mut g, idx);
    assert_eq!(
        g.vars.sv_u8(sv::PLAYER_ROLLZVEL) as i8,
        -32,
        "double-tap within window must start roll"
    );
}

#[test]
fn barrel_roll_no_start_when_delay_already_zero() {
    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.set_sv_u8(sv::PLAYER_ROLLZVEL, 0);
    g.vars.set_sv_u8(sv::PLAYER_ROLLDELAY, 0);

    // Fresh shoulder with delay==0 → .lragain only (reload), no roll start.
    set_pad(&mut g, pad::TLEFT);
    strat_player(&mut g, idx);
    assert_eq!(g.vars.sv_u8(sv::PLAYER_ROLLZVEL) as i8, 0);
    assert_eq!(g.vars.sv_u8(sv::PLAYER_ROLLDELAY), 3);
}

#[test]
fn barrel_roll_tleft_polarity_positive() {
    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.set_sv_u8(sv::PLAYER_ROLLZVEL, 0);
    g.vars.set_sv_u8(sv::PLAYER_ROLLDELAY, 2); // window open

    set_pad(&mut g, 0);
    set_pad(&mut g, pad::TLEFT); // fresh edge
    strat_player(&mut g, idx);
    assert_eq!(g.vars.sv_u8(sv::PLAYER_ROLLZVEL) as i8, 32);
}
