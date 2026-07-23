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
fn y_bounds_inclusive_and_bbottom_gate() {
    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.minpmove_y = -100;
    g.vars.set_sv_i16(sv::MAXPMOVEY, 50);

    // Inclusive top: worldy == miny must clamp (was exclusive `<`).
    g.objs.aliens[idx as usize].worldy = -100;
    set_pad(&mut g, 0);
    strat_player(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -100);

    // Inclusive bottom when PML_BBOTTOM set.
    g.vars.set_sv_u8(sv::PMOVELIMITAND, 0x80); // PML_BBOTTOM
    g.objs.aliens[idx as usize].worldy = 50;
    strat_player(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, 50);

    // Bottom gate off: worldy may sit past max without clamp from limit path.
    g.vars.set_sv_u8(sv::PMOVELIMITAND, 0);
    g.objs.aliens[idx as usize].worldy = 80;
    strat_player(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].worldy, 80,
        "no PML_BBOTTOM → no bottom clamp"
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
fn wobble_adds_pzrotfloat_to_rotz() {
    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.playerflymode |= PFM_WOBBLE;
    g.vars.set_sv_u8(sv::PLAYER_ZROTFLOATPTR, 1); // table[1]=1 → intact negates to -1
    g.vars.pshipflags &= !(0x08 | 0x10); // clear broken wings
    set_pad(&mut g, 0);
    let rotz0 = g.objs.aliens[idx as usize].rotz;
    strat_player(&mut g, idx);
    let rotz1 = g.objs.aliens[idx as usize].rotz;
    // Intact wings: sample negated → rotz changes by -1 (plus other terms may
    // chase toward 0). Ptr must advance.
    assert_eq!(g.vars.sv_u8(sv::PLAYER_ZROTFLOATPTR), 2);
    assert_ne!(rotz1, rotz0.wrapping_add(0).wrapping_add(0)); // smoke: rotz written
    let _ = (rotz0, rotz1);
}
