//! Fast native replay gate for the controller-only Corneria oracle route.

use sf_game::shell::{GameState, GameplayEntryPhase, Shell};
use sf_game::vars::{GF_PLAYERDEAD, GF_PLAYERDYING, PSF2_PLAYERHP0};
use sf_oracle::sf1_input::{
    corneria_attack_carrier_input, corneria_front_end_input, CORNERIA_ATTACK_CARRIER_FRAME,
    CORNERIA_ATTACK_CARRIER_SHAPE,
};

const REPLAY_TICK_BUDGET: u32 = 4_000;
const EXPECTED_BODY_DURABILITY: u8 = 1;

fn configured_shell() -> Shell {
    let mut shell = Shell::new();
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    shell.set_spawn_player(Box::new(sf_strat::player::strat_spawn_player));
    shell.set_advance_startup_player(Box::new(
        sf_strat::player::advance_player_during_level_initialization,
    ));
    shell.set_initialize_player(Box::new(sf_strat::player::initialize_player_for_map));
    shell.set_prepare_presentation_player(Box::new(sf_strat::player::prepare_presentation_player));
    shell.set_shape_extents(sf_render::shapes::sf1_shape_half_extents());
    shell
}

#[test]
fn controller_only_route_reaches_corneria_attack_carrier() {
    let mut shell = configured_shell();
    let probed_frame = std::env::var("SF1_CORNERIA_ROUTE_STATE_FRAME")
        .ok()
        .map(|value| {
            value
                .parse::<u16>()
                .expect("route state frame must be decimal")
        });
    for tick in 0..REPLAY_TICK_BUDGET {
        let active = shell.state() == GameState::Playing
            && shell.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        let input = if active {
            corneria_attack_carrier_input(shell.game.vars.gameframe)
        } else {
            corneria_front_end_input(tick)
        };
        shell.tick(input);
        if probed_frame == Some(shell.game.vars.gameframe) {
            let player = shell.game.vars.internal_playpt as usize;
            eprintln!(
                "corneria_route_state frame={} input={} player={:?} strategy={:?}",
                shell.game.vars.gameframe,
                input,
                shell.game.objs.aliens[player],
                shell.game.vars.strategy,
            );
        }
        assert_eq!(
            shell.game.vars.gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD),
            0,
            "controller tape lost the player at level frame {}",
            shell.game.vars.gameframe,
        );
        assert_eq!(
            shell.game.vars.pshipflags2 & PSF2_PLAYERHP0,
            0,
            "controller tape depleted body durability at level frame {}",
            shell.game.vars.gameframe,
        );
        if shell
            .game
            .objs
            .aliens
            .iter()
            .any(|object| object.active && object.shape == CORNERIA_ATTACK_CARRIER_SHAPE)
        {
            assert_eq!(shell.game.vars.gameframe, CORNERIA_ATTACK_CARRIER_FRAME);
            let body = shell.game.coldet.pcbox.body.expect("player body proxy");
            assert_eq!(
                shell.game.objs.aliens[body as usize].hp,
                EXPECTED_BODY_DURABILITY,
            );
            return;
        }
    }
    panic!("controller tape did not reach the Corneria Attack Carrier");
}
