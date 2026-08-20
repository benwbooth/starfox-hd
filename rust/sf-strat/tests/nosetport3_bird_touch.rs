//! Tick 133: nosetport3 gate + bird_touch path inline.

use sf_game::game::{Game, Hooks};
use sf_game::shell::{
    le, GameState, Shell, SoundCmd, BRIEFING_INPUT_DELAY_TICKS, INTRO_INPUT_DELAY_TICKS,
    TITLE_INPUT_DELAY_TICKS, TITLE_PRESENTATION_INPUT_READY_TICKS,
};
use sf_game::vars::{PSF3_ENGINESND, PSF3_NOCOLLISIONS};
use sf_strat::path_adapter::path_bird_touch;
use sf_strat::table::register_all;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Ev {
    Music(u8),
    NoSet(bool),
}

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<Ev>>>);
impl Hooks for Rec {
    fn play_music(&mut self, id: u8) {
        self.0.borrow_mut().push(Ev::Music(id));
    }
    fn set_nosetport3(&mut self, disabled: bool) {
        self.0.borrow_mut().push(Ev::NoSet(disabled));
    }
}

#[test]
fn bird_touch_sets_nosetport3_enterspec_and_bgm2() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    g.vars.pshipflags3 |= PSF3_ENGINESND;
    path_bird_touch(&mut g);
    assert_eq!(g.world.levelfinished, le::ENTERSPEC);
    assert_eq!(g.vars.pshipflags3 & PSF3_ENGINESND, 0);
    assert_ne!(g.vars.pshipflags3 & PSF3_NOCOLLISIONS, 0);
    assert!(log.borrow().contains(&Ev::NoSet(true)));
    assert!(log.borrow().contains(&Ev::Music(0x02)));
}

#[test]
fn path_register_all_registers_bird_touch_inline() {
    let mut g = Game::new();
    register_all(&mut g);
    let pw = g.path.as_ref().expect("PathWorld");
    let ips = sf_path::literals::get_catalog().ips;
    assert_ne!(ips.e_big_bird_touch, 0xFFFF);
    assert_eq!(
        pw.find_inline_code(ips.e_big_bird_touch)
            .map(|(callback, _)| callback),
        Some(8),
        "bird_touch IP must map to CB_E_BIG_BIRD_TOUCH"
    );
}

#[test]
fn shell_planets_init_clears_nosetport3() {
    let mut sh = Shell::new();
    sh.tick(0);
    sh.tick(0);
    while sh.game.vars.gameframe < INTRO_INPUT_DELAY_TICKS {
        sh.tick(sf_core::pad::A);
    }
    while sh.state() != GameState::Title {
        sh.tick(0);
    }
    sh.tick(0);
    for _ in 1..TITLE_PRESENTATION_INPUT_READY_TICKS {
        sh.tick(0);
    }
    sh.game.vars.gameframe = TITLE_INPUT_DELAY_TICKS;
    let _ = sh.drain_sound();
    sh.tick(sf_core::pad::START);
    while sh.state() == GameState::Title {
        sh.tick(0);
    }
    sh.tick(0);
    sh.game.vars.gameframe = BRIEFING_INPUT_DELAY_TICKS - 1;
    sh.tick(sf_core::pad::START);
    sh.tick(0);
    sh.tick(sf_core::pad::DOWN);
    sh.tick(0);
    sh.tick(sf_core::pad::START);
    while sh.state() == GameState::Briefing {
        sh.tick(0);
    }
    let snd = sh.drain_sound();
    assert!(
        snd.contains(&SoundCmd::NoSetPort3(false)),
        "planetseq stz nosetport3, got {snd:?}"
    );
}

#[test]
fn shell_hooks_set_nosetport3_queues_cmd() {
    let mut sh = Shell::new();
    sh.game.hooks.set_nosetport3(true);
    assert!(sh.drain_sound().contains(&SoundCmd::NoSetPort3(true)));
    sh.game.hooks.set_nosetport3(false);
    assert!(sh.drain_sound().contains(&SoundCmd::NoSetPort3(false)));
}
