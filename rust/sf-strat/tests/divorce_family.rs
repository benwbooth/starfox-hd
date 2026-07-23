//! Unit tests for `divorce_family` (ROM `divorcefamily_l`, STRATROU.ASM:3000).

use sf_game::Game;
use sf_strat::enemy_a::{
    boss_attach_child_to_mother, divorce_family, ASF3_CHILDOBJ, ASF3_MOTHEROBJ,
};

fn live_game() -> Game {
    Game::new()
}

#[test]
fn divorce_child_unlinks_from_mother() {
    let mut g = live_game();
    let mother = g.objs.alloc().unwrap();
    let child = g.objs.alloc().unwrap();
    assert!(boss_attach_child_to_mother(&mut g, mother, child, 1));
    assert!(g.objs.aliens[mother as usize].sflags3 & ASF3_MOTHEROBJ != 0);
    assert!(g.objs.aliens[child as usize].sflags3 & ASF3_CHILDOBJ != 0);
    assert_ne!(g.objs.aliens[child as usize].ptr, 0);

    divorce_family(&mut g, child);

    assert!(g.objs.aliens[child as usize].sflags3 & ASF3_CHILDOBJ == 0);
    assert_eq!(g.objs.aliens[child as usize].ptr, 0);
    // Mother loses its only child → mother flag cleared.
    assert!(g.objs.aliens[mother as usize].sflags3 & ASF3_MOTHEROBJ == 0);
    assert_eq!(g.objs.aliens[mother as usize].sword1, 0);
}

#[test]
fn divorce_mother_orphans_children() {
    let mut g = live_game();
    let mother = g.objs.alloc().unwrap();
    let c1 = g.objs.alloc().unwrap();
    let c2 = g.objs.alloc().unwrap();
    assert!(boss_attach_child_to_mother(&mut g, mother, c1, 1));
    assert!(boss_attach_child_to_mother(&mut g, mother, c2, 2));

    divorce_family(&mut g, mother);

    assert!(g.objs.aliens[mother as usize].sflags3 & ASF3_MOTHEROBJ == 0);
    for c in [c1, c2] {
        assert!(g.objs.aliens[c as usize].sflags3 & ASF3_CHILDOBJ == 0);
        assert_eq!(g.objs.aliens[c as usize].ptr, 0);
    }
}
