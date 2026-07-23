//! Tick 197: AUDIT_PLAYER_MOVE High #1 — boost/brake sbyte2 + noctrl gates.

use sf_core::pad;
use sf_game::game::{Game, Hooks};
use sf_game::vars::PSF_NOCTRL;
use sf_strat::common::{sv, StratRam};
use sf_strat::player::{boost_brake_update, strat_player, strat_spawn_player};
use std::cell::RefCell;
use std::rc::Rc;

const PSF2_BOOSTING: u8 = 32;
const PSF2_BRAKING: u8 = 64; // confirm below if tests fail
const MAX_PSPEED: u8 = 85;
const MIN_PSPEED: u8 = 20;

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<u8>>>);
impl Hooks for Rec {
    fn play_se(&mut self, id: u8) {
        self.0.borrow_mut().push(id);
    }
}

fn set_pad(g: &mut Game, buttons: u16) {
    let prev = g.vars.pad1;
    g.vars.lastcont0 = (prev >> 8) as u8;
    g.vars.lastcontl0 = (prev & 0xFF) as u8;
    g.vars.pad1 = buttons;
}

fn ready(g: &mut Game) -> u16 {
    let idx = strat_spawn_player(g).expect("player");
    g.vars.set_sv_u8(sv::STAYBLACK, (-1i8) as u8);
    g.vars.set_sv_u8(sv::DOINGWIPE, 0);
    g.vars.set_sv_u8(sv::PLAYER_NOCTRLCNT, 0);
    g.vars.pshipflags &= !PSF_NOCTRL;
    g.objs.aliens[idx as usize].sbyte2 = 0;
    g.vars.pshipflags2 &= !(PSF2_BOOSTING | PSF2_BRAKING);
    idx
}

/// High #1: held X boosts once (SE $32), then sbyte2 gate blocks re-fire/SFX.
#[test]
fn boost_held_x_pulses_once_until_sbyte2_expires() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    let idx = ready(&mut g);
    set_pad(&mut g, pad::X);

    boost_brake_update(&mut g, idx);
    assert_eq!(*log.borrow(), vec![0x32]);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 20);
    assert_eq!(g.objs.aliens[idx as usize].vel, MAX_PSPEED);
    assert_ne!(g.vars.pshipflags2 & PSF2_BOOSTING, 0);

    // Held X while timer 20→1: no extra SE.
    for step in 1..=19 {
        log.borrow_mut().clear();
        set_pad(&mut g, pad::X);
        boost_brake_update(&mut g, idx);
        assert!(
            log.borrow().is_empty(),
            "no SE while counting down (step {step}, got {:?})",
            *log.borrow()
        );
        assert_eq!(g.objs.aliens[idx as usize].sbyte2, 20 - step);
    }
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 1);

    // Release pad so the expire tick clears the flag without same-frame re-fire.
    log.borrow_mut().clear();
    set_pad(&mut g, 0);
    boost_brake_update(&mut g, idx);
    assert!(log.borrow().is_empty());
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 0);
    assert_eq!(g.vars.pshipflags2 & PSF2_BOOSTING, 0);

    // Next frame with X held may re-trigger (pulsed burst).
    log.borrow_mut().clear();
    set_pad(&mut g, pad::X);
    boost_brake_update(&mut g, idx);
    assert_eq!(*log.borrow(), vec![0x32]);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 20);
}

/// High #1: brake (B) same pulse gate; SE $33 once.
#[test]
fn brake_held_b_pulses_once() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    let idx = ready(&mut g);
    set_pad(&mut g, pad::B);

    boost_brake_update(&mut g, idx);
    assert_eq!(*log.borrow(), vec![0x33]);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 30);
    assert_eq!(g.vars.sv_u8(sv::PLAYER_TOSPEED), MIN_PSPEED);

    log.borrow_mut().clear();
    set_pad(&mut g, pad::B);
    boost_brake_update(&mut g, idx);
    assert!(log.borrow().is_empty());
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 29);
}

/// High #1: noctrl / stayblack / wipe / noctrlcnt block pad boost.
#[test]
fn boost_blocked_during_noctrl_sequences() {
    for setup in ["noctrl", "stayblack", "wipe", "noctrlcnt"] {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
        let idx = ready(&mut g);
        match setup {
            "noctrl" => g.vars.pshipflags |= PSF_NOCTRL,
            "stayblack" => g.vars.set_sv_u8(sv::STAYBLACK, 0),
            "wipe" => g.vars.set_sv_u8(sv::DOINGWIPE, 1),
            "noctrlcnt" => g.vars.set_sv_u8(sv::PLAYER_NOCTRLCNT, 5),
            _ => unreachable!(),
        }
        set_pad(&mut g, pad::X);
        boost_brake_update(&mut g, idx);
        assert!(log.borrow().is_empty(), "{setup}: must not play boost SE");
        assert_eq!(
            g.objs.aliens[idx as usize].sbyte2, 0,
            "{setup}: must not arm timer"
        );
    }
}

#[test]
fn viewmove_publishes_cruise_boost_and_brake_audio_bits() {
    let mut g = Game::new();
    let idx = ready(&mut g);

    strat_player(&mut g, idx);
    assert_eq!(g.vars.player_snd_flag, 0b0100, "cruise pitch");

    g.objs.aliens[idx as usize].sbyte2 = 2;
    g.vars.pshipflags2 = (g.vars.pshipflags2 & !PSF2_BRAKING) | PSF2_BOOSTING;
    strat_player(&mut g, idx);
    assert_eq!(g.vars.player_snd_flag, 0b1000, "boost pitch");

    g.objs.aliens[idx as usize].sbyte2 = 2;
    g.vars.pshipflags2 = (g.vars.pshipflags2 & !PSF2_BOOSTING) | PSF2_BRAKING;
    strat_player(&mut g, idx);
    assert_eq!(g.vars.player_snd_flag, 0b1100, "brake pitch");
}
