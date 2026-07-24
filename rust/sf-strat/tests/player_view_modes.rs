use sf_core::player_view::PlayerViewMode;
use sf_game::vars::{CLOSE_VIEW_DISTANCE, OUTVIEWDIST};
use sf_game::Game;
use sf_map::catalog::{map_id, opening_player_view};
use sf_strat::player::strat_spawn_player_for_map;

#[test]
fn every_spawned_map_applies_its_source_view_declaration() {
    for map in (map_id::M1_1..=map_id::CONTINUE).chain([map_id::CREDITS, map_id::TRAINING]) {
        let expected = opening_player_view(map).expect("catalog declaration");
        let mut game = Game::new();
        sf_strat::table::register_all(&mut game);
        let _player = strat_spawn_player_for_map(&mut game, map).expect("player spawn");

        assert_eq!(game.vars.player_view_mode, expected.mode, "map {map}");
        assert_eq!(game.vars.player_view_options, expected.options, "map {map}");
        let expected_distance = if map == map_id::CREDITS {
            // playercred_Istrat explicitly overrides the preceding pstrat
            // close distance with outviewdist (PSTRATS.ASM:587-588).
            OUTVIEWDIST
        } else {
            match expected.mode {
                PlayerViewMode::CloseExterior => CLOSE_VIEW_DISTANCE,
                PlayerViewMode::Exterior => OUTVIEWDIST,
                other => panic!("opening map {map} has transition-only mode {other:?}"),
            }
        };
        assert_eq!(game.vars.viewdist, expected_distance, "map {map}");
    }
}
