//! ROM `elaser2die` / `pelaser2die` / `playerbeamdie` (GSTRATS.ASM).

use sf_game::alien::{ASF3_REALOBJ, ASF_COLLDISABLE, ATZREMOVE};
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::enemy_a::{
    elaser2die_istrat, elaser2die_strat, pelaser2die_istrat, playerbeamdie_istrat, ASF2_RELEXPLODE,
};

#[test]
fn elaser2die_spawns_flash_and_animates_out() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("laser");
    g.objs.aliens[idx as usize].worldz = 500;
    elaser2die_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].ptr, 0);
    let flash = g.objs.aliens[idx as usize].ptr - 1;
    assert!(g.objs.aliens[flash as usize].active);
    assert_eq!(g.objs.aliens[idx as usize].next, Some(flash));
    assert_eq!(g.objs.aliens[flash as usize].sflags3 & ASF3_REALOBJ, 0);
    assert_ne!(g.objs.aliens[flash as usize].sflags & ASF_COLLDISABLE, 0);
    // First strat tick already ran from istrat fall-through: anim += 2
    assert_eq!(g.objs.aliens[idx as usize].animframe & 0x7F, 2);

    // ROM: cmp #8 then add_anim; removal is the tick that *starts* on frame 8.
    let mut guard = 0;
    while g.objs.aldead == 0 {
        elaser2die_strat(&mut g, idx);
        guard += 1;
        assert!(guard < 16, "elaser2die never removed");
    }
    assert!(!g.objs.aliens[flash as usize].active);
}

#[test]
fn elaser2die_scrolls_when_relexplode() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("laser");
    g.vars.pviewvelz = 12;
    g.objs.aliens[idx as usize].worldz = 100;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_RELEXPLODE;
    g.objs.aliens[idx as usize].animframe = 0x80; // frame 0
    elaser2die_istrat(&mut g, idx);
    // istrat already ran one strat tick → worldz += 12, anim 2
    assert_eq!(g.objs.aliens[idx as usize].worldz, 112);
}

#[test]
fn pelaser2die_decrements_numplasers_and_clears_zremove() {
    let mut g = Game::new();
    g.vars.set_sv_u8(sv::NUMPLASERS, 3);
    let idx = g.objs.alloc().expect("laser");
    g.objs.aliens[idx as usize].type_ |= ATZREMOVE;
    pelaser2die_istrat(&mut g, idx);
    assert_eq!(g.vars.sv_u8(sv::NUMPLASERS), 2);
    assert_eq!(g.objs.aliens[idx as usize].type_ & ATZREMOVE, 0);
    assert!(g.objs.aliens[idx as usize].expstratptr.is_some());
}

#[test]
fn playerbeamdie_decrements_and_sets_remove_exp() {
    let mut g = Game::new();
    g.vars.set_sv_u8(sv::NUMPLASERS, 1);
    let idx = g.objs.alloc().expect("beam");
    playerbeamdie_istrat(&mut g, idx);
    assert_eq!(g.vars.sv_u8(sv::NUMPLASERS), 0);
    assert!(g.objs.aliens[idx as usize].expstratptr.is_some());
    // Invoke expstrat
    let exp = g.objs.aliens[idx as usize].expstratptr.unwrap();
    g.call_strat(exp, idx);
    assert_eq!(g.objs.aldead, 1);
}
