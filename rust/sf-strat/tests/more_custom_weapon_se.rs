//! Tick 207: more custom RELSLOW/HPLASMA/RELFAST fire paths → gen_weapon SE.

use sf_game::game::{Game, Hooks, PosSndFamilyId};
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bossb::bossbrobfirep1_init;
use sf_strat::bosses::boss2spark_istrat;
use sf_strat::bossh::bosshtop_init;
use sf_strat::enemy_a::{boss_attach_child_to_mother, ship3_strat, strat_spacebarwalker_init};
use sf_strat::enemy_b::{bossftur1_istrat, bossftur1_strat, strat_spacepilon_init};
use std::cell::RefCell;
use std::rc::Rc;

const ASF2_SFLAG1: u8 = 0x10;

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

/// boss2spark: sbyte1≥10 + delay-1 → RELSLOW + lasersound.
#[test]
fn boss2spark_relslow_plays_laser_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 0, -80, 2000);
    boss2spark_istrat(&mut g, e);
    g.objs.aliens[e as usize].sbyte1 = 10;
    g.vars.gameframe = 0; // frame_tick_mod(1)
    if let Some(s) = g.objs.aliens[e as usize].stratptr {
        g.call_strat(s, e);
    }
    assert_eq!(
        count_family(&log, PosSndFamilyId::Laser),
        1,
        "boss2spark RELSLOW → lasersound; got {:?}",
        log.borrow()
    );
}

/// spacebarwalker: player behind + /16 gate → RELSLOW + lasersound.
#[test]
fn spacebarwalker_relslow_plays_laser_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0); // behind walker at z=2000
    let e = spawn_obj(&mut g, 0, -40, 2000);
    // (gf+idx)&0xF==0 with idx=1 → gf=15
    g.vars.gameframe = 15;
    strat_spacebarwalker_init(&mut g, e);
    assert_eq!(
        count_family(&log, PosSndFamilyId::Laser),
        1,
        "spacebarwalker RELSLOW → lasersound; got {:?}",
        log.borrow()
    );
}

/// bossFtur open window → RELSLOW + lasersound.
#[test]
fn bossftur_relslow_plays_laser_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let mother = spawn_obj(&mut g, 0, 0, 3000);
    let tur = spawn_obj(&mut g, 0, -40, 3000);
    assert!(boss_attach_child_to_mother(&mut g, mother, tur, 1));
    bossftur1_istrat(&mut g, tur);
    {
        let al = &mut g.objs.aliens[tur as usize];
        al.hp = 8; // not hardHP dead
        al.sflags2 |= ASF2_SFLAG1; // open
        al.sbyte2 = 10; // <=15 fire window
        al.sbyte3 = 10; // <=20
    }
    g.vars.gameframe = 0; // &7==0
    log.borrow_mut().clear();
    bossftur1_strat(&mut g, tur);
    assert_eq!(
        count_family(&log, PosSndFamilyId::Laser),
        1,
        "bossFtur RELSLOW → lasersound; got {:?}",
        log.borrow()
    );
}

/// spacepilon state1 fire → HPLASMA + enemybattry.
#[test]
fn spacepilon_hplasma_plays_battry_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 0, -40, 2000);
    strat_spacepilon_init(&mut g, e);
    {
        let al = &mut g.objs.aliens[e as usize];
        al.stratstate = 1;
        al.sbyte2 = 1;
        al.sbyte4 = 10;
    }
    g.vars.gameframe = 0; // &31==0
    log.borrow_mut().clear();
    if let Some(s) = g.objs.aliens[e as usize].stratptr {
        g.call_strat(s, e);
    }
    assert_eq!(
        count_family(&log, PosSndFamilyId::EnemyBattry),
        1,
        "spacepilon HPLASMA → enemybattry; got {:?}",
        log.borrow()
    );
}

/// bossbrob P1 spray → HPLASMA + enemybattry.
#[test]
fn bossbrobfirep1_hplasma_plays_battry_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 0, -80, 2000);
    g.vars.gameframe = 0; // notdelay(4)
    bossbrobfirep1_init(&mut g, e);
    assert!(
        count_family(&log, PosSndFamilyId::EnemyBattry) >= 1,
        "bossbrob P1 HPLASMA → enemybattry; got {:?}",
        log.borrow()
    );
    assert!(
        count_shape(&g, 405) >= 1,
        "bossbrob HPLASMA must use #bouncyball"
    );
}

/// bossH top facing forward → HPLASMA + enemybattry.
#[test]
fn bosshtop_hplasma_plays_battry_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 0, -80, 2000);
    g.objs.aliens[e as usize].roty = 0; // facing forward
    g.vars.gameframe = 0; // notdelay(4)
    bosshtop_init(&mut g, e);
    if let Some(s) = g.objs.aliens[e as usize].stratptr {
        g.call_strat(s, e);
    }
    assert_eq!(
        count_family(&log, PosSndFamilyId::EnemyBattry),
        1,
        "bosshtop HPLASMA → enemybattry; got {:?}",
        log.borrow()
    );
}

/// ship3 at altitude → RELFAST + lasersound.
#[test]
fn ship3_relfast_plays_laser_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 0, 0, 2000); // worldy 0 <= CY+800
    g.objs.aliens[e as usize].vy = 0;
    g.vars.gameframe = 0; // frame_tick_mod(1)
    ship3_strat(&mut g, e);
    assert_eq!(
        count_family(&log, PosSndFamilyId::Laser),
        1,
        "ship3 RELFAST → lasersound; got {:?}",
        log.borrow()
    );
}
