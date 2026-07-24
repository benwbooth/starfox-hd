//! ROM fire_spread / spread_* / spreada_init + DefElaserCol.

use sf_game::alien::{
    ObjectVisualKind, ACF_COLLTYPE4, ACF_COLLTYPE5, ACF_WEAPON, AFEXP, ASF_COLLIDE,
};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::enemy_a::{
    defelasercol_istrat, fire_elaser, fire_spread, spread_istrat, spread_strat, spreada_init,
    ASF2_RELEXPLODE,
};

const SHAPE_ELASER2: u16 = 511;
const GENERIC_EXPLOSION_POLYGON_TICKS: u8 = 12;

#[test]
fn fire_spread_stats_and_arm_countdown() {
    let mut g = Game::new();
    let firer = g.objs.alloc().expect("f");
    g.objs.aliens[firer as usize].vel = 15;

    let shot = fire_spread(&mut g, firer).expect("spread");
    {
        let al = &g.objs.aliens[shot as usize];
        assert_eq!(al.hp, 2);
        assert_eq!(al.ap, 4);
        assert_eq!(al.vel, 40);
        assert_eq!(al.count, 50);
        assert_eq!(al.sbyte3, 10);
        assert_eq!(al.sflags2 & ASF2_RELEXPLODE, 0);
        assert_ne!(al.collflags & ACF_COLLTYPE5, 0); // friend
        assert_ne!(al.collflags & ACF_COLLTYPE4, 0); // enemyweap
    }

    // 10 coast ticks: sbyte3 10→0, still not exploded.
    for _ in 0..10 {
        spread_strat(&mut g, shot);
    }
    assert_eq!(g.objs.aliens[shot as usize].sbyte3, 0);
    assert!(g.objs.aliens[shot as usize].active);

    // Next tick: beqdec hits 0 -> spreada_init -> generic explosion pair.
    spread_strat(&mut g, shot);
    let polygon = g.objs.aliens[shot as usize];
    assert_ne!(polygon.flags & AFEXP, 0);
    assert_eq!(polygon.count, 0);
    assert_eq!(polygon.count1, GENERIC_EXPLOSION_POLYGON_TICKS);
    assert_eq!(g.objs.aldead, 0);
    assert!(g.objs.active_indices().into_iter().any(|slot| {
        slot != shot && g.objs.aliens[slot as usize].visual_kind == ObjectVisualKind::ScaledSprite
    }));
}

#[test]
fn spreada_fires_qh_at_valid_targets() {
    let mut g = Game::new();
    let mother = g.objs.alloc().expect("m");
    {
        let al = &mut g.objs.aliens[mother as usize];
        al.vel = 40;
        al.count = 50;
        al.sbyte3 = 0;
    }
    spread_istrat(&mut g, mother);

    // Valid enemy target.
    let enemy = g.objs.alloc().expect("e");
    g.objs.aliens[enemy as usize].hp = 4;
    g.objs.aliens[enemy as usize].worldz = 500;

    // Skipped: weapon flag.
    let weap = g.objs.alloc().expect("w");
    g.objs.aliens[weap as usize].hp = 2;
    g.objs.aliens[weap as usize].collflags |= ACF_WEAPON;

    // Skipped: hardHP.
    let hard = g.objs.alloc().expect("h");
    g.objs.aliens[hard as usize].hp = HARD_HP;

    let before = g.objs.active_indices().len();
    spreada_init(&mut g, mother);
    let after = g.objs.active_indices().len();
    // At least one QH missile spawned for the valid enemy; mother marked dead.
    assert!(after >= before); // mother may still be in list until free
    assert_eq!(g.objs.aldead, 1);
    // Find a QH (ap=50) aimed at enemy.
    let mut found_qh = false;
    for &i in &g.objs.active_indices() {
        let al = &g.objs.aliens[i as usize];
        if al.ap == 50 && al.ptr == enemy + 1 {
            found_qh = true;
            break;
        }
    }
    assert!(found_qh, "QHMISSILE1 should lock onto valid enemy");
}

#[test]
fn defelasercol_rebounds_elaser2() {
    let mut g = Game::new();
    let shield = g.objs.alloc().expect("s");
    // Harmless main strat so jmpto_strat after deflect is safe.
    spread_istrat(&mut g, shield);
    let laser = fire_elaser(&mut g, shield).expect("laser");
    g.objs.aliens[laser as usize].shape = SHAPE_ELASER2;
    g.objs.aliens[laser as usize].roty = 0;
    g.objs.aliens[laser as usize].rotx = 10;
    g.objs.aliens[laser as usize].vel = 66;

    g.objs.aliens[shield as usize].collobjptr = laser;
    g.objs.aliens[shield as usize].sflags |= ASF_COLLIDE;

    let before = g.objs.active_indices().len();
    defelasercol_istrat(&mut g, shield);
    let after = g.objs.active_indices().len();

    assert_eq!(g.objs.aliens[shield as usize].sflags & ASF_COLLIDE, 0);
    // Laser restored.
    assert_eq!(g.objs.aliens[laser as usize].roty, 0);
    assert_eq!(g.objs.aliens[laser as usize].rotx, 10);
    assert_eq!(g.objs.aliens[laser as usize].vel, 66);
    // Rebound shot spawned (RebElaser).
    assert!(after > before, "RebElaser should spawn from deflect");
}
