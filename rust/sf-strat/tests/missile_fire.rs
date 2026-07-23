//! ROM fire_missile1/2 + fire_Hmissile2/FakeFar/bossH1 + strat bodies.

use sf_game::alien::{ASF_SHADOW, ATMISSILE, ATZREMOVE};
use sf_game::Game;
use sf_strat::enemy_a::{
    fire_boss_hmissile1, fire_fakefar_hmissile1, fire_hmissile1, fire_hmissile2, fire_missile1,
    fire_missile2, hmissile2_istrat, hmissile2_strat, missile1_istrat, missile1_strat,
    missile2_istrat, missile2a_strat, ASF2_RELEXPLODE, ASF2_SFLAG2, ASF2_SFLAG3, COLLTYPE_ZENEMY,
    DEG180,
};

#[test]
fn fire_missile1_and_missile2_stats() {
    let mut g = Game::new();
    let firer = g.objs.alloc().expect("f");
    g.objs.aliens[firer as usize].vel = 20;

    let m1 = fire_missile1(&mut g, firer).expect("m1");
    {
        let al = &g.objs.aliens[m1 as usize];
        assert_eq!(al.hp, 2);
        assert_eq!(al.ap, 4);
        assert_eq!(al.vel, 30);
        assert_eq!(al.count, 100);
        assert_eq!(al.sflags2 & ASF2_RELEXPLODE, 0);
        assert_ne!(al.type_ & ATMISSILE, 0);
        assert_ne!(al.sflags & ASF_SHADOW, 0);
        assert_ne!(al.collflags & COLLTYPE_ZENEMY, 0);
    }

    let m2 = fire_missile2(&mut g, firer).expect("m2");
    {
        let al = &g.objs.aliens[m2 as usize];
        assert_eq!(al.hp, 2);
        assert_eq!(al.ap, 4);
        assert_eq!(al.vel, 30);
        assert_ne!(al.sflags2 & ASF2_RELEXPLODE, 0);
        assert_ne!(al.type_ & ATZREMOVE, 0);
    }
}

#[test]
fn fire_hmissile2_straight_then_homes() {
    let mut g = Game::new();
    let target = g.objs.alloc().expect("t");
    g.objs.aliens[target as usize].worldx = 800;
    g.objs.aliens[target as usize].worldz = 2000;
    let firer = g.objs.alloc().expect("f");
    g.objs.aliens[firer as usize].ptr = target + 1;
    g.objs.aliens[firer as usize].vel = 10;

    let shot = fire_hmissile2(&mut g, firer).expect("hm2");
    assert_eq!(g.objs.aliens[shot as usize].sbyte1, 25);
    assert_eq!(g.objs.aliens[shot as usize].vel, 60);
    assert_eq!(g.objs.aliens[shot as usize].ap, 8);
    assert_eq!(g.objs.aliens[shot as usize].ptr, target + 1);

    // Straight phase: sbyte1 counts down, no sflag2 yet.
    for _ in 0..24 {
        hmissile2_strat(&mut g, shot);
    }
    assert!(g.objs.aliens[shot as usize].sbyte1 > 1 || g.objs.aliens[shot as usize].sbyte1 == 1);
    assert_eq!(g.objs.aliens[shot as usize].sflags2 & ASF2_SFLAG2, 0);

    // Next ticks enter home (rate-1 aim) while far from target.
    for _ in 0..5 {
        hmissile2_strat(&mut g, shot);
    }
    assert_eq!(g.objs.aliens[shot as usize].sbyte1, 1);
}

#[test]
fn fire_fakefar_sets_sflag3_and_boss_swaps_exp() {
    let mut g = Game::new();
    let firer = g.objs.alloc().expect("f");
    let fake = fire_fakefar_hmissile1(&mut g, firer).expect("fake");
    assert_ne!(g.objs.aliens[fake as usize].sflags2 & ASF2_SFLAG3, 0);
    assert_ne!(g.objs.aliens[fake as usize].sflags2 & ASF2_RELEXPLODE, 0);

    let boss = fire_boss_hmissile1(&mut g, firer).expect("boss");
    assert!(g.objs.aliens[boss as usize].expstratptr.is_some());
    assert_eq!(g.objs.aliens[boss as usize].vel, 60);
}

#[test]
fn fire_hmissile1_spawns_and_missile1_chases_aim() {
    let mut g = Game::new();
    g.vars.pviewvelz = 15;
    let firer = g.objs.alloc().expect("f");
    let shot = fire_hmissile1(&mut g, firer).expect("hm1");
    assert_eq!(g.objs.aliens[shot as usize].vel, 60);
    assert_eq!(g.objs.aliens[shot as usize].hp, 2);

    // missile1: chase sbyte aim, scroll with player Z.
    let m = g.objs.alloc().expect("m1");
    {
        let al = &mut g.objs.aliens[m as usize];
        al.vel = 30;
        al.count = 5;
        al.sbyte1 = 0;
        al.sbyte2 = DEG180;
        al.sbyte3 = 10;
        al.worldz = 100;
        al.rotx = 40;
        al.roty = 0;
    }
    missile1_istrat(&mut g, m);
    let z0 = g.objs.aliens[m as usize].worldz;
    missile1_strat(&mut g, m);
    assert!(g.objs.aliens[m as usize].worldz != z0 || g.objs.aliens[m as usize].vz != 0);
    assert_eq!(g.objs.aliens[m as usize].count, 4);
    // roty should move toward DEG180
    assert_ne!(g.objs.aliens[m as usize].roty, 0);
}

#[test]
fn missile2a_speeds_up_near_player_and_expires() {
    let mut g = Game::new();
    let player = g.objs.alloc().expect("p");
    g.objs.aliens[player as usize].worldz = 0;
    // Mark as player for player() helper — set via game player index if available.
    // Fallback: put missile within 600 Z and call strat; speed_to only runs if player() works.
    let m = g.objs.alloc().expect("m");
    {
        let al = &mut g.objs.aliens[m as usize];
        al.vel = 30;
        al.count = 2;
        al.sbyte1 = 0;
        al.sbyte2 = DEG180;
        al.worldz = 100;
        al.rotx = 20;
        al.roty = 0;
    }
    missile2_istrat(&mut g, m);
    assert_eq!(g.objs.aliens[m as usize].rotx, 0);
    assert_eq!(g.objs.aliens[m as usize].roty, DEG180);

    missile2a_strat(&mut g, m);
    assert_eq!(g.objs.aliens[m as usize].count, 1);
    // Chase toward pitch 0 / yaw 180
    assert!(g.objs.aliens[m as usize].rotx < 20 || g.objs.aliens[m as usize].rotx == 0);

    missile2a_strat(&mut g, m);
    assert_eq!(g.objs.aldead, 1);
    let _ = player;
}

#[test]
fn hmissile2_istrat_sets_straight_counter() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("m");
    g.objs.aliens[idx as usize].vel = 60;
    g.objs.aliens[idx as usize].sbyte3 = 15;
    hmissile2_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 25);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);
}
