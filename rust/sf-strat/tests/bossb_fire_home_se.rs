//! Tick 209: bossB `fire_home` SE by ROM weapon family — HMISSILE1 /
//! CHICKHMISSILE1 / BOSSHMISSILE1 → `missilesound_l`; RELSLOWELASERHOME →
//! `lasersound_l`. Spinend close path uses laser-home (was bare RELSLOW).

use sf_game::game::{Game, Hooks, PosSndFamilyId};
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bossb::{
    bossbdodge_init, bossbdodge_strat, bossbentsplit2_istrat, bossbentsplit2_strat,
    bossbrobouch_srou, bossbrobrndpos_istrat, bossbrobrndpos_strat, bossbscream2_init,
    bossbscream2_strat, bossbspinend_cont,
};
use std::cell::RefCell;
use std::rc::Rc;

const SHAPE_ENEMY_LASER: u16 = 478;

#[derive(Debug, Clone, PartialEq, Eq)]
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

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = 0;
    al.worldy = -40;
    al.worldz = z;
    g.vars.internal_playpt = 0;
    g.vars.player_posz = z;
}

fn spawn_obj(g: &mut Game, x: i16, y: i16, z: i16) -> u16 {
    let idx = g.objs.alloc().expect("e");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    al.hp = 40;
    idx
}

fn count_family(log: &RefCell<Vec<SndEvent>>, fam: PosSndFamilyId) -> usize {
    log.borrow()
        .iter()
        .filter(|e| matches!(e, SndEvent::MakeSnd(f, _, _) if *f == fam))
        .count()
}

fn count_shape(g: &Game, shape: u16) -> usize {
    g.objs
        .active_indices()
        .into_iter()
        .filter(|&idx| g.objs.aliens[idx as usize].shape == shape)
        .count()
}

/// Dodge far-from-target → HMISSILE1 → missilesound.
#[test]
fn bossbdodge_hmissile_plays_missile_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 5000, 5000, 2000); // far from move-table
    g.objs.aliens[e as usize].hp = 40;
    bossbdodge_init(&mut g, e);
    log.borrow_mut().clear();
    // Keep retarget from rewriting index; stay far; notdelay 3.
    g.objs.aliens[e as usize].sbyte3 = 2;
    g.objs.aliens[e as usize].sbyte1 = 0;
    g.objs.aliens[e as usize].sbyte2 = 0;
    g.objs.aliens[e as usize].worldx = 5000;
    g.objs.aliens[e as usize].worldy = 5000;
    g.vars.gameframe = 0; // &7 == 0
    bossbdodge_strat(&mut g, e);
    assert!(
        count_family(&log, PosSndFamilyId::Missile) >= 1,
        "dodge HMISSILE1 → missilesound; got {:?}",
        *log.borrow()
    );
    assert_eq!(count_family(&log, PosSndFamilyId::Laser), 0);
    assert!(count_shape(&g, 403) >= 1, "HMISSILE1 must use #missile");
}

/// Spinend far → HMISSILE1 Missile; close → RELSLOWELASERHOME Laser.
#[test]
fn bossbspinend_home_se_by_range() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 5000, 5000, 1000);
    g.objs.aliens[e as usize].ptr = 0;
    g.objs.aliens[e as usize].sbyte1 = 0;
    g.objs.aliens[e as usize].sword1 = 0;
    // Far: (gameframe+idx)&7==0
    g.vars.gameframe = e.wrapping_neg() as u16; // +idx → 0
    bossbspinend_cont(&mut g, e);
    assert!(
        count_family(&log, PosSndFamilyId::Missile) >= 1,
        "spinend far HMISSILE1; got {:?}",
        *log.borrow()
    );
    assert_eq!(count_shape(&g, 403), 1, "far shot must be #missile");

    log.borrow_mut().clear();
    // Park on table slot 0 so range < 300; notdelay_stag 2 → &3==0.
    g.objs.aliens[e as usize].worldx = 0;
    g.objs.aliens[e as usize].worldy = -400; // bossbpos_tab slot0-ish after chase
                                             // Force already-at-target by setting to chased coords after one cont...
                                             // Call once to chase, then set exactly to target and fire.
    g.vars.gameframe = 1u16.wrapping_sub(e); // +idx → 1, not fire yet
    let lasers_before = count_shape(&g, SHAPE_ENEMY_LASER);
    bossbspinend_cont(&mut g, e);
    // Snap to current chased position so next tick range < 300.
    let (x, y) = (
        g.objs.aliens[e as usize].worldx,
        g.objs.aliens[e as usize].worldy,
    );
    // Overwrite to exact tab target: index = sbyte1 + (sword1&3)<<5 = 0
    // bossbpos_tab(0) — use current after many chases by setting equal.
    g.objs.aliens[e as usize].worldx = x;
    g.objs.aliens[e as usize].worldy = y;
    // Better: set world to tab entry directly via repeated chase to convergence
    for _ in 0..40 {
        g.vars.gameframe = 1; // avoid fire while converging
        bossbspinend_cont(&mut g, e);
    }
    log.borrow_mut().clear();
    g.vars.gameframe = e.wrapping_neg() as u16; // +idx → 0, notdelay 2 fires
    bossbspinend_cont(&mut g, e);
    assert!(
        count_family(&log, PosSndFamilyId::Laser) >= 1,
        "spinend close RELSLOWELASERHOME → laser; got {:?}",
        *log.borrow()
    );
    assert_eq!(
        count_shape(&g, SHAPE_ENEMY_LASER),
        lasers_before + 1,
        "close shot must be #elaser2a"
    );
}

/// Scream2 at sbyte1 countdown 66 → CHICKHMISSILE1 Missile.
#[test]
fn bossbscream2_chickhmissile_plays_missile_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 0, -200, 1500);
    bossbscream2_init(&mut g, e);
    log.borrow_mut().clear();
    g.objs.aliens[e as usize].sbyte1 = 67; // dec → 66 → fire
    bossbscream2_strat(&mut g, e);
    assert!(
        count_family(&log, PosSndFamilyId::Missile) >= 1,
        "CHICKHMISSILE1 → missilesound; got {:?}",
        *log.borrow()
    );
    assert_eq!(count_shape(&g, 417), 1, "CHICKHMISSILE1 must use #c_miss");
}

/// Entsplit2 → RELSLOWELASERHOME Laser.
#[test]
fn bossbentsplit2_relslowhome_plays_laser_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 100, -100, 800);
    bossbentsplit2_istrat(&mut g, e);
    log.borrow_mut().clear();
    g.vars.gameframe = e.wrapping_neg() as u16; // +idx → 0, notdelay_stag 3
    g.objs.aliens[e as usize].sbyte1 = 2; // avoid split2 handoff
    bossbentsplit2_strat(&mut g, e);
    assert!(
        count_family(&log, PosSndFamilyId::Laser) >= 1,
        "entsplit2 RELSLOWELASERHOME → laser; got {:?}",
        *log.borrow()
    );
    assert_eq!(count_family(&log, PosSndFamilyId::Missile), 0);
    assert_eq!(
        count_shape(&g, SHAPE_ENEMY_LASER),
        1,
        "home laser must use #elaser2a"
    );
}

/// Brob rndpos → RELSLOWELASERHOME Laser; ouch → BOSSHMISSILE1 Missile.
#[test]
fn bossbrob_rndpos_laser_and_ouch_missile_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 0, -80, 1200);
    bossbrobrndpos_istrat(&mut g, e);
    log.borrow_mut().clear();
    g.vars.gameframe = e.wrapping_neg() as u16; // +idx → 0, notdelay_stag 4
    g.objs.aliens[e as usize].sbyte1 = 1; // skip $2d SE path
    bossbrobrndpos_strat(&mut g, e);
    assert!(
        count_family(&log, PosSndFamilyId::Laser) >= 1,
        "rndpos RELSLOWELASERHOME → laser; got {:?}",
        *log.borrow()
    );

    log.borrow_mut().clear();
    g.objs.aliens[e as usize].sbyte3 = 10;
    g.objs.aliens[e as usize].sbyte4 = 0;
    g.vars.gameframe = 0; // notdelay 4
    let missiles_before = count_shape(&g, 403);
    bossbrobouch_srou(&mut g, e);
    assert!(
        count_family(&log, PosSndFamilyId::Missile) >= 1,
        "ouch BOSSHMISSILE1 → missilesound; got {:?}",
        *log.borrow()
    );
    assert_eq!(
        count_shape(&g, 403),
        missiles_before + 1,
        "BOSSHMISSILE1 must use #missile"
    );
}
