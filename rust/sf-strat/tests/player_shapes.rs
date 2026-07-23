//! ROM `select_ship` / `setYplayershape_l` (GSTRATS.ASM:178-246).

use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::player_sv as sv;
use sf_strat::player::{
    select_ship, set_y_player_shape, strat_spawn_player, PSHIPNUM_NORM, PSHIPNUM_NULL,
    PSHIPNUM_WIRE, PSHIPNUM_ZOOM,
};

const PSF_BRKLWING: u8 = 8;
const PSF_BRKRWING: u8 = 16;
const SHAPE_ARWING: u16 = 2;

#[test]
fn select_ship_norm_fills_four_slots() {
    let mut g = Game::new();
    select_ship(&mut g, PSHIPNUM_NORM);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPE), SHAPE_ARWING);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPEL), 368);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPER), 369);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPELR), 370);
}

#[test]
fn select_ship_null_clears_slots() {
    let mut g = Game::new();
    select_ship(&mut g, PSHIPNUM_NORM);
    select_ship(&mut g, PSHIPNUM_NULL);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPE), 0);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPEL), 0);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPER), 0);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPELR), 0);
}

#[test]
fn select_ship_clamps_out_of_range_to_norm() {
    let mut g = Game::new();
    select_ship(&mut g, 99);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPE), SHAPE_ARWING);
}

#[test]
fn set_y_player_shape_picks_by_wing_damage() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");

    // Intact.
    g.vars.pshipflags = 0;
    set_y_player_shape(&mut g, idx, PSHIPNUM_NORM);
    assert_eq!(g.objs.aliens[idx as usize].shape, SHAPE_ARWING);

    // Left wing broken → the remaining-right wireframe mesh.
    g.vars.pshipflags = PSF_BRKLWING;
    set_y_player_shape(&mut g, idx, PSHIPNUM_WIRE);
    assert_eq!(g.objs.aliens[idx as usize].shape, 352);

    // Both wings → playershapeLR.
    g.vars.pshipflags = PSF_BRKLWING | PSF_BRKRWING;
    set_y_player_shape(&mut g, idx, PSHIPNUM_ZOOM);
    assert_eq!(g.objs.aliens[idx as usize].shape, 379);

    // Null ship + intact → invisible.
    g.vars.pshipflags = 0;
    set_y_player_shape(&mut g, idx, PSHIPNUM_NULL);
    assert_eq!(g.objs.aliens[idx as usize].shape, 0);
}

#[test]
fn select_ship_uses_retail_cockpit_black_and_zoom_rows() {
    let mut g = Game::new();

    select_ship(&mut g, 3);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPE), 371);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPELR), 371);

    select_ship(&mut g, 5);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPE), 372);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPEL), 373);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPER), 374);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPELR), 375);

    select_ship(&mut g, 6);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPE), 376);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPEL), 377);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPER), 378);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPELR), 379);
}

#[test]
fn stage_spawn_preserves_inventory_upgrade_and_wing_damage() {
    const DOUBLE_LASER: u8 = 1;

    let mut game = Game::new();
    game.vars.strategy.special_weapon_count = 1;
    game.vars.pshipflags = PSF_BRKLWING;
    game.vars.pshipflags2 = DOUBLE_LASER;

    let player = strat_spawn_player(&mut game).expect("player slot");

    assert_eq!(game.vars.strategy.special_weapon_count, 1);
    assert_eq!(game.vars.pshipflags2 & DOUBLE_LASER, DOUBLE_LASER);
    assert_eq!(game.objs.aliens[player as usize].shape, 368);
}
