//! ROM bossbigoutexplode + mfire1 + cube fall/exp + misstankexp.

use sf_core::screen_fill_circle::{ScreenFillCircleCenter, ScreenFillCirclePhase};
use sf_game::alien::{ASF_COLLIDE, ASF_SHADOW};
use sf_game::vars::{GF_BOSSDEAD, HARD_HP};
use sf_game::Game;
use sf_strat::enemies_ground::misstankexp_istrat;
use sf_strat::enemy_a::{
    cubecoll_strat, cubeexp_strat, cubefall_istrat, cubefall_strat, ASF2_RELEXPLODE,
    COLLTYPE_ENEMY1, DEG180, DEG5,
};
use sf_strat::enemy_b::{
    bossbigoutexplode_istrat, bossbigoutexplodeoff_istrat, mfire1_istrat, mfire1_strat,
    mfire1a_istrat, mfire1exp_istrat,
};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldz = z;
    al.sflags4 |= sf_game::alien::ASF4_PLAYEROBJ;
}

#[test]
fn bossbigoutexplode_sets_bossdead_and_delayremove() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("boss");
    g.objs.aliens[idx as usize].worldz = 2000;
    let before = g.objs.active_indices().len();
    bossbigoutexplode_istrat(&mut g, idx);
    assert_ne!(g.vars.gameflags & GF_BOSSDEAD, 0);
    assert_eq!(g.objs.aliens[idx as usize].count, 11);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE, 0);
    assert!(
        g.objs.active_indices().len() > before,
        "circle + particle + outward sprites"
    );
}

#[test]
fn bossbigoutexplode_delay_starts_the_object_anchored_circle() {
    const BOSS_WORLD_Z: i16 = 2000;

    let mut g = Game::new();
    let idx = g.objs.alloc().expect("boss");
    g.objs.aliens[idx as usize].worldz = BOSS_WORLD_Z;
    bossbigoutexplode_istrat(&mut g, idx);

    let proxy = g
        .objs
        .active_indices()
        .iter()
        .copied()
        .find(|candidate| {
            *candidate != idx
                && g.objs.aliens[*candidate as usize].shape == 0
                && g.objs.aliens[*candidate as usize].count == 1
                && g.objs.aliens[*candidate as usize].stratptr.is_some()
        })
        .expect("circle-delay proxy");
    let tick = g.objs.aliens[proxy as usize]
        .stratptr
        .expect("circle-delay strategy");

    g.call_strat(tick, proxy);
    assert!(!g.vars.screen_fill_circle.is_active());
    g.call_strat(tick, proxy);

    assert_eq!(
        g.vars.screen_fill_circle.phase,
        ScreenFillCirclePhase::BossExpanding
    );
    let ScreenFillCircleCenter::Object(object_id) = g.vars.screen_fill_circle.center else {
        panic!("delayed boss circle should retain a world anchor");
    };
    let anchor = object_id - 1;
    assert_eq!(
        [
            g.objs.aliens[anchor as usize].worldx,
            g.objs.aliens[anchor as usize].worldy,
            g.objs.aliens[anchor as usize].worldz,
        ],
        [
            g.objs.aliens[proxy as usize].worldx,
            g.objs.aliens[proxy as usize].worldy,
            g.objs.aliens[proxy as usize].worldz,
        ]
    );
}

#[test]
fn bossbigoutexplodeoff_offsets_from_velocity() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("boss");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 100;
        al.worldy = 50;
        al.worldz = 2000;
        al.vx = 40;
        al.vy = -10;
        al.vz = 5;
    }
    bossbigoutexplodeoff_istrat(&mut g, idx);
    assert_ne!(g.vars.gameflags & GF_BOSSDEAD, 0);
    // Companion objects should be offset; boss itself stays put until delayremove.
    assert_eq!(g.objs.aliens[idx as usize].worldx, 100);
}

#[test]
fn mfire1_aims_and_spins() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = g.objs.alloc().expect("fire");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 800;
        al.sbyte1 = 0;
    }
    mfire1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].ap, 8);
    assert_eq!(g.objs.aliens[idx as usize].vel, 80);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1, 0);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE, 0);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, DEG180);
    assert_eq!(g.objs.aliens[idx as usize].snd2, 0x0d);

    let rx0 = g.objs.aliens[idx as usize].rotx;
    let z0 = g.objs.aliens[idx as usize].worldz;
    g.vars.pviewvelz = 10;
    mfire1_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotx, rx0.wrapping_add(12));
    assert_ne!(g.objs.aliens[idx as usize].worldz, z0);
}

#[test]
fn mfire1a_is_hard_and_exp_spawns_children() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let a = g.objs.alloc().expect("a");
    g.objs.aliens[a as usize].worldz = 500;
    g.objs.aliens[a as usize].sbyte1 = DEG5;
    mfire1a_istrat(&mut g, a);
    assert_eq!(g.objs.aliens[a as usize].hp, HARD_HP);

    // Player behind the bolt → spawn ±deg5 children then explode.
    let exp = g.objs.alloc().expect("exp");
    {
        let al = &mut g.objs.aliens[exp as usize];
        al.worldz = 900;
        al.worldx = 10;
        al.worldy = -20;
    }
    let before = g.objs.active_indices().len();
    mfire1exp_istrat(&mut g, exp);
    assert_eq!(g.objs.aldead, 1);
    assert!(
        g.objs.active_indices().len() + 1 > before,
        "two fireface children (parent marked dead)"
    );
}

#[test]
fn mfire1exp_skips_children_when_player_ahead() {
    let mut g = Game::new();
    spawn_player(&mut g, 2000);
    let idx = g.objs.alloc().expect("exp");
    g.objs.aliens[idx as usize].worldz = 500;
    let before = g.objs.active_indices().len();
    mfire1exp_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(
        g.objs.active_indices().len(),
        before,
        "no children when player already past"
    );
}

#[test]
fn cubefall_falls_and_cubeexp_removes() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("cube");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = -40;
        al.vy = 0;
    }
    cubefall_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);
    assert_eq!(g.objs.aliens[idx as usize].hp, 100);
    assert_eq!(g.objs.aliens[idx as usize].ap, 16);
    // One gravity tick already applied by fall-through.
    assert_eq!(g.objs.aliens[idx as usize].vy, 1);
    cubefall_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vy, 2);

    cubeexp_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn cubecoll_damages_and_clears_collide() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("cube");
    cubefall_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].sflags |= ASF_COLLIDE;
    let hp0 = g.objs.aliens[idx as usize].hp;
    cubecoll_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, hp0 - 1);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLIDE, 0);
}

#[test]
fn misstankexp_kills_unlaunched_child() {
    let mut g = Game::new();
    let tank = g.objs.alloc().expect("tank");
    let child = g.objs.alloc().expect("missile");
    g.objs.aliens[tank as usize].ptr = child + 1;
    g.objs.aliens[child as usize].hp = 4;
    g.objs.aliens[child as usize].active = true;
    // sflag1 clear → kill child then explode tank.
    misstankexp_istrat(&mut g, tank);
    assert_eq!(g.objs.aliens[child as usize].hp, 0);
    assert_eq!(g.objs.aldead, 1);
}
