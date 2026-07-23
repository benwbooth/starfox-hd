//! Tick 204: misspod / walker1 / truck custom `s_fire_weapon` paths must
//! `jsl missilesound_l` via `make_snd(Missile)` (gen_weapon), not silent spawn.

use sf_game::alien::ATMISSILE;
use sf_game::game::{Game, Hooks, PosSndFamilyId};
use sf_strat::enemies_ground::{
    misspoda_init, misspoda_strat, truck_cont, walker1_istrat, walker1_strat,
};
use sf_strat::enemy_a::ASF2_SFLAG2;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SndEvent {
    MakeSnd(PosSndFamilyId, i16, i16),
    PlaySe(u8),
}

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<SndEvent>>>);
impl Hooks for Rec {
    fn make_snd(&mut self, family: PosSndFamilyId, x: i16, z: i16) {
        self.0.borrow_mut().push(SndEvent::MakeSnd(family, x, z));
    }
    fn play_se(&mut self, sound_id: u8) {
        self.0.borrow_mut().push(SndEvent::PlaySe(sound_id));
    }
}

fn spawn_player(g: &mut Game, x: i16, y: i16, z: i16) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    g.vars.internal_playpt = 0;
    g.vars.player_posx = x;
    g.vars.player_posy = y;
    g.vars.player_posz = z;
}

fn spawn_enemy(g: &mut Game, x: i16, y: i16, z: i16) -> u16 {
    let idx = g.objs.alloc().expect("e");
    let al = &mut g.objs.aliens[idx as usize];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    idx
}

fn count_missiles(g: &Game, skip: u16) -> usize {
    g.objs
        .aliens
        .iter()
        .enumerate()
        .filter(|(i, a)| *i as u16 != skip && *i != 0 && a.active && a.type_ & ATMISSILE != 0)
        .count()
}

fn missile_snd_count(log: &RefCell<Vec<SndEvent>>, x: i16, z: i16) -> usize {
    log.borrow()
        .iter()
        .filter(|e| matches!(e, SndEvent::MakeSnd(PosSndFamilyId::Missile, mx, mz) if *mx == x && *mz == z))
        .count()
}

/// misspoda_init: trigse $49 once, then 5× missilesound (one per s_fire_weapon).
#[test]
fn misspoda_init_trigse49_and_five_missile_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0, -40, 0);
    let e = spawn_enemy(&mut g, 40, 0, 500);
    g.objs.aliens[e as usize].sbyte1 = 0; // misspodX pattern
    misspoda_init(&mut g, e);

    assert_eq!(count_missiles(&g, e), 5, "5 missile2 shots");
    assert!(
        log.borrow()
            .iter()
            .any(|e| matches!(e, SndEvent::PlaySe(0x49))),
        "misspoda_init trigse $49; got {:?}",
        log.borrow()
    );
    assert_eq!(
        missile_snd_count(&log, 40, 500),
        5,
        "5× missilesound_l; got {:?}",
        log.borrow()
    );
}

/// misspoda_strat alone (no init trigse): still 5× Missile SE.
#[test]
fn misspoda_strat_five_missile_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0, -40, 0);
    let e = spawn_enemy(&mut g, -20, 0, 800);
    g.objs.aliens[e as usize].sbyte1 = 1; // misspodH
    misspoda_strat(&mut g, e);

    assert_eq!(count_missiles(&g, e), 5);
    assert_eq!(missile_snd_count(&log, -20, 800), 5);
    assert!(
        !log.borrow()
            .iter()
            .any(|e| matches!(e, SndEvent::PlaySe(_))),
        "strat body has no flat trigse; got {:?}",
        log.borrow()
    );
}

/// walker1 in-range: one HMISSILE1 + missilesound.
#[test]
fn walker1_fire_plays_missile_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0, -40, 0);
    let e = spawn_enemy(&mut g, 0, -100, 2000); // xzdist 2000 < 3000
    walker1_istrat(&mut g, e);
    walker1_strat(&mut g, e);

    assert_eq!(count_missiles(&g, e), 1);
    let wx = g.objs.aliens[e as usize].worldx;
    let wz = g.objs.aliens[e as usize].worldz;
    assert_eq!(
        missile_snd_count(&log, wx, wz),
        1,
        "walker1 HMISSILE1 → missilesound at firer XZ; got {:?}",
        log.borrow()
    );
}

/// truck_norm fire gate: one HMISSILE1 + missilesound.
#[test]
fn truck_fire_plays_missile_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0, -40, 0);
    let e = spawn_enemy(&mut g, 10, 0, 2000); // |dz|=2000 ∈ [1000,3000)
    g.vars.gameframe = 0; // notdelay(4): gameframe & 15 == 0
    {
        let al = &mut g.objs.aliens[e as usize];
        al.sbyte1 = al.roty;
        al.vel = 30;
        al.sflags2 &= !ASF2_SFLAG2;
    }
    truck_cont(&mut g, e);

    assert_eq!(count_missiles(&g, e), 1);
    assert_eq!(
        missile_snd_count(&log, 10, 2000),
        1,
        "truck HMISSILE1 → missilesound; got {:?}",
        log.borrow()
    );
}
