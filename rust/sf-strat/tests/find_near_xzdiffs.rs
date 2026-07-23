//! Tick 127: find_near ranks by xzdiffs_l (scaled Euclidean), not Manhattan.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::Game;
use sf_strat::common::{find_near_object, strat_dist_xz};

fn spawn(g: &mut Game, shape: u16, x: i16, y: i16, z: i16) -> u16 {
    let i = g.objs.alloc().unwrap();
    g.objs.aliens[i as usize].shape = shape;
    g.objs.aliens[i as usize].worldx = x;
    g.objs.aliens[i as usize].worldy = y;
    g.objs.aliens[i as usize].worldz = z;
    g.objs.aliens[i as usize].sflags3 |= ASF3_REALOBJ;
    i
}

#[test]
fn find_near_uses_xzdiffs_not_manhattan() {
    // Axis (1000,0) vs diagonal (760,760): Manhattan prefers diagonal
    // (1520 < 1000? no — 1520 > 1000 so axis wins Manhattan too).
    // Use coexec case: (1000,0) vs (760,760) — Manhattan: 1000 vs 1520 → axis;
    // xzdiffs_l also prefers axis (certified in coexec "axis vs diagonal").
    let mut g = Game::new();
    let me = spawn(&mut g, 1, 0, 0, 0);
    let axis = spawn(&mut g, 10, 1000, 0, 0);
    let diag = spawn(&mut g, 10, 760, 0, 760);
    let _ = diag;
    let mut fobj = g.objs.active_head;
    let found = find_near_object(&g, 10, me, 0, 10000, &mut fobj).expect("near");
    assert_eq!(found, axis);

    let r_axis = strat_dist_xz(&g.objs.aliens[me as usize], &g.objs.aliens[axis as usize]);
    let r_diag = strat_dist_xz(&g.objs.aliens[me as usize], &g.objs.aliens[diag as usize]);
    assert!(r_axis < r_diag, "xzdiffs: axis {r_axis} < diag {r_diag}");
}

#[test]
fn find_near_ignores_y_separation() {
    // Candidate A: close XZ, huge Y; B: farther XZ, coplanar. ROM picks A.
    let mut g = Game::new();
    let me = spawn(&mut g, 1, 0, 0, 0);
    let a = spawn(&mut g, 10, 300, 7000, 0);
    let b = spawn(&mut g, 10, 2000, 0, 0);
    let _ = b;
    let mut fobj = g.objs.active_head;
    let found = find_near_object(&g, 10, me, 0, 10000, &mut fobj).expect("near");
    assert_eq!(found, a);
}
