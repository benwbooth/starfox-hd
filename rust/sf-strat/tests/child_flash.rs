//! ROM `childremove_Istrat` / `flash_Istrat` / `flash_strat` (GSTRATS.ASM).

use sf_game::alien::{ASF3_CHILDOBJ, ASF3_MOTHEROBJ, ASF_COLLDISABLE};
use sf_game::Game;
use sf_strat::common::{child_remove_istrat, flash_istrat, flash_strat};

#[test]
fn child_remove_unlinks_and_marks_dead() {
    let mut g = Game::new();
    let mother = g.objs.alloc().expect("mother");
    let child = g.objs.alloc().expect("child");
    {
        let m = &mut g.objs.aliens[mother as usize];
        m.sflags3 |= ASF3_MOTHEROBJ;
        m.sword1 = (child as i16).wrapping_add(1);
    }
    {
        let c = &mut g.objs.aliens[child as usize];
        c.sflags3 |= ASF3_CHILDOBJ;
        c.ptr = (mother as u16).wrapping_add(1);
        c.sword1 = 0;
    }

    child_remove_istrat(&mut g, child);
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(g.objs.aliens[child as usize].sflags3 & ASF3_CHILDOBJ, 0);
    assert_eq!(g.objs.aliens[mother as usize].sword1, 0);
}

#[test]
fn flash_animates_then_removes() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.vars.pviewvelz = 10;
    g.objs.aliens[idx as usize].worldz = 100;

    flash_istrat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE != 0);
    assert_eq!(g.objs.aliens[idx as usize].colframe & 0x7F, 0);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 110);

    flash_strat(&mut g, idx); // frame 0 → 1
    assert_eq!(g.objs.aldead, 0);
    assert_eq!(g.objs.aliens[idx as usize].colframe & 0x7F, 1);

    flash_strat(&mut g, idx); // frame 1 → still <2 after add? wait: cmp before add
                              // ROM: cmp #2 then beq remove; else add. So at frame 1: not remove, add→2.
    assert_eq!(g.objs.aldead, 0);
    assert_eq!(g.objs.aliens[idx as usize].colframe & 0x7F, 2);

    flash_strat(&mut g, idx); // frame 2 → remove
    assert_eq!(g.objs.aldead, 1);
}
