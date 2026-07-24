//! ROM explode_end / explode_strat / lexplode + bazexp/bazfall + headfire.

use sf_game::alien::ASF_COLLDISABLE;
use sf_game::Game;
use sf_strat::enemies_ground::{bazexp_istrat, bazfall_istrat, bazfall_strat, SH_BAZOOKA2};
use sf_strat::enemy_a::{
    explode_end, explode_strat, headfire_istrat, headfire_strat, lexplode_strat, ASF2_RELEXPLODE,
    DEG45,
};

#[test]
fn explode_end_removes_when_count_ge_count1() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("e");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.count = 5;
        al.count1 = 6;
        al.vx = 3;
        al.worldx = 0;
    }
    explode_end(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 3);
    assert_eq!(g.objs.aldead, 0);

    g.objs.aliens[idx as usize].count = 6;
    explode_end(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn explode_strat_advances_and_relexplode_scrolls() {
    let mut g = Game::new();
    g.vars.pviewvelz = 20;
    let idx = g.objs.alloc().expect("e");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.count = 0;
        al.count1 = 3;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.worldz = 100;
        al.colframe = 0x80;
    }
    let z0 = g.objs.aliens[idx as usize].worldz;
    explode_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 1);
    assert_ne!(g.objs.aliens[idx as usize].colframe & 0x7F, 0);
    assert_eq!(g.objs.aliens[idx as usize].worldz, z0.wrapping_add(20));
}

#[test]
fn lexplode_skips_anim_on_odd_frames() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("e");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.count = 0;
        al.count1 = 10;
        al.colframe = 0x80;
    }
    g.vars.gameframe = 1; // odd → skip anim
    lexplode_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 0);
    assert_eq!(g.objs.aliens[idx as usize].colframe & 0x7F, 0);

    g.vars.gameframe = 0; // even → animate
    lexplode_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 1);
}

#[test]
fn bazexp_spawns_fall_debris() {
    let mut g = Game::new();
    let baz = g.objs.alloc().expect("b");
    g.objs.aliens[baz as usize].worldx = 100;
    g.objs.aliens[baz as usize].worldy = -25;
    g.objs.aliens[baz as usize].worldz = 500;
    g.objs.aliens[baz as usize].rotx = 10;
    g.objs.aliens[baz as usize].roty = 20;
    g.objs.aliens[baz as usize].rotz = 30;
    g.objs.aliens[baz as usize].sword1 = SH_BAZOOKA2 as i16;
    let before = g.objs.active_indices().len();
    bazexp_istrat(&mut g, baz);
    let after = g.objs.active_indices().len();
    assert!(after > before, "debris child spawned");
    let barrel = g
        .objs
        .aliens
        .iter()
        .find(|alien| alien.active && alien.shape == SH_BAZOOKA2)
        .expect("authored barrel mesh");
    assert_eq!(
        (barrel.worldx, barrel.worldy, barrel.worldz),
        (100, -25, 500)
    );
    assert_eq!((barrel.rotx, barrel.roty, barrel.rotz), (10, 20, 30));
    assert_eq!(barrel.count, 30);
    assert_ne!(barrel.sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn bazfall_tumbles_and_expires() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("f");
    bazfall_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 30);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    let rot0 = g.objs.aliens[idx as usize].roty;
    bazfall_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].roty, rot0.wrapping_add(16));
    assert_eq!(g.objs.aliens[idx as usize].count, 29);
    g.objs.aliens[idx as usize].count = 1;
    bazfall_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn headfire_falls_then_dashes() {
    let mut g = Game::new();
    let _player = g.objs.alloc().expect("p");
    g.objs.aliens[0].worldx = 200;
    g.objs.aliens[0].worldz = 1000;
    let idx = g.objs.alloc().expect("h");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = -50;
        al.worldz = 0;
        al.vy = 0;
    }
    headfire_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 6);
    assert_eq!(g.objs.aliens[idx as usize].ap, 10);

    // Fall until ground.
    for _ in 0..20 {
        if g.objs.aliens[idx as usize].sbyte1 == 1 {
            break;
        }
        headfire_strat(&mut g, idx);
    }
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 1);
    assert_eq!(g.objs.aliens[idx as usize].worldy, 0);
    assert_eq!(g.objs.aliens[idx as usize].vel, 120);
    assert_ne!(g.objs.aliens[idx as usize].roty, 0);

    let rx0 = g.objs.aliens[idx as usize].rotx;
    headfire_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotx, rx0.wrapping_add(DEG45));
}
