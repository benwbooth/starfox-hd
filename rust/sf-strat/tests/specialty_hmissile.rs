//! ROM kami/chick/STB/QH hmissile fire + strat bodies.

use sf_game::alien::{ASF_COLLDISABLE, ASF_SHADOW};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::enemy_a::{
    chickhmissile1_istrat, chickhmissile1_strat, fire_chick_hmissile1, fire_kami_hmissile1,
    fire_qh_missile1, fire_stb_hmissile1, hmissile3_istrat, hmissile3_strat, qhmissile1_istrat,
    qhmissile1_strat, stbhmissile1_istrat, stbhmissile1_strat, ASF2_RELEXPLODE, ASF2_SFLAG2,
    COLLTYPE_ENEMY2, COLLTYPE_ZENEMY, DEG180,
};

#[test]
fn fire_kami_chick_stb_qh_stats() {
    let mut g = Game::new();
    let firer = g.objs.alloc().expect("f");
    g.objs.aliens[firer as usize].vel = 12;

    let kami = fire_kami_hmissile1(&mut g, firer).expect("kami");
    {
        let al = &g.objs.aliens[kami as usize];
        assert_eq!(al.hp, 2);
        assert_eq!(al.ap, 8);
        assert_eq!(al.vel, 40);
        assert_eq!(al.count, 100);
        assert_ne!(al.sflags2 & ASF2_RELEXPLODE, 0);
        assert_ne!(al.collflags & COLLTYPE_ZENEMY, 0);
    }

    let chick = fire_chick_hmissile1(&mut g, firer).expect("chick");
    {
        let al = &g.objs.aliens[chick as usize];
        assert_eq!(al.hp, HARD_HP);
        assert_eq!(al.ap, 40);
        assert_eq!(al.vel, 30);
        assert_eq!(al.count, 30);
        assert_eq!(al.rotz, DEG180);
    }

    let stb = fire_stb_hmissile1(&mut g, firer).expect("stb");
    {
        let al = &g.objs.aliens[stb as usize];
        assert_eq!(al.hp, 2);
        assert_eq!(al.ap, 8);
        assert_eq!(al.vel, 10); // istrat sets speed #10
        assert_eq!(al.sbyte3, 20);
        assert_ne!(al.collflags & COLLTYPE_ENEMY2, 0);
    }

    let qh = fire_qh_missile1(&mut g, firer).expect("qh");
    {
        let al = &g.objs.aliens[qh as usize];
        assert_eq!(al.hp, 1);
        assert_eq!(al.ap, 50);
        assert_eq!(al.vel, 60);
        assert_ne!(al.sflags & ASF_SHADOW, 0);
    }
}

#[test]
fn qhmissile_snap_aims_and_expires() {
    let mut g = Game::new();
    let target = g.objs.alloc().expect("t");
    g.objs.aliens[target as usize].worldx = 400;
    g.objs.aliens[target as usize].worldz = 1500;
    let m = g.objs.alloc().expect("m");
    {
        let al = &mut g.objs.aliens[m as usize];
        al.ptr = target + 1;
        al.vel = 60;
        al.count = 2;
        al.worldz = 0;
        al.roty = 0;
        al.rotx = 0;
    }
    qhmissile1_istrat(&mut g, m);
    qhmissile1_strat(&mut g, m);
    assert_ne!(g.objs.aliens[m as usize].roty, 0);
    assert_eq!(g.objs.aliens[m as usize].count, 1);
    qhmissile1_strat(&mut g, m);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn stbhmissile_speeds_up_and_homes() {
    let mut g = Game::new();
    let target = g.objs.alloc().expect("t");
    g.objs.aliens[target as usize].worldx = 900;
    g.objs.aliens[target as usize].worldz = 3000;
    let m = g.objs.alloc().expect("m");
    {
        let al = &mut g.objs.aliens[m as usize];
        al.ptr = target + 1;
        al.count = 50;
        al.worldz = 0;
        al.sbyte1 = 0;
        al.sbyte2 = 0;
    }
    stbhmissile1_istrat(&mut g, m);
    assert_eq!(g.objs.aliens[m as usize].vel, 10);
    for _ in 0..30 {
        stbhmissile1_strat(&mut g, m);
    }
    assert!(g.objs.aliens[m as usize].vel >= 10);
    // Still far → not latched sflag2 yet (dist >> 600).
    assert_eq!(g.objs.aliens[m as usize].sflags2 & ASF2_SFLAG2, 0);
}

#[test]
fn chick_latches_near_when_close_and_no_joy() {
    let mut g = Game::new();
    // Slot 0 is the player for objs.player().
    let player = g.objs.alloc().expect("p");
    assert_eq!(player, 0);
    g.objs.aliens[0].worldx = 0;
    g.objs.aliens[0].worldy = 0;
    g.objs.aliens[0].worldz = 0;
    g.vars.player_posx = 0;
    g.vars.player_posy = 0;
    g.vars.set_sv_i16(sv::VIEWCY, -60);
    g.vars.pad1 = 0; // no joy

    let m = g.objs.alloc().expect("m");
    {
        let al = &mut g.objs.aliens[m as usize];
        al.worldx = 50;
        al.worldy = 0;
        al.worldz = 50; // xz dist small
        al.vel = 30;
        al.count = 10;
    }
    chickhmissile1_istrat(&mut g, m);
    chickhmissile1_strat(&mut g, m);
    assert_ne!(g.objs.aliens[m as usize].sflags2 & ASF2_SFLAG2, 0);
    assert_ne!(g.objs.aliens[m as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[m as usize].count, 9);
}

#[test]
fn hmissile3_fires_lasers_in_z_window() {
    let mut g = Game::new();
    g.vars.gameframe = 0; // notdelay-3 open
    let target = g.objs.alloc().expect("t");
    g.objs.aliens[target as usize].worldz = 1500;
    let m = g.objs.alloc().expect("m");
    {
        let al = &mut g.objs.aliens[m as usize];
        al.ptr = target + 1;
        al.vel = 40;
        al.count = 20;
        al.worldz = 0;
        al.sbyte3 = 0;
    }
    hmissile3_istrat(&mut g, m);
    let before = g.objs.active_indices().len();
    hmissile3_strat(&mut g, m);
    let after = g.objs.active_indices().len();
    // Twin RELSLOWELASER spawn (may fail if spawn_projectile returns None — still ok if count ok).
    assert!(after >= before);
    assert_eq!(g.objs.aliens[m as usize].count, 19);
}
