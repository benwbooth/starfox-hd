//! Production strategy wiring: anchor after movement, then shared viewmove.

use sf_core::player_view::PlayerViewMode;
use sf_game::Game;
use sf_strat::player::{
    player_in_space_strat, player_on_water_strat, player_undergnd_strat, set_player_in_space,
    set_player_on_water, set_player_undergnd, strat_player, strat_spawn_player,
};

const NO_VIEW_MOVE: u8 = 4;
type Strategy = fn(&mut Game, u16);

fn check_strategy(setup: Strategy, tick: Strategy, space: bool, underground: bool) {
    for mode in [
        PlayerViewMode::Exterior,
        PlayerViewMode::CloseExterior,
        PlayerViewMode::EnteringCockpit,
        PlayerViewMode::Cockpit,
        PlayerViewMode::LeavingCockpit,
    ] {
        for locked in [false, true] {
            let mut game = Game::new();
            sf_strat::table::register_all(&mut game);
            let player = strat_spawn_player(&mut game).unwrap();
            setup(&mut game, player);
            game.vars.player_view_mode = mode;
            game.vars.strategy.view_center_y = 97;
            game.vars.strategy.player_view_position = [777, 888, 999];
            game.vars.player_snd_flag = 0;
            if locked {
                game.vars.pstratflags |= NO_VIEW_MOVE;
            }
            let actor = &mut game.objs.aliens[player as usize];
            actor.worldx = -123;
            actor.worldy = -143;
            actor.worldz = 456;
            actor.hp = 100;
            tick(&mut game, player);
            let actor = &game.objs.aliens[player as usize];
            let [x, y] = [actor.worldx, actor.worldy];
            let expected = if mode == PlayerViewMode::Cockpit && (space || !locked) {
                [x, y]
            } else if space {
                let dy = y.wrapping_add(60);
                [
                    (x >> 1) + (x >> 2),
                    ((dy >> 1) + (dy >> 3)).wrapping_sub(60),
                ]
            } else {
                let cy = game.vars.strategy.view_center_y;
                let dy = y.wrapping_sub(cy);
                [
                    (x >> 1) + (x >> 2) + (x >> 3),
                    if underground {
                        cy
                    } else {
                        ((dy >> 1) + (dy >> 2)).wrapping_add(cy)
                    },
                ]
            };
            assert_eq!(
                &game.vars.strategy.player_view_position[..2],
                &expected,
                "{mode:?} locked={locked}"
            );
            assert_ne!(
                game.vars.player_snd_flag, 0,
                "viewmove must still publish engine sound"
            );
            if locked {
                assert_eq!(game.vars.strategy.player_view_position[2], 999);
            } else if mode == PlayerViewMode::Cockpit {
                assert_eq!(game.vars.strategy.player_view_position[2], actor.worldz);
            }
        }
    }
}

#[test]
fn space_strategy_uses_source_anchor_before_shared_depth_chase() {
    check_strategy(set_player_in_space, player_in_space_strat, true, false);
}

#[test]
fn water_strategy_uses_active_source_surface_tail() {
    check_strategy(set_player_on_water, player_on_water_strat, false, false);
}

#[test]
fn underground_strategy_keeps_live_vertical_center() {
    check_strategy(set_player_undergnd, player_undergnd_strat, false, true);
}

#[test]
fn generic_player_strategy_uses_the_same_source_space_and_surface_tails() {
    check_strategy(set_player_in_space, strat_player, true, false);
    check_strategy(set_player_on_water, strat_player, false, false);
}
