//! enemy_b lane unit tests: the table-lane registration contract and a
//! couple of exact single-strategy behaviours (title spin, boss maxhp seed).

use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::enemy_b::{
    self, install_enemy_b, register, IS_BOSSA, IS_BOSSF, IS_BOSS7, STRAT_ADDR_BOSSF,
    STRAT_ADDR_SPACEPILON, STRAT_ADDR_TIT,
};

fn spawn(g: &mut Game, shape: u16) -> u16 {
    let idx = g.objs.alloc().expect("pool");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].shape = shape;
    idx
}

#[test]
fn install_returns_valid_distinct_handles() {
    let mut g = Game::new();
    let ids = install_enemy_b(&mut g);
    let handles = [ids.boss7, ids.bossa, ids.bossf, ids.spacepilon, ids.tit];
    // Every handle resolves inside the registry.
    for h in handles {
        assert!((h.0 as usize) < g.world.strat_registry.len());
    }
    // All five entry points are distinct functions.
    let mut sorted: Vec<u16> = handles.iter().map(|h| h.0).collect();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 5, "istrat handles must be distinct");

    // Idempotent: a second install memoizes on identity.
    let ids2 = install_enemy_b(&mut g);
    assert_eq!(ids.boss7.0, ids2.boss7.0);
    assert_eq!(ids.tit.0, ids2.tit.0);
}

#[test]
fn register_populates_istrats_and_address_map() {
    let mut g = Game::new();
    register(&mut g.world);

    assert!(g.world.istrats[IS_BOSS7].is_some());
    assert!(g.world.istrats[IS_BOSSA].is_some());
    assert!(g.world.istrats[IS_BOSSF].is_some());

    assert!(g.world.find_strategy_address(STRAT_ADDR_SPACEPILON).is_some());
    assert!(g.world.find_strategy_address(STRAT_ADDR_TIT).is_some());
    // bossF is both an istrat row and an address-map entry.
    let addr = g.world.find_strategy_address(STRAT_ADDR_BOSSF);
    assert_eq!(addr, g.world.istrats[IS_BOSSF]);
}

#[test]
fn title_init_and_spin_are_exact() {
    // ROM tit_istrat (ENDSEQ.ASM:1799-1804) sets rotx=-17 (0xEF), roty=96,
    // rotz=0, then falls through into tit_strat (ENDSEQ.ASM:1805-1809), which
    // rolls al_rotz += 2 per frame (Z-axis roll, NOT the yaw spin an earlier
    // port used). The port uses a display pose rotated ~90 deg from those raw
    // bytes (rotx=48, roty=32) because its title camera is pinned static at
    // (0,0,0): under that camera the raw ROM pose renders broadside and the
    // roll would collapse edge-on to a vertical spike. See strat_title_init.
    let mut g = Game::new();
    let e = spawn(&mut g, 2);
    let sid = g.world.register_strategy(enemy_b::strat_title_init);
    g.objs.aliens[e as usize].stratptr = Some(sid);

    // First tick runs the init strat (installs the tick strat and, per the
    // ASM fall-through, already rolls the first +2).
    g.run_strategies();
    let al = g.objs.aliens[e as usize];
    assert_eq!(al.hp, 255);
    assert_eq!(al.ap, 0);
    assert_eq!(al.collflags, 0);
    assert_eq!(al.rotx, 48, "display pitch pose (static-camera compensation)");
    assert_eq!(al.roty, 32, "display yaw pose");
    assert_eq!(al.rotz, 2, "install frame falls through into tit_strat");

    for i in 2u8..=30 {
        g.run_strategies();
        let al = g.objs.aliens[e as usize];
        assert_eq!(al.rotz, i.wrapping_mul(2), "constant +2/frame roll");
        assert_eq!(al.roty, 32, "yaw never moves");
        assert_eq!(al.rotx, 48, "pitch never moves");
    }
}
