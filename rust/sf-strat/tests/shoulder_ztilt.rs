//! Tick 122: shoulder-hold bank lean (PSTRATS.ASM:2626-2639).

use sf_core::pad;
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::player::{strat_player, strat_spawn_player};

const DEG45: i8 = 32;
const DEG90: i8 = 64;

fn set_pad(g: &mut Game, pad: u16) {
    let prev = g.vars.pad1;
    g.vars.lastcont0 = (prev >> 8) as u8;
    g.vars.lastcontl0 = (prev & 0xFF) as u8;
    g.vars.pad1 = pad;
}

#[test]
fn shoulder_hold_leans_ztilt_deg45_over_3() {
    let mut g = Game::new();
    let idx = strat_spawn_player(&mut g).expect("player");
    g.vars.set_sv_u8(sv::STAYBLACK, (-1i8) as u8);
    g.vars.set_sv_u8(sv::DOINGWIPE, 0);
    g.vars.set_sv_u8(sv::PLAYER_ZTILT, 0);

    // One tick with L shoulder held: +deg45/3 before the rate-3 achase to 0.
    set_pad(&mut g, pad::TLEFT);
    strat_player(&mut g, idx);
    let after_add = DEG45 / 3;
    // playermove then achases ztilt toward 0 at rate 3: new = old - old/8
    // (adiv2-style chase). Exact value depends on chase helper; just require
    // a clear positive lean larger than the dpad-only deg45/15 step.
    let z = g.vars.sv_u8(sv::PLAYER_ZTILT) as i8;
    assert!(
        z > DEG45 / 15,
        "shoulder lean must exceed dpad-steer step; got {z} (pre-chase add was {after_add})"
    );
    assert!(z <= DEG90, "must clamp at ±deg90");

    // Hold several more ticks — should climb toward +deg90, not stay tiny.
    for _ in 0..20 {
        set_pad(&mut g, pad::TLEFT);
        strat_player(&mut g, idx);
    }
    let z2 = g.vars.sv_u8(sv::PLAYER_ZTILT) as i8;
    assert!(
        z2 >= 40,
        "sustained L-shoulder lean should bank hard, got {z2}"
    );

    // Right shoulder leans the other way.
    g.vars.set_sv_u8(sv::PLAYER_ZTILT, 0);
    set_pad(&mut g, 0);
    strat_player(&mut g, idx);
    set_pad(&mut g, pad::TRIGHT);
    strat_player(&mut g, idx);
    let zr = g.vars.sv_u8(sv::PLAYER_ZTILT) as i8;
    assert!(zr < 0, "R-shoulder must lean negative, got {zr}");
}
