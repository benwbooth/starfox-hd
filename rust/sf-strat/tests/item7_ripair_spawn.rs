//! Tick 203: item7 broken-wing path spawns ripair (GASTRATS.ASM:2934-2956).
//! Closes AUDIT_ENEMY_A Medium #25 ACCEPTED simplification.

use sf_game::alien::ASF_COLLDISABLE;
use sf_game::game::{Game, Hooks};
use sf_strat::enemy_a::{
    ripair_istrat, ripair_strat, strat_item7_init, wm, PSF2_DOUBLASER, PSF3_BEAMBALL, PSF_BRKLWING,
    PSF_BRKRWING,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Se(u8);

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<Se>>>);
impl Hooks for Rec {
    fn play_se(&mut self, id: u8) {
        self.0.borrow_mut().push(Se(id));
    }
}

fn spawn_player(g: &mut Game) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = 0;
    al.worldy = -40;
    al.worldz = 0;
    g.vars.internal_playpt = 0;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = 0;
    g.vars.pviewvelz = 0;
}

fn spawn_item7(g: &mut Game, z: i16) -> u16 {
    let idx = g.objs.alloc().expect("i7");
    let al = &mut g.objs.aliens[idx as usize];
    al.active = true;
    al.worldx = 0;
    al.worldy = -40;
    al.worldz = z;
    al.sbyte1 = 1; // skip +20 drift
    idx
}

/// Broken wings: spawn ripair (SE $8b), keep break flags, no $17/$15/score yet.
#[test]
fn item7_broken_wings_spawns_ripair_not_inline_repair() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g);
    g.vars.pshipflags |= PSF_BRKLWING | PSF_BRKRWING;
    g.vars.write_ext16(wm::PLAYERSCORE, 0);

    let idx = spawn_item7(&mut g, 50); // |dz|=50 < 120, xy=0 < 60
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    strat_item7_init(&mut g, idx); // falls through into pickup

    assert!(
        g.objs.aliens.iter().filter(|a| a.active).count() > before,
        "ripair_w child spawned"
    );
    assert_ne!(g.vars.pshipflags & (PSF_BRKLWING | PSF_BRKRWING), 0);
    assert_eq!(
        g.vars.read_ext16(wm::PLAYERSCORE),
        0,
        "no score on repair branch"
    );
    assert!(
        !log.borrow()
            .iter()
            .any(|e| matches!(e, Se(0x15) | Se(0x17))),
        "no $15/$17 on spawn; got {:?}",
        log.borrow()
    );
    assert!(
        log.borrow().iter().any(|e| matches!(e, Se(0x8b))),
        "ripair_Istrat trigse $8b; got {:?}",
        log.borrow()
    );
    assert_eq!(g.objs.aliens[idx as usize].count, 20, "flashplayer lifecnt");
}

/// Intact wings: $15 + score + doublaser, no ripair.
#[test]
fn item7_intact_wings_upgrades_doublaser() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g);
    g.vars.pshipflags &= !(PSF_BRKLWING | PSF_BRKRWING);
    g.vars.pshipflags2 &= !PSF2_DOUBLASER;
    g.vars.write_ext16(wm::PLAYERSCORE, 10);

    let idx = spawn_item7(&mut g, 40);
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    strat_item7_init(&mut g, idx);

    assert_eq!(
        g.objs.aliens.iter().filter(|a| a.active).count(),
        before,
        "no ripair child on .dlaser path"
    );
    assert_eq!(g.vars.read_ext16(wm::PLAYERSCORE), 110);
    assert_ne!(g.vars.pshipflags2 & PSF2_DOUBLASER, 0);
    assert!(
        log.borrow().iter().any(|e| matches!(e, Se(0x15))),
        "TRIGSE $15; got {:?}",
        log.borrow()
    );
}

/// Second intact pickup with doublaser already on → beamball.
#[test]
fn item7_second_pickup_sets_beamball() {
    let mut g = Game::new();
    spawn_player(&mut g);
    g.vars.pshipflags2 |= PSF2_DOUBLASER;
    g.vars.pshipflags3 &= !PSF3_BEAMBALL;
    let idx = spawn_item7(&mut g, 40);
    strat_item7_init(&mut g, idx);
    assert_ne!(g.vars.pshipflags3 & PSF3_BEAMBALL, 0);
}

/// End-to-end: item7 broken → ripair catch clears wings + $17.
#[test]
fn item7_ripair_chain_repairs_on_catch() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g);
    g.vars.pshipflags |= PSF_BRKLWING | PSF_BRKRWING;
    let idx = spawn_item7(&mut g, 40);
    strat_item7_init(&mut g, idx);

    let pod = g
        .objs
        .aliens
        .iter()
        .enumerate()
        .find(|(i, a)| {
            *i as u16 != idx
                && *i != 0
                && a.active
                && a.sflags & ASF_COLLDISABLE != 0
                && a.sbyte1 == 30
        })
        .map(|(i, _)| i as u16)
        .expect("ripair child");

    // Burn approach countdown with XY far (same as ripman_woodsgo).
    for _ in 0..30 {
        g.objs.aliens[pod as usize].worldx = 100;
        g.objs.aliens[pod as usize].worldy = 100;
        ripair_strat(&mut g, pod);
    }
    {
        let al = &mut g.objs.aliens[pod as usize];
        al.worldx = 0;
        al.worldy = -40;
        al.worldz = 0;
        al.sbyte1 = 1;
    }
    ripair_strat(&mut g, pod);
    assert_eq!(g.vars.pshipflags & (PSF_BRKLWING | PSF_BRKRWING), 0);
    assert!(
        log.borrow().iter().any(|e| matches!(e, Se(0x17))),
        "TRIGSE $17 on catch; got {:?}",
        log.borrow()
    );
    let _ = ripair_istrat; // silence if unused in some cfgs
}
