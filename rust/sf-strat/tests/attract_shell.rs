//! End-to-end ENDSEQ attract-cycle regressions through the real shell.

use sf_core::{
    pad,
    sf1_controls::{BriefingChoice, BriefingPhase, ControlType},
};
use sf_game::alien::ASF4_TEXTOBJ;
use sf_game::shell::{
    GameState, Shell, SoundCmd, BRIEFING_CONFIRM_SOUND, BRIEFING_INPUT_DELAY_TICKS,
    BRIEFING_MOVE_SOUND, INTRO_INPUT_DELAY_TICKS, MUSIC_ATTRACT_INTRO, MUSIC_CONTROLLER_SCREEN,
    MUSIC_FADE_OUT, TITLE_ATTRACT_DURATION_TICKS, TITLE_INPUT_DELAY_TICKS,
    TRAINING_INPUT_DELAY_TICKS,
};
use sf_map::catalog::map_id;
use sf_strat::common::{sv, StratRam};

const TRANSITION_LIMIT_TICKS: usize = 40;
const NATURAL_INTRO_LIMIT_TICKS: usize = 1000;
const TEXT_PATH_SPAWN_LIMIT_TICKS: usize = 100;
const EXPECTED_TEXT_PATHS: usize = 2;
const TEXT_PATH_VIEW_DISTANCE: i16 = 4000;

fn make_shell() -> Shell {
    let mut shell = Shell::new();
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    shell.set_spawn_player(Box::new(|game, map| {
        let _ = sf_strat::player::strat_spawn_player_for_map(game, map);
    }));
    shell
}

fn tick_until_state(shell: &mut Shell, expected: GameState, limit: usize) {
    for _ in 0..limit {
        if shell.state() == expected {
            return;
        }
        shell.tick(0);
    }
    assert_eq!(shell.state(), expected);
}

fn skip_boot_intro(shell: &mut Shell) {
    shell.tick(0);
    assert_eq!(shell.state(), GameState::AttractIntro);
    shell.tick(0);
    for _ in 1..INTRO_INPUT_DELAY_TICKS {
        shell.tick(pad::START);
        assert_eq!(shell.state(), GameState::AttractIntro);
    }
    shell.tick(pad::START);
    tick_until_state(shell, GameState::Title, TRANSITION_LIMIT_TICKS);
    shell.tick(0);
}

fn enter_briefing(shell: &mut Shell) {
    skip_boot_intro(shell);
    shell.game.vars.gameframe = TITLE_INPUT_DELAY_TICKS;
    shell.tick(pad::START);
    tick_until_state(shell, GameState::Briefing, TRANSITION_LIMIT_TICKS);
    shell.tick(0);
}

#[test]
fn boot_loads_the_retail_intro_map_player_and_music() {
    let mut shell = make_shell();
    shell.tick(0);
    assert_eq!(shell.state(), GameState::AttractIntro);

    shell.tick(0);
    assert_eq!(shell.game.world.loaded_map_id, Some(map_id::INTRO));
    assert_eq!(shell.frame().newmap, map_id::INTRO);
    assert!(shell.game.objs.player().is_some());
    assert!(shell
        .drain_sound()
        .contains(&SoundCmd::PlayMusic(MUSIC_ATTRACT_INTRO)));
}

#[test]
fn nintendo_presents_paths_stay_at_the_authored_view_distance() {
    let mut shell = make_shell();
    shell.tick(0);
    shell.tick(0);

    let mut paths = Vec::new();
    for _ in 0..TEXT_PATH_SPAWN_LIMIT_TICKS {
        paths = shell
            .game
            .objs
            .active_indices()
            .into_iter()
            .filter(|&object| shell.game.objs.aliens[object as usize].sflags4 & ASF4_TEXTOBJ != 0)
            .collect();
        if paths.len() == EXPECTED_TEXT_PATHS {
            break;
        }
        shell.tick(0);
    }
    assert_eq!(paths.len(), EXPECTED_TEXT_PATHS);

    let path_frame_view_z = shell.game.vars.sv_i16(sv::VIEWPOSZ);
    shell.tick(0);
    let expected_z = path_frame_view_z.wrapping_add(TEXT_PATH_VIEW_DISTANCE);
    let actual = paths
        .iter()
        .map(|&object| {
            let path = shell.game.objs.aliens[object as usize];
            (object, path.worldz)
        })
        .collect::<Vec<_>>();
    assert!(
        actual.iter().all(|&(_, worldz)| worldz == expected_z),
        "expected {expected_z}, got {actual:?}"
    );
}

#[test]
fn intro_skip_gate_and_title_start_gate_reach_the_controller_screen() {
    let mut shell = make_shell();
    skip_boot_intro(&mut shell);
    assert_eq!(shell.game.world.loaded_map_id, Some(map_id::TITLE));

    shell.tick(pad::START);
    assert_eq!(shell.state(), GameState::Title);
    shell.tick(0);
    shell.game.vars.gameframe = TITLE_INPUT_DELAY_TICKS - 1;
    shell.tick(pad::START);
    assert_eq!(shell.state(), GameState::Title);
    let sounds = shell.drain_sound();
    assert!(sounds.contains(&SoundCmd::PlaySe(16)));
    assert!(sounds.contains(&SoundCmd::PlayMusic(MUSIC_FADE_OUT)));

    tick_until_state(&mut shell, GameState::Briefing, TRANSITION_LIMIT_TICKS);
    shell.tick(0);
    assert_eq!(shell.game.world.loaded_map_id, Some(map_id::CONTINUE));
    assert!(shell
        .drain_sound()
        .contains(&SoundCmd::PlayMusic(MUSIC_CONTROLLER_SCREEN)));
}

#[test]
fn controller_layout_and_game_selection_match_cont_asm() {
    let mut shell = make_shell();
    enter_briefing(&mut shell);
    assert_eq!(shell.frame().briefing_phase, BriefingPhase::ControlType);
    assert_eq!(shell.frame().control_type, ControlType::A);

    shell.tick(pad::SELECT);
    assert_eq!(shell.frame().control_type, ControlType::B);
    assert!(shell
        .drain_sound()
        .contains(&SoundCmd::PlaySe(BRIEFING_MOVE_SOUND)));
    shell.tick(0);

    shell.game.vars.gameframe = BRIEFING_INPUT_DELAY_TICKS - 2;
    shell.tick(pad::START);
    assert_eq!(shell.frame().briefing_phase, BriefingPhase::ControlType);
    shell.tick(0);
    shell.game.vars.gameframe = BRIEFING_INPUT_DELAY_TICKS - 1;
    shell.tick(pad::START);
    assert_eq!(shell.frame().briefing_phase, BriefingPhase::Destination);
    assert!(shell
        .drain_sound()
        .contains(&SoundCmd::PlaySe(BRIEFING_CONFIRM_SOUND)));
    shell.tick(0);

    shell.tick(pad::DOWN);
    assert_eq!(shell.frame().briefing_choice, BriefingChoice::Game);
    shell.tick(0);
    shell.tick(pad::START);
    let sounds = shell.drain_sound();
    assert!(sounds.contains(&SoundCmd::PlaySe(BRIEFING_CONFIRM_SOUND)));
    assert!(sounds.contains(&SoundCmd::PlayMusic(MUSIC_FADE_OUT)));
    tick_until_state(&mut shell, GameState::PlanetSelect, TRANSITION_LIMIT_TICKS);
}

#[test]
fn controller_demo_player_camera_matches_the_retail_frame() {
    const ORACLE_CAPTURE_GAME_FRAME: u16 = 26;
    const SOURCE_CONTROLLER_DEMO_SHAPE: u16 = 411;
    const SOURCE_PLAYER_VIEW_VELOCITY: i16 = 63;
    const SOURCE_PLAYER_Y: i16 = -50;
    const SOURCE_PLAYER_Z: i16 = 1638;
    const SOURCE_PLAYER_VIEW_Z: i16 = 1575;
    const SOURCE_VIEW_DISTANCE: i16 = 200;

    let mut shell = make_shell();
    enter_briefing(&mut shell);
    while shell.game.vars.gameframe < ORACLE_CAPTURE_GAME_FRAME {
        shell.tick(0);
    }
    let player = shell
        .game
        .objs
        .player()
        .expect("controller demonstration player");
    assert_eq!(
        shell
            .draw_list()
            .iter()
            .map(|entry| entry.shape_id)
            .collect::<Vec<_>>(),
        vec![SOURCE_CONTROLLER_DEMO_SHAPE],
        "the controller entry point must not execute TITLE.ASM's title craft"
    );
    assert_eq!(
        (
            player.worldx,
            player.worldy,
            player.worldz,
            shell.game.vars.pviewvelz,
            shell.game.vars.sv_i16(sv::PVIEWPOSX),
            shell.game.vars.sv_i16(sv::PVIEWPOSY),
            shell.game.vars.sv_i16(sv::PVIEWPOSZ),
            shell.game.vars.viewdist,
            shell.game.vars.sv_i16(sv::OUTDIST),
        ),
        (
            0,
            SOURCE_PLAYER_Y,
            SOURCE_PLAYER_Z,
            SOURCE_PLAYER_VIEW_VELOCITY,
            0,
            SOURCE_PLAYER_Y,
            SOURCE_PLAYER_VIEW_Z,
            SOURCE_VIEW_DISTANCE,
            SOURCE_VIEW_DISTANCE,
        )
    );
}

#[test]
fn training_selection_launches_and_returns_to_game_selected() {
    let mut shell = make_shell();
    enter_briefing(&mut shell);
    shell.game.vars.gameframe = BRIEFING_INPUT_DELAY_TICKS - 1;
    shell.tick(pad::START);
    assert_eq!(shell.frame().briefing_phase, BriefingPhase::Destination);
    shell.tick(0);
    shell.tick(pad::START);
    tick_until_state(&mut shell, GameState::Playing, TRANSITION_LIMIT_TICKS);
    assert_eq!(shell.game.world.loaded_map_id, Some(map_id::TRAINING));

    shell.tick(0);
    shell.game.vars.gameframe = TRAINING_INPUT_DELAY_TICKS - 1;
    shell.tick(pad::START);
    tick_until_state(&mut shell, GameState::Briefing, TRANSITION_LIMIT_TICKS);
    shell.tick(0);
    assert_eq!(shell.frame().briefing_phase, BriefingPhase::Destination);
    assert_eq!(shell.frame().briefing_choice, BriefingChoice::Game);
}

#[test]
fn unattended_title_fades_back_to_a_fresh_intro() {
    let mut shell = make_shell();
    skip_boot_intro(&mut shell);
    shell.game.vars.gameframe = TITLE_ATTRACT_DURATION_TICKS - 1;
    shell.tick(0);
    assert_eq!(shell.state(), GameState::Title);
    assert!(shell
        .drain_sound()
        .contains(&SoundCmd::PlayMusic(MUSIC_FADE_OUT)));

    tick_until_state(&mut shell, GameState::AttractIntro, TRANSITION_LIMIT_TICKS);
    shell.tick(0);
    assert_eq!(shell.game.world.loaded_map_id, Some(map_id::INTRO));
    assert!(shell
        .drain_sound()
        .contains(&SoundCmd::PlayMusic(MUSIC_ATTRACT_INTRO)));
}

#[test]
fn lead_fighter_naturally_completes_the_intro_without_input() {
    let mut shell = make_shell();
    shell.tick(0);
    shell.tick(0);

    tick_until_state(&mut shell, GameState::Title, NATURAL_INTRO_LIMIT_TICKS);
    assert!(
        shell.game.vars.strategy.intro_exit_requested,
        "the typed lead-fighter exit request never reached the shell"
    );
}
