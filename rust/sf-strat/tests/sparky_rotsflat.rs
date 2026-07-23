//! ROM `rotsflatstay_Istrat` / `sparky_Istrat` / `sparky_strat` (GSTRATS.ASM).

use sf_game::alien::ASF_COLLDISABLE;
use sf_game::Game;
use sf_strat::common::{rotsflatstay_istrat, sparky_istrat, sparky_strat};

#[test]
fn rotsflatstay_disables_collision_and_clears_strats() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    let dummy = g.world.register_strategy(|_g, _i| {});
    g.objs.aliens[idx as usize].stratptr = Some(dummy);
    g.objs.aliens[idx as usize].collstratptr = Some(dummy);
    g.objs.aliens[idx as usize].expstratptr = Some(dummy);

    rotsflatstay_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert!(g.objs.aliens[idx as usize].stratptr.is_none());
    assert!(g.objs.aliens[idx as usize].collstratptr.is_none());
    assert!(g.objs.aliens[idx as usize].expstratptr.is_none());
}

#[test]
fn sparky_lives_two_ticks_then_removes() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    sparky_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 2);
    assert_eq!(g.objs.aliens[idx as usize].shape, 361);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());

    sparky_strat(&mut g, idx); // 2→1, stay
    assert_eq!(g.objs.aldead, 0);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 1);

    sparky_strat(&mut g, idx); // 1→0, remove
    assert_eq!(g.objs.aldead, 1);
}
