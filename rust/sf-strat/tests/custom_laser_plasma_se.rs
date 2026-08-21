//! Tick 206: custom RELSLOW / HPLASMA / SHORTPLASMA fire paths must play
//! ROM gen_weapon SE (`lasersound_l` / `enemybattrysound_l`).

use sf_game::alien::{ACF_WEAPON, NUMBER_AL};
use sf_game::game::{Game, Hooks, PosSndFamilyId};
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bosses::chicken_arm_init;
use sf_strat::enemies_ground::winglazerman3_strat;
use sf_strat::enemy_a::{
    boss1turretfire_end, boss_attach_child_to_mother, houdai_strat, set_bossflags,
    strat_bomwing_init, strat_houdai_init,
};
use sf_strat::enemy_b::bossfa_strat;
use std::cell::RefCell;
use std::rc::Rc;

const DEG180: u8 = 0x80;
const SH_CHICK_GRABBER: u16 = 384;
const CH_SFLAG2: u8 = 0x20;

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

fn count_weapon_objects(g: &Game) -> usize {
    (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].collflags & ACF_WEAPON != 0)
        .count()
}

/// winglazerman3 notdelay-2: twin RELSLOW → 2× lasersound.
#[test]
fn winglazerman_twin_relslow_plays_laser_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 0, -40, 1500);
    g.objs.aliens[e as usize].sbyte1 = 10;
    g.objs.aliens[e as usize].sbyte3 = 5;
    g.vars.gameframe = 0; // notdelay(2): gf&3==0
    winglazerman3_strat(&mut g, e);

    assert!(count_weapon_objects(&g) >= 2, "twin RELSLOWELASER");
    assert_eq!(
        count_family(&log, PosSndFamilyId::Laser),
        2,
        "2× lasersound_l; got {:?}",
        log.borrow()
    );
}

/// boss1 turret open fire: RELSLOW → lasersound.
#[test]
fn boss1turret_relslow_plays_laser_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let mother = spawn_obj(&mut g, 0, -80, 2000);
    let tur = spawn_obj(&mut g, 0, -80, 2000);
    assert!(boss_attach_child_to_mother(&mut g, mother, tur, 2));
    set_bossflags(&mut g, 0);
    let phase = tur as u16;
    g.vars.gameframe = (32u16).wrapping_sub(phase % 32);
    assert_eq!((g.vars.gameframe.wrapping_add(phase)) & 31, 0);
    boss1turretfire_end(&mut g, tur, mother);

    assert!(count_weapon_objects(&g) >= 1);
    assert_eq!(
        count_family(&log, PosSndFamilyId::Laser),
        1,
        "turret RELSLOW → lasersound; got {:?}",
        log.borrow()
    );
}

/// bossFA dual RELSLOW volley → 2× lasersound.
#[test]
fn bossfa_twin_relslow_plays_laser_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let mother = spawn_obj(&mut g, 0, 0, 3000);
    let fa = spawn_obj(&mut g, 0, 900, 3000); // above space_viewcy+800 gate
    assert!(boss_attach_child_to_mother(&mut g, mother, fa, 1));
    g.objs.aliens[mother as usize].sflags2 &= !0x10; // not combined (sflag1)
    g.objs.aliens[fa as usize].roty = DEG180;
    g.objs.aliens[fa as usize].stratstate = 1;
    g.vars.gameframe = 0; // &3==0
    bossfa_strat(&mut g, fa);

    assert!(count_weapon_objects(&g) >= 2);
    assert_eq!(
        count_family(&log, PosSndFamilyId::Laser),
        2,
        "bossFA 2× RELSLOW → lasersound; got {:?}",
        log.borrow()
    );
}

/// houdai SHORTPLASMA → enemybattrysound.
#[test]
fn houdai_shortplasma_plays_battry_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 0, 0, 2000); // xzdist 2000 >= 800
    strat_houdai_init(&mut g, e);
    // The retail object-pool phase for slot 1 is 108, so
    // (gameframe + phase) & 15 == 0 at gameframe 4.
    g.vars.gameframe = 4;
    houdai_strat(&mut g, e);

    assert_eq!(
        count_family(&log, PosSndFamilyId::EnemyBattry),
        1,
        "SHORTPLASMA → enemybattry; got {:?}",
        log.borrow()
    );
}

/// bomwing phase2 HPLASMA → enemybattrysound.
#[test]
fn bomwing_hplasma_plays_battry_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 0, -40, 1000);
    strat_bomwing_init(&mut g, e);
    // Force phase2 fire: sbyte1==0 enters phase2; roty=deg180 (not +Z cone);
    // gf&7==0.
    g.objs.aliens[e as usize].sbyte1 = 0;
    g.objs.aliens[e as usize].roty = DEG180;
    g.vars.gameframe = 0;
    if let Some(s) = g.objs.aliens[e as usize].stratptr {
        g.call_strat(s, e);
    }

    assert_eq!(
        count_family(&log, PosSndFamilyId::EnemyBattry),
        1,
        "bomwing HPLASMA → enemybattry; got {:?}",
        log.borrow()
    );
}

/// chicken grabber arm sflag2 → HPLASMA + enemybattrysound.
#[test]
fn chicken_arm_hplasma_plays_battry_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let arm = spawn_obj(&mut g, 0, -80, 1500);
    g.objs.aliens[arm as usize].shape = SH_CHICK_GRABBER;
    g.objs.aliens[arm as usize].sflags2 |= CH_SFLAG2;
    chicken_arm_init(&mut g, arm); // falls into strat → fire_bulge

    assert_eq!(
        count_family(&log, PosSndFamilyId::EnemyBattry),
        1,
        "chicken HPLASMA → enemybattry; got {:?}",
        log.borrow()
    );
}
