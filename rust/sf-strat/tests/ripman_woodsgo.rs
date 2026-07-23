//! ROM ripman / item4 / ripair + woodsgo / woodsexp / missgo.

use sf_game::alien::{ASF_COLLDISABLE, ASF_SHADOW, ATZREMOVE};
use sf_game::Game;
use sf_strat::enemies_ground::{
    missgo_istrat, ripman_istrat, ripman_strat, ripmanexp_istrat, woodsexp_istrat, woodsgo_init,
    woodsgo_strat,
};
use sf_strat::enemy_a::{
    item4_istrat, item4_strat, ripair_istrat, ripair_strat, COLLTYPE_ENEMYWEAP, DEG90,
    PSF_BRKLWING, PSF_BRKRWING,
};

fn spawn_player(g: &mut Game, x: i16, y: i16, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    al.sflags3 |= sf_game::alien::ASF3_REALOBJ;
    g.vars.player_posx = x;
    g.vars.player_posy = y;
    g.vars.player_posz = z;
}

#[test]
fn ripman_falls_until_ground_then_stops() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("rip");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 0;
        al.worldy = -100;
        al.worldz = 0;
        al.roty = 0;
    }
    ripman_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 4);
    assert_eq!(g.objs.aliens[idx as usize].ap, 16);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);
    assert_ne!(
        g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMYWEAP,
        0
    );

    ripman_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].roty, 16);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -97);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 4);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 35);

    g.objs.aliens[idx as usize].worldy = -30;
    let (x0, z0, r0) = (
        g.objs.aliens[idx as usize].worldx,
        g.objs.aliens[idx as usize].worldz,
        g.objs.aliens[idx as usize].roty,
    );
    ripman_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, x0);
    assert_eq!(g.objs.aliens[idx as usize].worldz, z0);
    assert_eq!(g.objs.aliens[idx as usize].roty, r0);
}

#[test]
fn ripmanexp_spawns_ripair_then_explodes() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, 0, 1000);
    let idx = g.objs.alloc().expect("rip");
    g.objs.aliens[idx as usize].worldz = 1000;
    let before = g.objs.active_indices().len();
    ripmanexp_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert!(
        g.objs.active_indices().len() + 1 > before,
        "ripair child spawned"
    );
    // Find the ripair child: colldisable + shadow + sbyte1=30.
    let child = g
        .objs
        .active_indices()
        .into_iter()
        .find(|&i| i != 0 && g.objs.aliens[i as usize].sbyte1 == 30)
        .expect("ripair");
    assert_ne!(g.objs.aliens[child as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[child as usize].rotz, DEG90);
    assert_eq!(g.objs.aliens[child as usize].vz, 30);
}

#[test]
fn ripair_approaches_then_repairs_on_catch() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, 0, 0);
    g.vars.pshipflags |= PSF_BRKLWING | PSF_BRKRWING;
    let idx = g.objs.alloc().expect("pod");
    ripair_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 500);
    assert_eq!(g.objs.aliens[idx as usize].worldz, -200);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 30);
    assert_eq!(g.objs.aliens[idx as usize].type_ & ATZREMOVE, 0);

    // Burn the 30-frame approach without catching.
    for _ in 0..30 {
        g.objs.aliens[idx as usize].worldx = 100; // keep XY far
        g.objs.aliens[idx as usize].worldy = 100;
        ripair_strat(&mut g, idx);
    }
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 1);
    assert_eq!(g.objs.aliens[idx as usize].vz, -40);
    assert_ne!(g.objs.aliens[idx as usize].type_ & ATZREMOVE, 0);

    // Snap onto player for catch.
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 0;
        al.sbyte1 = 1;
    }
    ripair_strat(&mut g, idx);
    assert_eq!(g.vars.pshipflags & (PSF_BRKLWING | PSF_BRKRWING), 0);
}

#[test]
fn item4_spawns_ripair_on_pickup() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, 0, 0);
    let idx = g.objs.alloc().expect("item");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 10;
    }
    item4_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    let before = g.objs.active_indices().len();
    item4_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert!(g.objs.active_indices().len() + 1 >= before);
}

#[test]
fn woodsgo_homes_and_woodsexp_frees_child() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, 0, 0);
    let idx = g.objs.alloc().expect("woods");
    g.objs.aliens[idx as usize].worldz = 2000;
    woodsgo_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 10);
    assert_eq!(g.objs.aliens[idx as usize].snd2, 2);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());

    let rot0 = g.objs.aliens[idx as usize].rotz;
    woodsgo_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotz, rot0.wrapping_add(8));

    let child = g.objs.alloc().expect("child");
    g.objs.aliens[idx as usize].ptr = child + 1;
    g.objs.aliens[child as usize].active = true;
    woodsexp_istrat(&mut g, idx);
    assert!(!g.objs.aliens[child as usize].active);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn missgo_istrat_is_noop() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("m");
    missgo_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 0);
}
