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
    g.vars.game_mode = 0;
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

    // When the body-bottom lane is enabled, the predictive detailed-collision
    // path owns the floor and parks the body there before new velocity is
    // generated.
    const BODY_BOTTOM_COLLISION: u8 = 128;
    g.vars.set_sv_u8(sv::PMOVELIMITAND, BODY_BOTTOM_COLLISION);
    g.objs.aliens[idx as usize].worldy = 80;
    strat_player(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].worldy, 50,
        "detailed body-bottom collision must own the floor clamp"
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
fn upper_bound_is_checked_before_velocity_without_a_second_clamp() {
    const ANGLE_FRACTION_SCALE: i16 = 256;

    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.game_mode = 0;
    g.vars.minpmove_y = -100;
    g.vars.set_sv_i16(sv::MAXPMOVEY, 100);
    g.vars.set_sv_u8(sv::PMOVELIMITAND, 0);
    g.vars.set_sv_i16(sv::PLROTX, -8 * ANGLE_FRACTION_SCALE);
    g.vars.strategy.frame_rate = 4;
    g.objs.aliens[idx as usize].vel = 65;
    g.objs.aliens[idx as usize].worldy = -95;

    set_pad(&mut g, 0);
    strat_player(&mut g, idx);

    assert!(
        g.objs.aliens[idx as usize].worldy < g.vars.minpmove_y,
        "the source permits the new velocity to overshoot until the next strategy pass"
    );
}

#[test]
fn body_floor_predicts_the_next_step_and_levels_normal_speed_pitch() {
    const GAMEPLAY_FRAME_RATE: u8 = 6;
    const NEXT_FRAME_RATE: u8 = 7;
    const BODY_BOTTOM_COLLISION: u8 = 128;
    const FLOOR_Y: i16 = -20;
    const ANGLE_FRACTION_SCALE: i16 = 256;
    const COLLISION_PITCH: i16 = 8 * ANGLE_FRACTION_SCALE;

    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.game_mode = 0;
    g.vars.minpmove_y = -100;
    g.vars.set_sv_i16(sv::MAXPMOVEY, FLOOR_Y);
    g.vars.set_sv_u8(sv::PMOVELIMITAND, BODY_BOTTOM_COLLISION);
    g.vars.set_sv_i16(sv::PLROTX, COLLISION_PITCH);
    g.vars.strategy.frame_rate = GAMEPLAY_FRAME_RATE;
    g.vars.pviewvelz = 63;
    g.objs.aliens[idx as usize].vel = 65;
    g.vars.set_sv_u8(sv::PLAYER_MEDSPEED, 65);
    g.objs.aliens[idx as usize].worldy = -53;

    set_pad(&mut g, 0);
    strat_player(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -34);
    assert_eq!(g.objs.aliens[idx as usize].vy, 19);
    assert_eq!(g.vars.sv_i16(sv::PLROTX), 7 * ANGLE_FRACTION_SCALE);

    // The existing +19 velocity predicts an overshoot on the following tick.
    // Retail clamps to the floor and clears pitch before rebuilding vectors.
    g.vars.strategy.frame_rate = NEXT_FRAME_RATE;
    strat_player(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, FLOOR_Y);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 0);
    assert_eq!(g.objs.aliens[idx as usize].vy, 0);
    assert_eq!(g.objs.aliens[idx as usize].vz, 63);
    assert_eq!(g.vars.pviewvelz, 63);
    assert_eq!(g.vars.sv_i16(sv::PLROTX), 0);
}

#[test]
fn wing_collision_roll_uses_the_source_damped_spring() {
    const LEFT_WING_COLLISION: u8 = 2;
    const COLLISION_SHAKE: i16 = -512;
    const FIRST_REBOUND_VELOCITY: i16 = 512;
    const SECOND_REBOUND_SHAKE: i16 = 384;

    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.pshipflags |= LEFT_WING_COLLISION;
    set_pad(&mut g, 0);
    strat_player(&mut g, idx);
    assert_eq!(g.vars.strategy.player_depth_shake, COLLISION_SHAKE);
    assert_eq!(g.vars.strategy.player_depth_shake_velocity, 0);

    g.vars.pshipflags &= !LEFT_WING_COLLISION;
    strat_player(&mut g, idx);
    assert_eq!(g.vars.strategy.player_depth_shake, 0);
    assert_eq!(
        g.vars.strategy.player_depth_shake_velocity,
        FIRST_REBOUND_VELOCITY
    );

    strat_player(&mut g, idx);
    assert_eq!(g.vars.strategy.player_depth_shake, SECOND_REBOUND_SHAKE);
    assert_eq!(
        g.vars.strategy.player_depth_shake_velocity,
        FIRST_REBOUND_VELOCITY
    );
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
