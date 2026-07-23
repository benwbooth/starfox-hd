//! Tick 153: AUDIT_ENEMY_A High #10 — base1 hit-triggered door
//! (KSTRATS.ASM:373-408): idle until HF1 → open anim 0→8 + DoorOpen →
//! wait sbyte1=5 → DoorClose → close anim 8→0 → re-init.

use sf_game::game::{Game, Hooks, PosSndFamilyId};
use sf_game::vars::HARD_HP;
use sf_strat::enemy_a::strat_base1_init;
use std::cell::RefCell;
use std::rc::Rc;

const HF1: u8 = 0x01;
const DEG180: u8 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SndEvent {
    MakeSnd(PosSndFamilyId, i16, i16),
}

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<SndEvent>>>);
impl Hooks for Rec {
    fn make_snd(&mut self, family: PosSndFamilyId, x: i16, z: i16) {
        self.0.borrow_mut().push(SndEvent::MakeSnd(family, x, z));
    }
}

fn run(g: &mut Game, idx: u16) {
    let s = g.objs.aliens[idx as usize].stratptr.expect("strat");
    g.call_strat(s, idx);
}

#[test]
fn base1_init_matches_rom_alptrs_aldata() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("door");
    g.objs.aliens[idx as usize].active = true;
    strat_base1_init(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert!(al.stratptr.is_some());
    assert!(al.collstratptr.is_none(), "null collide");
    assert!(al.expstratptr.is_none(), "null explode");
    assert_eq!(al.hp, HARD_HP);
    assert_eq!(al.ap, 2);
    assert_eq!(al.roty, DEG180);
    assert_eq!(al.animframe, 0);
}

#[test]
fn base1_idles_until_hf1_then_opens() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    let idx = g.objs.alloc().expect("door");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.active = true;
        al.worldx = 40;
        al.worldz = 900;
    }
    strat_base1_init(&mut g, idx);
    let idle = g.objs.aliens[idx as usize].stratptr;

    // No HF1 → stay idle, no sound, anim unchanged.
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratptr, idle);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 0);
    assert!(log.borrow().is_empty());

    // HF1 → DoorOpen + open strat; first open tick advances anim 0→1.
    g.objs.aliens[idx as usize].hitflags |= HF1;
    run(&mut g, idx);
    assert_eq!(
        *log.borrow(),
        vec![SndEvent::MakeSnd(PosSndFamilyId::DoorOpen, 40, 900)]
    );
    assert_eq!(g.objs.aliens[idx as usize].hitflags & HF1, 0);
    assert_ne!(g.objs.aliens[idx as usize].stratptr, idle);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 1);
}

#[test]
fn base1_full_open_wait_close_cycle() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    let idx = g.objs.alloc().expect("door");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.active = true;
        al.worldx = -20;
        al.worldz = 300;
    }
    strat_base1_init(&mut g, idx);
    let idle = g.objs.aliens[idx as usize].stratptr;

    // Trigger open.
    g.objs.aliens[idx as usize].hitflags |= HF1;
    run(&mut g, idx); // anim 1, DoorOpen
    assert_eq!(log.borrow().len(), 1);

    // Open until anim==8 (7 more +1 ticks from 1→8).
    while g.objs.aliens[idx as usize].animframe < 8 {
        run(&mut g, idx);
    }
    assert_eq!(g.objs.aliens[idx as usize].animframe, 8);

    // Next tick at anim==8 enters wait (sbyte1=5) and decs to 4 same frame.
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 4);
    assert_eq!(log.borrow().len(), 1, "no close SE yet");

    // Dec 4→3→2→1→0 over four ticks.
    for _ in 0..4 {
        run(&mut g, idx);
    }
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 0);

    // sbyte1==0 → DoorClose + start closing (anim 8→7).
    run(&mut g, idx);
    assert_eq!(
        log.borrow().last().copied(),
        Some(SndEvent::MakeSnd(PosSndFamilyId::DoorClose, -20, 300))
    );
    assert_eq!(g.objs.aliens[idx as usize].animframe, 7);

    // Close to 0 then re-init to idle.
    while g.objs.aliens[idx as usize].animframe > 0 {
        run(&mut g, idx);
    }
    run(&mut g, idx); // anim==0 → base1_istrat
    assert_eq!(g.objs.aliens[idx as usize].stratptr, idle);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 0);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG180);
    assert_eq!(g.objs.aliens[idx as usize].ap, 2);
    assert!(g.objs.aliens[idx as usize].collstratptr.is_none());
}
