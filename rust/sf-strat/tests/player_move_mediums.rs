//! Tick 123: player-move Mediums — outdist ease, Y-bounds, BOOSTOBJ, wobble.

use sf_core::pad;
use sf_game::vars::PFM_WOBBLE;
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::player::{strat_player, strat_spawn_player};

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
fn outdist_chases_viewdist_unless_novdistc() {
    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.viewdist = 200;
    g.vars.set_sv_i16(sv::OUTDIST, 0);
    g.vars.pstratflags &= !sf_game::vars::PSTF_NOVDISTC;

    set_pad(&mut g, 0);
    strat_player(&mut g, idx);
    let od1 = g.vars.sv_i16(sv::OUTDIST);
    assert!(
        od1 > 0 && od1 < 200,
        "rate-3 chase should step toward 200, got {od1}"
    );

    // Gate: PSTF_NOVDISTC freezes outdist.
    let frozen = od1;
    g.vars.pstratflags |= sf_game::vars::PSTF_NOVDISTC;
    strat_player(&mut g, idx);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), frozen);
}

#[test]
fn y_bounds_are_inclusive_and_body_collision_owns_the_floor_when_enabled() {
    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.minpmove_y = -100;
    g.vars.set_sv_i16(sv::MAXPMOVEY, 50);

    // Inclusive top: worldy == miny must clamp (was exclusive `<`).
    g.objs.aliens[idx as usize].worldy = -100;
    set_pad(&mut g, 0);
    strat_player(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -100);

    // With the detailed body-bottom collision lane disabled, the ordinary
    // lower-screen clamp owns the boundary and is inclusive.
    g.vars.set_sv_u8(sv::PMOVELIMITAND, 0);
    g.objs.aliens[idx as usize].worldy = 50;
    strat_player(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, 50);

    // When the body-bottom lane is enabled, detailed collision owns the floor,
    // so this ordinary limit path must leave the position alone.
    const BODY_BOTTOM_COLLISION: u8 = 128;
    g.vars.set_sv_u8(sv::PMOVELIMITAND, BODY_BOTTOM_COLLISION);
    g.objs.aliens[idx as usize].worldy = 80;
    strat_player(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].worldy, 80,
        "body-bottom collision lane must bypass the ordinary floor clamp"
    );

    // Controller-demo bounds deliberately cross after the lower clamp:
    // retail applies lower first, then upper, and therefore settles at min.
    g.vars.minpmove_y = -50;
    g.vars.set_sv_i16(sv::MAXPMOVEY, -70);
    g.vars.set_sv_u8(sv::PMOVELIMITAND, 0);
    g.objs.aliens[idx as usize].worldy = 0;
    strat_player(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -50);
}

#[test]
fn pad_x_boost_tags_boostobj() {
    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.set_sv_i16(sv::BOOSTOBJ, -1);
    g.objs.aliens[idx as usize].sbyte2 = 0;
    set_pad(&mut g, pad::X);
    strat_player(&mut g, idx);
    assert_eq!(g.vars.sv_i16(sv::BOOSTOBJ), idx as i16);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 20);
}

#[test]
fn wobble_halves_the_intact_wing_sample_toward_zero() {
    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.playerflymode |= PFM_WOBBLE;
    g.vars.set_sv_u8(sv::PLAYER_ZROTFLOATPTR, 1); // table[1]=1 → intact negates to -1
    g.vars.pshipflags &= !(0x08 | 0x10); // clear broken wings
    set_pad(&mut g, 0);
    let rotz0 = g.objs.aliens[idx as usize].rotz;
    strat_player(&mut g, idx);
    let rotz1 = g.objs.aliens[idx as usize].rotz;
    // Intact wings halve the signed source sample toward zero, so table[1]
    // contributes zero. The oscillator cursor still advances.
    assert_eq!(g.vars.sv_u8(sv::PLAYER_ZROTFLOATPTR), 2);
    assert_eq!(rotz1, rotz0);
}
