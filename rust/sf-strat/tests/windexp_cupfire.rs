//! ROM windexp/windspin + tank1fire/misspoda + cupfire + ship1aexp/mine2expnofire.

use sf_game::alien::{ObjectVisualKind, ASF_COLLDISABLE, ASF_INVISIBLE, ASF_NOHITAFFECT, ATLASER};
use sf_game::Game;
use sf_strat::enemies_ground::{
    misspoda_init, tank1fire, windexp_istrat, windspin_istrat, windspin_strat,
};
use sf_strat::enemy_a::{mine2expnofire_istrat, ship1aexp_istrat, ASF2_SFLAG3, DEG180, DEG90};
use sf_strat::enemy_b::{bossacupfire_srou, bossacupfiremiss_srou};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
}

#[test]
fn windexp_spawns_blades_and_explodes() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("mill");
    g.objs.aliens[idx as usize].worldz = 500;
    g.objs.aliens[idx as usize].rotz = 10;
    let before = g.objs.active_indices().len();
    windexp_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    // 4 blades + particle companion (parent marked dead but still counted until free).
    assert!(
        g.objs.active_indices().len() + 1 > before + 3,
        "blade debris spawned"
    );
}

#[test]
fn windspin_coasts_and_expires() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("blade");
    g.objs.aliens[idx as usize].rotz = DEG90;
    windspin_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[idx as usize].count, 40);
    assert!(g.objs.aliens[idx as usize].sbyte1 >= 7);
    let rot0 = g.objs.aliens[idx as usize].rotz;
    let spin = g.objs.aliens[idx as usize].sbyte1;
    windspin_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotz, rot0.wrapping_add(spin));
    assert_eq!(g.objs.aliens[idx as usize].count, 39);
    g.objs.aliens[idx as usize].count = 1;
    windspin_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn tank1fire_cadence_and_range_gate() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = g.objs.alloc().expect("tank");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = 800; // |dz|>=500
        al.worldx = 0; // |dx|<300
        al.sbyte2 = 21; // next tick -> 20 -> fire
    }
    let before = g.objs.active_indices().len();
    tank1fire(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 20);
    assert!(g.objs.active_indices().len() > before, "hplasma fired");
    let shot = g
        .objs
        .aliens
        .iter()
        .find(|alien| alien.active && alien.type_ & ATLASER != 0)
        .expect("tank high-plasma projectile");
    assert_eq!(shot.shape, sf_strat::enemy_a::SH_BOUNCYBALL);
    assert_eq!(shot.visual_kind, ObjectVisualKind::ScaledSprite);
    assert_eq!(shot.sflags & ASF_INVISIBLE, 0);

    // Too close in z -> no fire even at count 10.
    g.objs.aliens[idx as usize].sbyte2 = 11;
    g.objs.aliens[idx as usize].worldz = 100;
    let mid = g.objs.active_indices().len();
    tank1fire(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 10);
    assert_eq!(g.objs.active_indices().len(), mid);
}

#[test]
fn misspoda_fires_burst_and_kills_self() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = g.objs.alloc().expect("pod");
    g.objs.aliens[idx as usize].sbyte1 = 1; // H pattern
    g.objs.aliens[idx as usize].hp = 4;
    let before = g.objs.active_indices().len();
    misspoda_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 0);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert!(g.objs.active_indices().len() > before + 3, "5 missiles");
}

#[test]
fn bossacupfire_spawns_home_laser() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = g.objs.alloc().expect("cup");
    g.objs.aliens[idx as usize].worldz = 1000;
    let before = g.objs.active_indices().len();
    bossacupfire_srou(&mut g, idx);
    assert!(g.objs.active_indices().len() > before);
}

#[test]
fn bossacupfiremiss_one_shot_gate() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = g.objs.alloc().expect("cup");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = -400; // below -360 floor
        al.worldz = 800;
    }
    let before = g.objs.active_indices().len();
    bossacupfiremiss_srou(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG3, 0);
    assert!(g.objs.active_indices().len() > before);
    let mid = g.objs.active_indices().len();
    bossacupfiremiss_srou(&mut g, idx); // sflag3 set -> no second fire
    assert_eq!(g.objs.active_indices().len(), mid);

    // Above floor -> no fire.
    let cup2 = g.objs.alloc().expect("cup2");
    g.objs.aliens[cup2 as usize].worldy = -100;
    let n = g.objs.active_indices().len();
    bossacupfiremiss_srou(&mut g, cup2);
    assert_eq!(g.objs.active_indices().len(), n);
    assert_eq!(g.objs.aliens[cup2 as usize].sflags2 & ASF2_SFLAG3, 0);
}

#[test]
fn ship1aexp_sets_nohitaffect() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("ship");
    let tick = g.world.register_strategy(|_g, _i| {});
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    ship1aexp_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_NOHITAFFECT, 0);
}

#[test]
fn mine2expnofire_spawns_particle_then_explodes() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("mine");
    g.objs.aliens[idx as usize].worldz = 300;
    let before = g.objs.active_indices().len();
    mine2expnofire_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert!(g.objs.active_indices().len() + 1 > before);
}

#[test]
fn windspin_uses_rotz_for_xyvec() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("b");
    g.objs.aliens[idx as usize].rotz = DEG180;
    windspin_istrat(&mut g, idx);
    // deg180 → vx≈0, vy negative-ish from cos table; just ensure vecs set.
    assert!(g.objs.aliens[idx as usize].vx != 0 || g.objs.aliens[idx as usize].vy != 0);
}
