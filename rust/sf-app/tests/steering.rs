//! Windowless steering-direction test. Drives the game logic (no wgpu/window)
//! through the opening into on-planet flight, holds LEFT, and checks which way
//! the player's `worldx` moves. From the view/projection matrices, +worldx
//! projects to screen-RIGHT, so a correct LEFT press must DECREASE worldx.

use sf_core::pad;
use sf_game::shell::{GameState, Shell};
use sf_game::vars::PSF_NOCTRL;

/// Read the canonical player slot. Shape id 2 is not unique once the opening
/// spawns companion/collision objects, so scanning the active list can select
/// a stationary non-player Arwing.
fn player_worldx(shell: &Shell) -> Option<i16> {
    let idx = shell.game.vars.internal_playpt;
    if idx < 0 {
        return None;
    }
    let player = &shell.game.objs.aliens[idx as usize];
    player.active.then_some(player.worldx)
}

fn make_shell() -> Shell {
    let mut shell = Shell::new();
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    shell.set_spawn_player(Box::new(|game, newmap| {
        let _ = sf_strat::player::strat_spawn_player_for_map(game, newmap);
    }));
    shell
}

/// Enter Corneria without sending gameplay START presses (which now toggle the
/// faithful pause latch), then wait for the hangar sequence to return control.
fn drive_to_controllable() -> Shell {
    let mut shell = make_shell();
    for _ in 0..4 {
        if shell.state() == GameState::Title {
            break;
        }
        shell.tick(0);
    }
    shell.tick(pad::START); // Title -> PlanetSelect
    shell.tick(0); // release START so the next press is an edge
    shell.tick(pad::START); // PlanetSelect -> Playing

    for _ in 0..700 {
        if shell.state() == GameState::Playing && shell.game.vars.pshipflags & PSF_NOCTRL == 0 {
            break;
        }
        shell.tick(0);
    }
    assert_eq!(shell.state(), GameState::Playing);
    assert_eq!(shell.game.vars.pshipflags & PSF_NOCTRL, 0);
    shell
}

#[test]
fn left_input_moves_ship_screen_left() {
    let mut shell = drive_to_controllable();
    let x0 = player_worldx(&shell).expect("player Arwing should exist in gameplay");

    // Hold LEFT.
    for _ in 0..15 {
        shell.tick(pad::LEFT);
    }
    let x_left = player_worldx(&shell).expect("player should still exist after LEFT");

    // Hold RIGHT from the same baseline for comparison.
    let mut shell2 = drive_to_controllable();
    let x0b = player_worldx(&shell2).unwrap();
    for _ in 0..15 {
        shell2.tick(pad::RIGHT);
    }
    let x_right = player_worldx(&shell2).unwrap();

    let dl = x_left as i32 - x0 as i32;
    let dr = x_right as i32 - x0b as i32;
    eprintln!("STEER-TEST  LEFT: {x0}->{x_left} (Δ {dl})   RIGHT: {x0b}->{x_right} (Δ {dr})");

    // +worldx = screen-RIGHT, so LEFT must decrease and RIGHT must increase.
    assert!(
        dl < 0,
        "LEFT should move worldx negative (screen-left); Δ={dl} — inverted"
    );
    assert!(
        dr > 0,
        "RIGHT should move worldx positive (screen-right); Δ={dr} — inverted"
    );
}
