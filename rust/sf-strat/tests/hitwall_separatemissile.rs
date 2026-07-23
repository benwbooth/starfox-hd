//! Tick 146: AUDIT_SOUND_IDS hitwall follow-up —
//! `pelasercollide_Istrat` solid-hit → `make_snd(HitWall)` (GSTRATS.ASM:763);
//! `separatemissile_l` has zero STRAT call sites (dead ROM helper).

use sf_game::alien::ASF_NOHITAFFECT;
use sf_game::game::{Game, Hooks, PosSndFamilyId};
use sf_game::vars::HARD_HP;
use sf_strat::enemy_a::pelasercollide_istrat;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SndEvent {
    PlaySe(u8),
    MakeSnd(PosSndFamilyId, i16, i16),
}

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<SndEvent>>>);
impl Hooks for Rec {
    fn play_se(&mut self, id: u8) {
        self.0.borrow_mut().push(SndEvent::PlaySe(id));
    }
    fn trig_se(&mut self, id: u8) {
        self.0.borrow_mut().push(SndEvent::PlaySe(id));
    }
    fn make_snd(&mut self, family: PosSndFamilyId, x: i16, z: i16) {
        self.0.borrow_mut().push(SndEvent::MakeSnd(family, x, z));
    }
}

#[test]
fn pelasercollide_solid_fires_hitwall_make_snd() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    let laser = g.objs.alloc().expect("laser");
    let wall = g.objs.alloc().expect("wall");
    {
        let al = &mut g.objs.aliens[laser as usize];
        al.active = true;
        al.hp = 5;
        al.worldx = 40;
        al.worldz = 900;
        al.collobjptr = wall;
    }
    {
        let al = &mut g.objs.aliens[wall as usize];
        al.active = true;
        al.hp = HARD_HP;
        al.sflags |= ASF_NOHITAFFECT;
        al.collstratptr = None;
    }
    pelasercollide_istrat(&mut g, laser);
    assert_eq!(
        *log.borrow(),
        vec![SndEvent::MakeSnd(PosSndFamilyId::HitWall, 40, 900)],
        "solid hit → hitwallsound_l"
    );
    assert_eq!(g.objs.aliens[laser as usize].hp, 0);
}

#[test]
fn pelasercollide_soft_partner_skips_hitwall_sound() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    let laser = g.objs.alloc().expect("laser");
    let soft = g.objs.alloc().expect("soft");
    g.objs.aliens[laser as usize].active = true;
    g.objs.aliens[laser as usize].hp = 3;
    g.objs.aliens[laser as usize].collobjptr = soft;
    g.objs.aliens[soft as usize].active = true;
    g.objs.aliens[soft as usize].hp = 10;
    let dummy = g.world.register_strategy(|_g, _| {});
    g.objs.aliens[soft as usize].collstratptr = Some(dummy);
    pelasercollide_istrat(&mut g, laser);
    assert!(
        log.borrow().is_empty(),
        "partner with collstrat → .nsolidhit, no hitwallsound; got {:?}",
        log.borrow()
    );
}

/// `separatemissile_l` (SOUND.ASM:887) is defined but never `jsl`'d from STRAT/*.
/// POS_SEPARATEMISSILE / PosSndFamilyId::SeparateMissile stay for audio completeness.
#[test]
fn separatemissile_l_has_no_strat_call_sites() {
    // Structural: family id exists; no strat should emit SeparateMissile today.
    // (If a future map/weapon wires it, delete this assertion and add a caller test.)
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    let _ = g.objs.alloc();
    assert!(
        !log.borrow()
            .iter()
            .any(|e| matches!(e, SndEvent::MakeSnd(PosSndFamilyId::SeparateMissile, ..))),
        "no accidental SeparateMissile"
    );
}
