use std::cell::RefCell;
use std::rc::Rc;

use sf_core::pad;
use sf_core::player_view::{PlayerViewMode, PlayerViewOptions};
use sf_game::game::Hooks;
use sf_game::vars::{
    CLOSE_VIEW_DISTANCE, OUTVIEWDIST, PSF3_NOVIEWCHANGE, PSF_NOCTRL, PSF_NOFIRE, PSTF_INSEQ,
    PSTF_NOTDIE, STAY_BLACK_INACTIVE,
};
use sf_game::world::op;
use sf_game::Game;
use sf_map::catalog::background_id;
use sf_map::levels::BuiltLevel;

#[derive(Clone)]
struct SoundLog(Rc<RefCell<Vec<u8>>>);

impl Hooks for SoundLog {
    fn play_se(&mut self, sound_id: u8) {
        self.0.borrow_mut().push(sound_id);
    }
}

fn playable_game() -> (Game, u16, Rc<RefCell<Vec<u8>>>) {
    let sounds = Rc::new(RefCell::new(Vec::new()));
    let mut game = Game::with_hooks(Box::new(SoundLog(sounds.clone())));
    let player = game.objs.alloc().expect("player slot");
    game.vars.internal_playpt = player as i16;
    game.vars.strategy.stay_black = STAY_BLACK_INACTIVE;
    game.vars.player_view_mode = PlayerViewMode::Exterior;
    game.vars.player_view_options = PlayerViewOptions::ExteriorViews;
    game.vars.viewdist = OUTVIEWDIST;
    game.vars.strategy.view_distance = OUTVIEWDIST;
    (game, player, sounds)
}

#[test]
fn select_edge_cycles_once_and_plays_the_authored_sound() {
    let (mut game, _, sounds) = playable_game();
    game.vars.pad1 = pad::SELECT;
    game.tick();

    assert_eq!(game.vars.player_view_mode, PlayerViewMode::CloseExterior);
    assert_eq!(game.vars.viewdist, CLOSE_VIEW_DISTANCE);
    assert_eq!(&*sounds.borrow(), &[0x65]);

    // A held button is not another edge. Equalize the eased distance so the
    // input-history gate is the only blocker on the second tick.
    game.vars.strategy.view_distance = CLOSE_VIEW_DISTANCE;
    game.tick();
    assert_eq!(game.vars.player_view_mode, PlayerViewMode::CloseExterior);
    assert_eq!(&*sounds.borrow(), &[0x65]);
}

type Gate = fn(&mut Game, u16);

#[test]
fn every_retail_view_change_gate_blocks_select() {
    let gates: &[(&str, Gate)] = &[
        ("view changes disabled", |game, _| {
            game.vars.pshipflags3 |= PSF3_NOVIEWCHANGE;
        }),
        ("view distance still easing", |game, _| {
            game.vars.strategy.view_distance = CLOSE_VIEW_DISTANCE;
        }),
        ("black hold", |game, _| {
            game.vars.strategy.stay_black = 0;
        }),
        ("control locked", |game, _| {
            game.vars.pshipflags |= PSF_NOCTRL;
        }),
        ("player cannot die", |game, _| {
            game.vars.pstratflags |= PSTF_NOTDIE;
        }),
        ("scripted sequence", |game, _| {
            game.vars.pstratflags |= PSTF_INSEQ;
        }),
        ("boost or brake active", |game, player| {
            game.objs.aliens[player as usize].sbyte2 = 1;
        }),
        ("select already held", |game, _| {
            game.vars.lastcont0 = (pad::SELECT >> 8) as u8;
            game.vars.lastcontl0 = pad::SELECT as u8;
        }),
    ];

    for &(name, gate) in gates {
        let (mut game, player, sounds) = playable_game();
        gate(&mut game, player);
        game.vars.pad1 = pad::SELECT;
        game.tick();
        assert_eq!(
            game.vars.player_view_mode,
            PlayerViewMode::Exterior,
            "gate failed: {name}"
        );
        assert!(sounds.borrow().is_empty(), "gate played sound: {name}");
    }
}

#[test]
fn cleared_option_bound_retains_the_source_wrap_and_sound() {
    let (mut game, _, sounds) = playable_game();
    game.vars.player_view_options = PlayerViewOptions::Unconfigured;
    game.vars.pad1 = pad::SELECT;
    game.tick();

    // A cleared exclusive bound makes the source increment compare fail and
    // wrap back to exterior mode; it is not an additional input gate.
    assert_eq!(game.vars.player_view_mode, PlayerViewMode::Exterior);
    assert_eq!(&*sounds.borrow(), &[0x65]);
}

fn mark_enter_transition(game: &mut Game, _: u16) {
    game.vars.pshipflags |= PSF_NOCTRL;
}

fn mark_leave_transition(game: &mut Game, _: u16) {
    game.vars.pshipflags |= PSF_NOFIRE;
}

#[test]
fn cockpit_cycle_dispatches_registered_native_transitions() {
    let (mut game, player, _) = playable_game();
    let enter = game.world.register_strategy(mark_enter_transition);
    let leave = game.world.register_strategy(mark_leave_transition);
    game.vars.strategy_bindings.enter_cockpit = Some(enter.0);
    game.vars.strategy_bindings.leave_cockpit = Some(leave.0);
    game.vars.player_view_options = PlayerViewOptions::ExteriorAndCockpit;

    game.vars.player_view_mode = PlayerViewMode::CloseExterior;
    game.vars.viewdist = CLOSE_VIEW_DISTANCE;
    game.vars.strategy.view_distance = CLOSE_VIEW_DISTANCE;
    game.vars.pad1 = pad::SELECT;
    game.tick();
    assert_eq!(game.vars.player_view_mode, PlayerViewMode::EnteringCockpit);
    assert_ne!(game.vars.pshipflags & PSF_NOCTRL, 0);

    game.vars.pshipflags = 0;
    game.vars.pstratflags = 0;
    game.vars.lastcont0 = 0;
    game.vars.lastcontl0 = 0;
    game.vars.player_view_mode = PlayerViewMode::Cockpit;
    game.vars.viewdist = CLOSE_VIEW_DISTANCE;
    game.vars.strategy.view_distance = CLOSE_VIEW_DISTANCE;
    game.vars.pad1 = pad::SELECT;
    game.objs.aliens[player as usize].sbyte2 = 0;
    game.tick();
    assert_eq!(game.vars.player_view_mode, PlayerViewMode::LeavingCockpit);
    assert_ne!(game.vars.pshipflags & PSF_NOFIRE, 0);
}

fn level_from_bytes(data: Vec<u8>) -> BuiltLevel {
    BuiltLevel {
        data,
        labels: Vec::new(),
        native_callbacks: Vec::new(),
        inline_callbacks: Vec::new(),
    }
}

#[test]
fn background_boundaries_apply_or_preserve_the_source_view_declaration() {
    let (mut game, _, _) = playable_game();
    game.vars.player_view_mode = PlayerViewMode::Cockpit;
    game.vars.player_view_options = PlayerViewOptions::ExteriorAndCockpit;

    let tunnel_background = background_id::THREE_FOUR_TUNNEL;
    let [tunnel_low, tunnel_high] = tunnel_background.to_le_bytes();
    game.load_level(&level_from_bytes(vec![
        op::SETBG,
        tunnel_low,
        tunnel_high,
        op::END,
    ]));
    game.map_exec();
    assert_eq!(game.vars.player_view_mode, PlayerViewMode::Exterior);
    assert_eq!(
        game.vars.player_view_options,
        PlayerViewOptions::ExteriorViews
    );
    assert_eq!(game.vars.viewdist, OUTVIEWDIST);

    // The following clear-space background has a parameterless pstrat and
    // therefore preserves the tunnel declaration exactly.
    let clear_background = background_id::THREE_FOUR_CLEAR;
    let [clear_low, clear_high] = clear_background.to_le_bytes();
    game.load_level(&level_from_bytes(vec![
        op::SETBG,
        clear_low,
        clear_high,
        op::END,
    ]));
    game.map_exec();
    assert_eq!(game.vars.player_view_mode, PlayerViewMode::Exterior);
    assert_eq!(
        game.vars.player_view_options,
        PlayerViewOptions::ExteriorViews
    );
}
