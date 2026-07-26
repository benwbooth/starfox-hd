//! Tick 205: boss custom `s_fire_weapon` HMISSILE paths must
//! `jsl missilesound_l` via `make_snd(Missile)` (gen_weapon).

use sf_game::alien::{ASF_INVISIBLE, ATMISSILE, NUMBER_AL};
use sf_game::game::{Game, Hooks, PosSndFamilyId};
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bosses::{self, strat_flingboss_init, strat_webmonster_init};
use sf_strat::enemy_a::{boss1back_strat, SH_MISSILE};
use sf_strat::enemy_b::bossaattack_strat;
use std::cell::RefCell;
use std::rc::Rc;

const SH_WM_BOSS_0_2: u16 = 434; // webmonster turret
const WM_SFLAG1: u8 = 0x10; // spinning / invuln
const WM_SFLAG2: u8 = 0x20; // armed-to-fire

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
    al.sflags4 |= 0x01; // ASF4_PLAYEROBJ
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

fn missile_snd_count(log: &RefCell<Vec<SndEvent>>) -> usize {
    log.borrow()
        .iter()
        .filter(|e| matches!(e, SndEvent::MakeSnd(PosSndFamilyId::Missile, _, _)))
        .count()
}

fn count_missiles(g: &Game) -> usize {
    (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].type_ & ATMISSILE != 0)
        .count()
}

fn assert_missile_presentation(g: &Game) {
    for missile in g
        .objs
        .aliens
        .iter()
        .filter(|alien| alien.active && alien.type_ & ATMISSILE != 0)
    {
        assert_eq!(missile.shape, SH_MISSILE);
        assert_eq!(missile.sflags & ASF_INVISIBLE, 0);
    }
}

/// boss1back far bombard: (gf+15)&63==0 → boss1_fire_hmissile1 ×1/2 + Missile SE.
#[test]
fn boss1back_hmissile_plays_missile_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 0, -80, 2000); // |dz|>=1500 → .nzi bombard
    g.vars.gameframe = 49; // (49+15)&63==0; 49&63!=0 → no HPLASMA
                           // Cover-gone latch so we don't try to detach a missing cover child.
    g.objs.aliens[e as usize].sflags4 |= 0x80; // BOSS1_PARENT_FLAG_COVER_GONE
    boss1back_strat(&mut g, e);

    let n = count_missiles(&g);
    assert!(n >= 1, "expected HMISSILE1 spawn(s), got {n}");
    assert_eq!(
        missile_snd_count(&log),
        n,
        "one missilesound per HMISSILE1; got {:?}",
        log.borrow()
    );
    assert_missile_presentation(&g);
}

/// bossA attack firemissiles phase: frame 20 of /64 → boss1_fire_hmissile1 + SE.
#[test]
fn bossaattack_hmissile_plays_missile_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let e = spawn_obj(&mut g, 0, -200, 1500);
    g.objs.aliens[e as usize].sbyte3 = 2; // .firemissiles
    g.objs.aliens[e as usize].roty = 0x80;
    g.vars.gameframe = 20; // phase 20 → yaw -deg22
    bossaattack_strat(&mut g, e);

    assert_eq!(count_missiles(&g), 1);
    assert_eq!(
        missile_snd_count(&log),
        1,
        "bossA HMISSILE1 → missilesound; got {:?}",
        log.borrow()
    );
    assert_missile_presentation(&g);
}

/// flingboss fin_body triggermissile2: gf&63==0 → BOSSHMISSILE1 + Missile SE.
#[test]
fn flingboss_triggermissile2_plays_missile_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    bosses::register(&mut g.world);
    spawn_player(&mut g, 0);
    let boss = spawn_obj(&mut g, 0, -80, 500);
    g.objs.aliens[boss as usize].shape = 12; // SH_FLINGBOSS
    strat_flingboss_init(&mut g, boss);

    // Drive approach until main: approach always adds deg22 to rotx first,
    // then hands off when upright (rotx==0) and |dz|<2000.
    const DEG22: u8 = 16;
    g.objs.aliens[boss as usize].worldz = 1000;
    g.objs.aliens[boss as usize].rotx = 0u8.wrapping_sub(DEG22);
    if let Some(s) = g.objs.aliens[boss as usize].stratptr {
        g.call_strat(s, boss); // approach → initmain → main
    }
    log.borrow_mut().clear();
    g.vars.gameframe = 0; // triggermissile2 gate
    let before = count_missiles(&g);
    if let Some(s) = g.objs.aliens[boss as usize].stratptr {
        g.call_strat(s, boss); // main → fin_body → triggermissile2
    }
    let spawned = count_missiles(&g).saturating_sub(before);
    assert!(
        spawned >= 1,
        "flingboss should fire BOSSHMISSILE1 on gf&63==0"
    );
    assert!(
        missile_snd_count(&log) >= 1,
        "flingboss fire → missilesound; got {:?}",
        log.borrow()
    );
    assert_missile_presentation(&g);
}

/// webmonster propturret armed (sflag2, not spinning): one HMISSILE1 + SE.
#[test]
fn webmonster_propturret_fire_plays_missile_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    bosses::register(&mut g.world);
    spawn_player(&mut g, 0);
    let boss = spawn_obj(&mut g, 0, 0, 2000);
    g.objs.aliens[boss as usize].shape = 85; // SH_BOSS_0_1
    strat_webmonster_init(&mut g, boss);

    let turret = (0..NUMBER_AL)
        .find(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == SH_WM_BOSS_0_2)
        .expect("propturret child");
    {
        let al = &mut g.objs.aliens[turret];
        al.sflags2 &= !WM_SFLAG1; // not spinning
        al.sflags2 |= WM_SFLAG2; // armed
    }
    log.borrow_mut().clear();
    let s = g.objs.aliens[turret].stratptr.expect("turret strat");
    g.call_strat(s, turret as u16);

    assert_eq!(count_missiles(&g), 1);
    assert_eq!(
        missile_snd_count(&log),
        1,
        "propturret HMISSILE1 → missilesound; got {:?}",
        log.borrow()
    );
    assert_missile_presentation(&g);
}
