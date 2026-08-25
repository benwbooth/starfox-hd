//! Windowless steering-direction test. Drives the game logic (no wgpu/window)
//! through the opening into on-planet flight, holds LEFT, and checks which way
//! the player's `worldx` moves. From the view/projection matrices, +worldx
//! projects to screen-RIGHT, so a correct LEFT press must DECREASE worldx.

use sf_core::{pad, sf1_planets::PlanetSequencePhase};
use sf_game::alien::{ASF2_COLLDISABLE, ASF4_INVISIBLE};
use sf_game::shell::{
    GameState, GameplayEntryPhase, Shell, BRIEFING_INPUT_DELAY_TICKS, INTRO_INPUT_DELAY_TICKS,
    TITLE_INPUT_DELAY_TICKS, TITLE_PRESENTATION_INPUT_READY_TICKS,
};
use sf_game::vars::{PSF3_NOCOLLISIONS, PSF_NOCTRL, PSTF_NOTDIE};
use std::collections::BTreeMap;

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
    shell.set_spawn_player(Box::new(sf_strat::player::strat_spawn_player));
    shell.set_advance_startup_player(Box::new(
        sf_strat::player::advance_player_during_level_initialization,
    ));
    shell.set_initialize_player(Box::new(sf_strat::player::initialize_player_for_map));
    shell.set_prepare_presentation_player(Box::new(sf_strat::player::prepare_presentation_player));
    shell
}

/// Enter Corneria without sending gameplay START presses (which now toggle the
/// faithful pause latch), then wait for the hangar sequence to return control.
fn drive_to_controllable() -> Shell {
    let mut shell = make_shell();
    shell.tick(0);
    shell.tick(0);
    while shell.game.vars.gameframe < INTRO_INPUT_DELAY_TICKS {
        shell.tick(pad::A);
    }
    while shell.state() != GameState::Title {
        shell.tick(0);
    }
    shell.tick(0);
    for _ in 1..TITLE_PRESENTATION_INPUT_READY_TICKS {
        shell.tick(0);
    }
    shell.game.vars.gameframe = TITLE_INPUT_DELAY_TICKS;
    shell.tick(pad::START); // Title -> controller screen
    while shell.state() == GameState::Title {
        shell.tick(0);
    }
    shell.tick(0);
    shell.game.vars.gameframe = BRIEFING_INPUT_DELAY_TICKS - 1;
    shell.tick(pad::START); // controller layout -> destination
    shell.tick(0);
    shell.tick(pad::DOWN); // select GAME
    shell.tick(0);
    shell.tick(pad::START); // controller screen -> PlanetSelect
    while shell.state() == GameState::Briefing {
        shell.tick(0);
    }
    while shell.frame().planet_presentation.phase == PlanetSequencePhase::InitialSetup {
        shell.tick(0);
    }
    shell.tick(pad::START); // confirm route

    for _ in 0..512 {
        if shell.frame().planet_presentation.phase == PlanetSequencePhase::Briefing {
            break;
        }
        shell.tick(0);
    }
    assert_eq!(
        shell.frame().planet_presentation.phase,
        PlanetSequencePhase::Briefing
    );
    shell.tick(0);
    shell.tick(pad::B); // dismiss General Pepper

    for _ in 0..900 {
        if shell.state() == GameState::Playing
            && shell.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel
            && shell.game.vars.pshipflags & PSF_NOCTRL == 0
        {
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

#[test]
fn corneria_arch_course_spawns_its_authored_twin_laser_reward() {
    const CHECKPOINT_RADIUS: i16 = 100;
    const CHECKPOINT_APPROACH_DEPTH: i32 = 180;
    const TWIN_LASER_SHAPE: u16 = 160;
    const COURSE_TICK_BUDGET: usize = 800;

    let mut shell = drive_to_controllable();
    shell.game.vars.pshipflags3 |= PSF3_NOCOLLISIONS;
    shell.game.vars.pstratflags |= PSTF_NOTDIE;

    let mut checkpoints_crossed = 0usize;
    let mut checkpoint_approaches = Vec::new();
    let mut nearest_checkpoint_depth = BTreeMap::<i16, u16>::new();
    let mut previous_counter = shell.game.vars.map.skill_fly;
    for _ in 0..COURSE_TICK_BUDGET {
        let player_index = shell.game.vars.internal_playpt as usize;
        let player_depth = shell.game.objs.aliens[player_index].worldz;
        for object in shell.game.objs.aliens.iter().filter(|object| {
            object.active
                && object.shape == 0
                && object.sword1 == CHECKPOINT_RADIUS
                && object.sflags2 & ASF2_COLLDISABLE != 0
                && object.sflags4 & ASF4_INVISIBLE != 0
        }) {
            let depth = object.worldz.wrapping_sub(player_depth).unsigned_abs();
            nearest_checkpoint_depth
                .entry(object.worldx)
                .and_modify(|nearest| *nearest = (*nearest).min(depth))
                .or_insert(depth);
        }
        let checkpoint = shell
            .game
            .objs
            .aliens
            .iter()
            .find(|object| {
                object.active
                    && object.shape == 0
                    && object.sword1 == CHECKPOINT_RADIUS
                    && object.sflags2 & ASF2_COLLDISABLE != 0
                    && object.sflags4 & ASF4_INVISIBLE != 0
                    && i32::from(object.worldz.wrapping_sub(player_depth)).abs()
                        < CHECKPOINT_APPROACH_DEPTH
            })
            .map(|object| (object.worldx, object.worldy));
        if let Some((x, y)) = checkpoint {
            if checkpoint_approaches.last().copied() != Some((x, y)) {
                checkpoint_approaches.push((x, y));
            }
            let player = &mut shell.game.objs.aliens[player_index];
            player.worldx = x;
            player.worldy = y;
            player.vx = 0;
            player.vy = 0;
        }

        shell.tick(0);
        let counter = shell.game.vars.map.skill_fly;
        if counter < previous_counter {
            checkpoints_crossed += usize::from(previous_counter - counter);
        }
        previous_counter = counter;

        if shell
            .game
            .objs
            .aliens
            .iter()
            .any(|object| object.active && object.shape == TWIN_LASER_SHAPE)
        {
            assert_eq!(counter, 0);
            assert_eq!(checkpoints_crossed, 4);
            return;
        }
    }

    let player_index = shell.game.vars.internal_playpt as usize;
    let player = shell.game.objs.aliens[player_index];
    let checkpoints = shell
        .game
        .objs
        .aliens
        .iter()
        .filter(|object| {
            object.active
                && object.shape == 0
                && object.sword1 == CHECKPOINT_RADIUS
                && object.sflags2 & ASF2_COLLDISABLE != 0
                && object.sflags4 & ASF4_INVISIBLE != 0
        })
        .map(|object| (object.worldx, object.worldy, object.worldz))
        .collect::<Vec<_>>();
    panic!(
        "Corneria arch reward did not spawn; crossed {checkpoints_crossed}, approaches={checkpoint_approaches:?}, nearest={nearest_checkpoint_depth:?}, counter={}, state={:?}, map={}, pointer={}, player=({},{},{}), checkpoints={checkpoints:?}",
        shell.game.vars.map.skill_fly,
        shell.state(),
        shell.frame().newmap,
        shell.game.vars.mapptr,
        player.worldx,
        player.worldy,
        player.worldz,
    );
}
