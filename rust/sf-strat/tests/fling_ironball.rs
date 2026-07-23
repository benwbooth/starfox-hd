//! ROM fire_ironball / fire_ironball2 / fire_ironball3 (fling variants).

use sf_game::Game;
use sf_strat::enemy_a::{
    fire_ironball, fire_ironball2, fire_ironball3, ironball_strat, wm, ASF2_SFLAG1, ASF2_SFLAG2,
    ASF2_SFLAG3, COLLTYPE_ENEMY1, DEG90,
};

#[test]
fn fire_ironball_sets_sflag3_and_muzzle() {
    let mut g = Game::new();
    let firer = g.objs.alloc().expect("f");
    {
        let al = &mut g.objs.aliens[firer as usize];
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 0;
        al.rotx = 0;
        al.roty = 0;
        al.rotz = 0;
    }
    let pitch0 = g.objs.aliens[firer as usize].rotx;
    let shot = fire_ironball(&mut g, firer).expect("ib1");
    {
        let al = &g.objs.aliens[shot as usize];
        assert_ne!(al.sflags2 & ASF2_SFLAG3, 0);
        assert_ne!(al.collflags & COLLTYPE_ENEMY1, 0);
        assert_eq!(al.ap, 6);
        // Muzzle with temporary -deg90 pitch maps (0,0,120<<2) onto Y.
        assert!(
            al.worldy.abs() > 100 || al.worldz.abs() > 100,
            "muzzle should offset, y={} z={}",
            al.worldy,
            al.worldz
        );
    }
    // Firer pitch restored then sintab nudge applied (may differ from pitch0).
    let _ = pitch0;
    let _ = DEG90;
}

#[test]
fn fire_ironball2_aims_sflag2_and_bumps_powerbuild() {
    let mut g = Game::new();
    let _player = g.objs.alloc().expect("p");
    g.objs.aliens[0].worldx = 300;
    g.objs.aliens[0].worldz = 2000;
    let firer = g.objs.alloc().expect("f");
    g.objs.aliens[firer as usize].worldz = 0;
    g.vars.write_ext8(wm::POWERBUILD, 5);

    let shot = fire_ironball2(&mut g, firer).expect("ib2");
    assert_ne!(g.objs.aliens[shot as usize].sflags2 & ASF2_SFLAG2, 0);
    assert_eq!(g.vars.read_ext8(wm::POWERBUILD), 6);
    // Aimed toward player (yaw nonzero with x offset).
    assert_ne!(g.objs.aliens[shot as usize].roty, 0);
}

#[test]
fn fire_ironball3_faster_sflag1() {
    let mut g = Game::new();
    let _player = g.objs.alloc().expect("p");
    g.objs.aliens[0].worldz = 1500;
    let firer = g.objs.alloc().expect("f");

    let shot = fire_ironball3(&mut g, firer).expect("ib3");
    {
        let al = &g.objs.aliens[shot as usize];
        assert_ne!(al.sflags2 & ASF2_SFLAG1, 0);
        // Base 96..103 + 20 → 116..123
        assert!(al.vel >= 116 && al.vel <= 123, "vel={}", al.vel);
    }
}

#[test]
fn ironball_sflag3_extra_x_chase() {
    let mut g = Game::new();
    let _player = g.objs.alloc().expect("p");
    g.objs.aliens[0].worldx = 100;
    g.objs.aliens[0].worldz = 0;
    let firer = g.objs.alloc().expect("f");
    g.objs.aliens[firer as usize].worldx = 0;
    g.objs.aliens[firer as usize].worldz = 0;
    g.objs.aliens[firer as usize].roty = 0;
    g.objs.aliens[firer as usize].rotx = 0;

    let shot = fire_ironball(&mut g, firer).expect("ib");
    assert_ne!(g.objs.aliens[shot as usize].sflags2 & ASF2_SFLAG3, 0);
    let x0 = g.objs.aliens[shot as usize].worldx;
    let z0 = g.objs.aliens[shot as usize].worldz;
    ironball_strat(&mut g, shot);
    // sflag3: multiple fchase steps + worldz -= 60
    assert!(
        g.objs.aliens[shot as usize].worldx != x0
            || g.objs.aliens[shot as usize].worldz != z0.wrapping_add(g.vars.pviewvelz as i16),
        "sflag3 should chase x and/or pull z"
    );
    // Stronger chase: at least several units toward player x=100
    assert!(
        g.objs.aliens[shot as usize].worldx >= x0.wrapping_add(2),
        "sflag3 multi-fchase toward player"
    );
}
