//! Tick 145: AUDIT_SOUND_IDS F3/F4 — sea up/down positional make_snd
//! (F1/F2/F5/F6 already covered by `enemy_a::sound_wiring_tests`).

use sf_game::game::{Game, Hooks, PosSndFamilyId};
use sf_strat::bosses::{sea_enemy_down_sea, sea_enemy_up_sea};
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

fn game_with_alien() -> (Game, u16, Rc<RefCell<Vec<SndEvent>>>) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    let idx = g.objs.alloc().expect("alloc");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.active = true;
        al.worldx = -80;
        al.worldz = 400;
    }
    (g, idx, log)
}

#[test]
fn f3_sea_up_fires_positional_enemyupsea() {
    let (mut g, idx, log) = game_with_alien();
    sea_enemy_up_sea(&mut g, idx);
    assert_eq!(
        *log.borrow(),
        vec![SndEvent::MakeSnd(PosSndFamilyId::EnemyUpSea, -80, 400)]
    );
}

#[test]
fn f4_sea_down_fires_positional_enemydownsea() {
    let (mut g, idx, log) = game_with_alien();
    sea_enemy_down_sea(&mut g, idx);
    assert_eq!(
        *log.borrow(),
        vec![SndEvent::MakeSnd(PosSndFamilyId::EnemyDownSea, -80, 400)]
    );
}
