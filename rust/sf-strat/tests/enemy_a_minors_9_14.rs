//! Tick 161: AUDIT_ENEMY_A Minors #9–#14 — bomwing/cameleon no enemy1;
//! flashplayer no colframe seed; gate bank $7E; gate touch→spin same frame;
//! explode special→gate2 + inviewpl gate. (#13 already FIXED via Medium #36.)

use sf_game::alien::{ASF2_COLLDISABLE, ASF3_REALOBJ, ASF_SPECIAL};
use sf_game::draw::AF_INVIEW_PL;
use sf_game::game::{Game, Hooks};
use sf_strat::enemy_a::{
    flashplayer_istrat, strat_bomwing_init, strat_cameleon_init, strat_explode, strat_gate_init,
    wm, COLLTYPE_ENEMY1,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<u8>>>);
impl Hooks for Rec {
    fn play_se(&mut self, id: u8) {
        self.0.borrow_mut().push(id);
    }
    fn trig_se(&mut self, id: u8) {
        self.0.borrow_mut().push(id);
    }
}

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = 0;
    al.worldy = -40;
    al.worldz = z;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn run(g: &mut Game, idx: u16) {
    let s = g.objs.aliens[idx as usize].stratptr.expect("strat");
    g.call_strat(s, idx);
}

/// Minor #9: bomwing_Istrat sets no COLLTYPE_ENEMY1.
#[test]
fn bomwing_init_has_no_enemy1_colltype() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    strat_bomwing_init(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1,
        0,
        "bomwing must not set enemy1"
    );
}

/// Minor #9: cameleon_istrat sets no COLLTYPE_ENEMY1.
#[test]
fn cameleon_init_has_no_enemy1_colltype() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    strat_cameleon_init(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1,
        0,
        "cameleon must not set enemy1"
    );
}

/// Minor #10: flashplayer_Istrat does not write colframe.
#[test]
fn flashplayer_istrat_leaves_colframe_untouched() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].colframe = 7;
    flashplayer_istrat(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].colframe, 7,
        "must not force colframe=0"
    );
}

/// Native gate restart state stores only the flat map-program cursor.
#[test]
fn gate_init_stores_flat_map_cursor() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.mapptr = 0x1234;
    strat_gate_init(&mut g, idx);
    assert_eq!(g.vars.shared.map_restart_temporary, 0x1234);
    assert_ne!(
        g.objs.aliens[idx as usize].sflags2 & ASF2_COLLDISABLE,
        0,
        "gate collision-disable belongs to the source second flag byte"
    );
}

/// Minor #12: gate touch falls through into spin the same frame (sbyte1 bumps).
#[test]
fn gate_touch_runs_spin_same_frame() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let box_idx = spawn(&mut g);
    g.objs.aliens[box_idx as usize].hp = 20;
    g.vars.write_ext16(wm::PCBOXOBJ_B, box_idx);
    let idx = spawn(&mut g);
    // Keep far during init so touch doesn't fire on spawn frame.
    g.objs.aliens[idx as usize].worldz = 10_000;
    strat_gate_init(&mut g, idx);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -40;
    g.objs.aliens[idx as usize].worldz = 50;
    g.objs.aliens[idx as usize].sbyte1 = 0;
    g.objs.aliens[idx as usize].rotz = 0;
    run(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].sbyte1, 1,
        "spin body must run on touch frame (sbyte1++)"
    );
    // First spin tick with entry sbyte1=0 adds 0 to rotz; colframe advances
    // from init #4 into the spin band (>=5).
    assert!(
        g.objs.aliens[idx as usize].colframe >= 5,
        "spin colanim must run same frame, colframe={}",
        g.objs.aliens[idx as usize].colframe
    );
}

/// Minor #14: special explode spawns a gate_2 heal ring.
#[test]
fn explode_special_spawns_gate2() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].flags |= AF_INVIEW_PL;
    g.objs.aliens[idx as usize].sflags |= ASF_SPECIAL;
    g.objs.aliens[idx as usize].sflags2 |= 0x08; // NOEXPSND
    g.objs.aliens[idx as usize].worldz = 800;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    strat_explode(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    // Dying object still active until free; gate_2 is an extra active slot.
    assert!(
        after > before,
        "special explode must spawn gate_2, before={before} after={after}"
    );
    assert_eq!(g.vars.read_ext8(wm::SPECIALS_DEAD), 1);
}

/// Minor #14: not inviewpl → silent remove (no destruct SE).
#[test]
fn explode_not_inview_removes_silently() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].flags &= !AF_INVIEW_PL;
    g.objs.aliens[idx as usize].worldz = 500;
    strat_explode(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert!(
        log.borrow().is_empty(),
        "not inviewpl must skip destruct SE, got {:?}",
        *log.borrow()
    );
}
