//! Tick 212: chicken `wings_istrat` (DSTRATS.ASM:4744-4764) — flap loop
//! (reset at frame 14 → 4) and sflag1 fold reverse to 0.

use sf_game::alien::{ASF3_REALOBJ, ASF_COLLDISABLE, ASF_NOHITAFFECT, ASF_SHADOW, ATZREMOVE};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_game::vars::{HARD_AP, HARD_HP};
use sf_strat::bosses::{chicken_wings_istrat, chicken_wings_strat};

const CH_SFLAG1: u8 = 0x10;

fn spawn_player(g: &mut Game) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.internal_playpt = 0;
}

fn spawn_wing(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("w");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn anim(g: &Game, idx: u16) -> u8 {
    g.objs.aliens[idx as usize].animframe & 0x7F
}

/// Init: hardHP/AP, nohitaffect, colldisable, shadow, no zremove, hitflash coll.
/// Fall-through `.strat` advances frame 0 → 1 with 0x80 active bit.
#[test]
fn wings_istrat_sets_flags_and_falls_into_flap() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_wing(&mut g);
    chicken_wings_istrat(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.hp, HARD_HP);
    assert_eq!(al.ap, HARD_AP);
    assert_ne!(al.sflags & ASF_NOHITAFFECT, 0);
    assert_ne!(al.sflags & ASF_COLLDISABLE, 0);
    assert_ne!(al.sflags & ASF_SHADOW, 0);
    assert_eq!(al.type_ & ATZREMOVE, 0);
    assert!(al.collstratptr.is_some(), "hitflash_istrat");
    assert!(al.expstratptr.is_some(), "explode_istrat");
    assert_eq!(al.animframe & 0x80, 0x80);
    assert_eq!(anim(&g, idx), 1, "init falls through flap +1");
}

/// Flap: 1..13 then at 14 reset to 4; thereafter 4..14 loop.
#[test]
fn wings_flap_resets_at_14_to_4() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_wing(&mut g);
    chicken_wings_istrat(&mut g, idx); // frame 1
                                       // Drive to frame 13.
    while anim(&g, idx) < 13 {
        chicken_wings_strat(&mut g, idx);
    }
    assert_eq!(anim(&g, idx), 13);
    chicken_wings_strat(&mut g, idx); // 13→14→init 4
    assert_eq!(anim(&g, idx), 4);
    // Next cycle: 4→5→…→13→4
    for expect in 5..=13 {
        chicken_wings_strat(&mut g, idx);
        assert_eq!(anim(&g, idx), expect);
    }
    chicken_wings_strat(&mut g, idx);
    assert_eq!(anim(&g, idx), 4);
}

/// sflag1 fold: reverse anim to 0 and hold.
#[test]
fn wings_fold_reverses_to_zero() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_wing(&mut g);
    chicken_wings_istrat(&mut g, idx);
    // Advance a few flap frames, then fold.
    for _ in 0..6 {
        chicken_wings_strat(&mut g, idx);
    }
    let mid = anim(&g, idx);
    assert!(mid > 0);
    g.objs.aliens[idx as usize].sflags2 |= CH_SFLAG1;
    for _ in 0..(mid as usize + 2) {
        chicken_wings_strat(&mut g, idx);
    }
    assert_eq!(anim(&g, idx), 0);
    chicken_wings_strat(&mut g, idx);
    assert_eq!(anim(&g, idx), 0, "holds at 0 while folded");
}

/// Clear sflag1 → resume flap from current frame.
#[test]
fn wings_unfold_resumes_flap() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_wing(&mut g);
    chicken_wings_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].animframe = 0x80 | 8;
    g.objs.aliens[idx as usize].sflags2 |= CH_SFLAG1;
    chicken_wings_strat(&mut g, idx);
    assert_eq!(anim(&g, idx), 7);
    g.objs.aliens[idx as usize].sflags2 &= !CH_SFLAG1;
    chicken_wings_strat(&mut g, idx);
    assert_eq!(anim(&g, idx), 8);
}
