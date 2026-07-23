//! Tick 83: teleporter_istrat (bossH prop) + bonfire fire points.

use sf_game::alien::ASF_COLLDISABLE;
use sf_game::vars::{HARD_AP, HARD_HP};
use sf_game::Game;
use sf_strat::bossh::{teleporter_istrat, teleporter_strat};
use sf_strat::enemy_a::ASF2_SFLAG1;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 2000;
    idx
}

#[test]
fn teleporter_init_and_bonfire_at_20() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = 0;
    g.vars.internal_playpt = 0;

    let idx = spawn(&mut g);
    teleporter_istrat(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.hp, HARD_HP);
    assert_eq!(al.ap, HARD_AP);
    assert_ne!(al.sflags & ASF_COLLDISABLE, 0);
    // Fall-through: beqdec 50→49, anim advances, ty -= 5
    assert_eq!(al.sbyte2, 49);
    assert_eq!(al.animframe & 0x7F, 1);
    assert_eq!(al.ty, 0u8.wrapping_sub(5));
    assert_eq!(al.rotx, 0);
    assert_eq!(al.roty, 0);
    assert_eq!(al.rotz, 0);

    // Drive to sbyte2==20 fire point: after init at 49, need 29 more ticks to hit 20
    // (49→20 takes 29 decs). On the frame that enters with sbyte2==20, fire then dec.
    g.objs.aliens[idx as usize].sbyte2 = 20;
    let before = g.objs.active_indices().len();
    teleporter_strat(&mut g, idx);
    assert!(
        g.objs.active_indices().len() > before,
        "bonfire at sbyte2==20"
    );
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 19);
}

#[test]
fn teleporter_sflag1_retracts_then_removes() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    teleporter_istrat(&mut g, idx);
    // Force open anim + death latch
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
    g.objs.aliens[idx as usize].animframe = 0x80 | 3;
    g.objs.aldead = 0;
    teleporter_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe & 0x7F, 2);
    assert_eq!(g.objs.aldead, 0);

    g.objs.aliens[idx as usize].animframe = 0x80; // frame 0
    teleporter_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn teleporter_knockitdown_removes_at_anim_cap() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    teleporter_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].sbyte2 = 0;
    g.objs.aliens[idx as usize].animframe = 0x80 | 19; // next +1 → cap → remove
    g.objs.aldead = 0;
    teleporter_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}
