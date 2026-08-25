use sf_core::player_view::PlayerViewMode;
use sf_game::vars::{TrainingPlayerStartupPhase, CLOSE_VIEW_DISTANCE, OUTVIEWDIST, PFM_WOBBLE};
use sf_game::Game;
use sf_map::catalog::{
    map_id, opening_player_strategy, opening_player_view, OpeningPlayerStrategy,
};
use sf_strat::common::{sv, StratRam};
use sf_strat::player::{
    initialize_planet_flight, player_colony_flyin_istrat, player_divegnd_istrat,
    player_inside_space_flyin_istrat, player_move_init, player_planet_flyin_istrat,
    player_space_flyin_istrat, player_warp_out_istrat, queue_player_cred_istrat,
    queue_player_on_cont_istrat, set_player_in_ltexit, set_player_on_planet, set_player_undergnd,
    strat_player_opening_init, strat_spawn_player, strat_spawn_player_for_map,
    CONTINUE_VIEW_DISTANCE,
};

fn register_game() -> Game {
    let mut game = Game::new();
    sf_strat::table::register_all(&mut game);
    game
}

fn apply_reference_opening(
    game: &mut Game,
    player: u16,
    map: u32,
    strategy: OpeningPlayerStrategy,
) {
    match strategy {
        OpeningPlayerStrategy::HangarLaunch => strat_player_opening_init(game, player),
        OpeningPlayerStrategy::InteriorSpaceFlyIn => player_inside_space_flyin_istrat(game, player),
        OpeningPlayerStrategy::HyperspaceExit => player_warp_out_istrat(game, player),
        OpeningPlayerStrategy::PlanetFlyIn => player_planet_flyin_istrat(game, player),
        OpeningPlayerStrategy::GroundDive => player_divegnd_istrat(game, player),
        OpeningPlayerStrategy::PlanetFlight if map == map_id::TRAINING => {
            initialize_planet_flight(game, player);
            player_move_init(game, player);
            game.vars
                .set_sv_i16(sv::PVIEWPOSZ, game.objs.aliens[player as usize].worldz);
            game.vars.playerflymode &= !PFM_WOBBLE;
            game.vars.training_player_startup = TrainingPlayerStartupPhase::InitialMovement;
        }
        OpeningPlayerStrategy::PlanetFlight => set_player_on_planet(game, player),
        OpeningPlayerStrategy::SpaceFlyIn => player_space_flyin_istrat(game, player),
        OpeningPlayerStrategy::ColonyFlyIn => player_colony_flyin_istrat(game, player),
        OpeningPlayerStrategy::UndergroundFlight => set_player_undergnd(game, player),
        OpeningPlayerStrategy::LongTunnelExit => set_player_in_ltexit(game, player),
        OpeningPlayerStrategy::ContinuePresentation => queue_player_on_cont_istrat(game, player),
        OpeningPlayerStrategy::PassivePresentation => queue_player_cred_istrat(game, player),
    }
}

#[test]
fn every_spawned_map_applies_its_source_view_declaration() {
    for map in (map_id::M1_1..=map_id::CONTINUE).chain([map_id::CREDITS, map_id::TRAINING]) {
        let expected = opening_player_view(map).expect("catalog declaration");
        let mut game = register_game();
        let _player = strat_spawn_player_for_map(&mut game, map).expect("player spawn");

        assert_eq!(game.vars.player_view_mode, expected.mode, "map {map}");
        assert_eq!(game.vars.player_view_options, expected.options, "map {map}");
        let expected_distance = match opening_player_strategy(map).expect("catalog strategy") {
            // playercred_Istrat explicitly overrides the preceding pstrat
            // close distance with outviewdist (PSTRATS.ASM:587-588).
            OpeningPlayerStrategy::PassivePresentation => {
                game.run_strategies();
                OUTVIEWDIST
            }
            // playeroncont_strat owns its fixed presentation distance.
            OpeningPlayerStrategy::ContinuePresentation => {
                game.run_strategies();
                CONTINUE_VIEW_DISTANCE
            }
            _ => match expected.mode {
                PlayerViewMode::CloseExterior => CLOSE_VIEW_DISTANCE,
                PlayerViewMode::Exterior => OUTVIEWDIST,
                other => panic!("opening map {map} has transition-only mode {other:?}"),
            },
        };
        assert_eq!(game.vars.viewdist, expected_distance, "map {map}");
    }
}

#[test]
fn every_spawned_map_installs_its_authored_player_initializer() {
    for map in (map_id::M1_1..=map_id::CONTINUE).chain([map_id::CREDITS, map_id::TRAINING]) {
        let strategy = opening_player_strategy(map).expect("catalog strategy");

        let mut actual = register_game();
        let actual_player = strat_spawn_player_for_map(&mut actual, map).expect("player spawn");

        let mut expected = register_game();
        let expected_player = strat_spawn_player(&mut expected).expect("reference player spawn");
        let view = opening_player_view(map).expect("catalog view");
        expected.vars.player_view_mode = view.mode;
        expected.vars.player_view_options = view.options;
        expected.apply_player_view_mode(expected_player);
        apply_reference_opening(&mut expected, expected_player, map, strategy);

        assert_eq!(actual_player, expected_player, "map {map}");
        assert_eq!(
            actual.objs.aliens, expected.objs.aliens,
            "object initialization differs for map {map} ({strategy:?})"
        );
        assert_eq!(
            actual.objs.active_head, expected.objs.active_head,
            "map {map}"
        );
        assert_eq!(actual.objs.free_head, expected.objs.free_head, "map {map}");
        assert_eq!(
            actual.vars.pshipflags, expected.vars.pshipflags,
            "map {map}"
        );
        assert_eq!(
            actual.vars.pshipflags2, expected.vars.pshipflags2,
            "map {map}"
        );
        assert_eq!(
            actual.vars.pshipflags3, expected.vars.pshipflags3,
            "map {map}"
        );
        assert_eq!(
            actual.vars.pstratflags, expected.vars.pstratflags,
            "map {map}"
        );
        assert_eq!(
            actual.vars.playerflymode, expected.vars.playerflymode,
            "map {map}"
        );
        assert_eq!(
            actual.vars.training_player_startup, expected.vars.training_player_startup,
            "map {map}"
        );
        assert_eq!(actual.vars.gameflags, expected.vars.gameflags, "map {map}");
        assert_eq!(actual.vars.game_mode, expected.vars.game_mode, "map {map}");
        assert_eq!(actual.vars.viewdist, expected.vars.viewdist, "map {map}");
        assert_eq!(
            actual.vars.sv_i16(sv::OUTDIST),
            expected.vars.sv_i16(sv::OUTDIST),
            "map {map}"
        );
        assert_eq!(
            actual.vars.sv_i16(sv::VIEWCY),
            expected.vars.sv_i16(sv::VIEWCY),
            "map {map}"
        );
        assert_eq!(
            actual.world.lastplayz, expected.world.lastplayz,
            "map {map}"
        );
    }
}
