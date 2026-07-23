//! Tick 109: ADD2POS + MOVEOBJTOEND + READ_JOYPAD + SETFADE* / ARCTAN16 flips.

use sf_core::pad::{self, read_joypad};
use sf_game::Game;
use sf_strat::common::add2pos_obj_y_from_obj_x;

#[test]
fn add2pos_copies_src_plus_offset() {
    let mut g = Game::new();
    let src = g.objs.alloc().unwrap();
    let dst = g.objs.alloc().unwrap();
    g.objs.aliens[src as usize].worldx = 100;
    g.objs.aliens[src as usize].worldy = 200;
    g.objs.aliens[src as usize].worldz = 300;
    // Need split borrows — copy src first.
    let src_al = g.objs.aliens[src as usize];
    add2pos_obj_y_from_obj_x(&src_al, &mut g.objs.aliens[dst as usize], 10, -20, 30);
    let d = &g.objs.aliens[dst as usize];
    assert_eq!(d.worldx, 110);
    assert_eq!(d.worldy, 180);
    assert_eq!(d.worldz, 330);
}

#[test]
fn move_obj_to_end_reorders_active_list() {
    let mut g = Game::new();
    // alloc pushes front: last alloc is head.
    let a = g.objs.alloc().unwrap();
    let b = g.objs.alloc().unwrap();
    let c = g.objs.alloc().unwrap();
    // Head = c, then b, then a.
    assert_eq!(g.objs.active_indices(), vec![c, b, a]);
    g.objs.move_obj_to_end(c);
    assert_eq!(g.objs.active_indices(), vec![b, a, c]);
    // Already at end — no-op.
    g.objs.move_obj_to_end(c);
    assert_eq!(g.objs.active_indices(), vec![b, a, c]);
    // Middle to end.
    g.objs.move_obj_to_end(a);
    assert_eq!(g.objs.active_indices(), vec![b, c, a]);
}

#[test]
fn read_joypad_edge_detect() {
    // First sample: all pressed bits are "new".
    let j0 = read_joypad(0, pad::A | pad::START);
    assert_eq!(j0.cont, pad::A | pad::START);
    assert_eq!(j0.trig, pad::A | pad::START);
    // Hold A, release START, press B: only B is new.
    let j1 = read_joypad(j0.cont, pad::A | pad::B);
    assert_eq!(j1.cont, pad::A | pad::B);
    assert_eq!(j1.trig, pad::B);
    // Hold same — no new edges.
    let j2 = read_joypad(j1.cont, pad::A | pad::B);
    assert_eq!(j2.trig, 0);
}

#[test]
fn setfade_opcodes_call_hooks() {
    // SETFADEUPDO/DOWNDO/QUP/QDOWN already wired as FADEUP/FADEDOWN/QFADE*.
    use sf_game::world::op;
    use sf_map::levels::BuiltLevel;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct FadeHooks {
        from: Rc<RefCell<Vec<i32>>>,
        to: Rc<RefCell<Vec<i32>>>,
    }
    impl sf_game::Hooks for FadeHooks {
        fn fade_from_black(&mut self, speed: i32) {
            self.from.borrow_mut().push(speed);
        }
        fn fade_to_black(&mut self, speed: i32) {
            self.to.borrow_mut().push(speed);
        }
    }

    let from = Rc::new(RefCell::new(Vec::new()));
    let to = Rc::new(RefCell::new(Vec::new()));
    let bytes = vec![
        op::FADEUP,
        op::FADEDOWN,
        op::QFADEUP,
        op::QFADEDOWN,
        op::END,
    ];
    let level = BuiltLevel {
        data: bytes,
        labels: Vec::new(),
        native_callbacks: Vec::new(),
        inline_callbacks: Vec::new(),
    };
    let mut g = Game::with_hooks(Box::new(FadeHooks {
        from: Rc::clone(&from),
        to: Rc::clone(&to),
    }));
    g.load_level(&level);
    g.map_exec();
    assert_eq!(*from.borrow(), vec![1, 2]); // FADEUP speed1, QFADEUP speed2
    assert_eq!(*to.borrow(), vec![1, 2]); // FADEDOWN / QFADEDOWN
    assert_eq!(g.vars.mapptr, 4);
}

#[test]
fn arctan16_wrapper_matches_angle_helper() {
    // ARCTAN16_L is the GSU far wrapper; aim angles use strat_angle_xz.
    use sf_game::alien::Alien;
    use sf_strat::common::strat_angle_xz;
    let mut a = Alien::default();
    let mut b = Alien::default();
    a.worldx = 0;
    a.worldz = 0;
    b.worldx = 100;
    b.worldz = 0;
    let ang = strat_angle_xz(&a, &b);
    // +X from origin → ~64 (90°) in SNES angle units.
    assert!((60..=68).contains(&ang), "ang={ang}");
}
