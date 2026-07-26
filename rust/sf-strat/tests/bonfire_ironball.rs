//! ROM fire_bonfire / bonfire_* + fire_ironball4 / ironball_* / ironballmissile.

use sf_game::alien::{ObjectVisualKind, ASF_COLLDISABLE, ASF_NOHITAFFECT, ASF_SHADOW};
use sf_game::vars::{HARD_AP, HARD_HP};
use sf_game::Game;
use sf_strat::enemy_a::{
    bonfire_strat, bonfire_trail_strat, fire_bonfire, fire_ironball4, ironball_istrat,
    ironball_strat, ironballmissile_istrat, ironballmissile_strat, ASF2_SFLAG1, ASF2_SFLAG2,
    COLLTYPE_ENEMY1,
};

#[test]
fn fire_bonfire_stats_and_trail() {
    let mut g = Game::new();
    let player = g.objs.alloc().expect("p");
    assert_eq!(player, 0);
    g.objs.aliens[0].worldx = 100;
    g.objs.aliens[0].worldz = 800;

    let firer = g.objs.alloc().expect("f");
    g.objs.aliens[firer as usize].worldx = 0;
    g.objs.aliens[firer as usize].worldz = 0;

    let ball = fire_bonfire(&mut g, firer).expect("bonfire");
    {
        let al = &g.objs.aliens[ball as usize];
        assert_eq!(al.hp, HARD_HP);
        assert_eq!(al.ap, HARD_AP);
        assert_eq!(al.vel, 120);
        assert_eq!(al.worldy, 0);
        assert_ne!(al.sflags & ASF_NOHITAFFECT, 0);
        assert_ne!(al.collflags & COLLTYPE_ENEMY1, 0);
        assert_eq!(al.visual_kind, ObjectVisualKind::ScaledSprite);
        assert_eq!(al.depthoffset, 0);
        assert_eq!(al.tx, 0);
        // Aimed toward player (yaw nonzero when player is offset in x).
        assert_ne!(al.roty, 0);
    }

    let before = g.objs.active_indices().len();
    bonfire_strat(&mut g, ball);
    let after = g.objs.active_indices().len();
    assert!(after > before, "trail spark should spawn");
}

#[test]
fn bonfire_trail_expires_after_10() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("t");
    g.objs.aliens[idx as usize].sbyte1 = 0;
    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
    // Drive via bonfire_istrat path: spawn trail through strat, or call trail strat directly.
    // Use fire_bonfire then tick mother once to get a trail, then age it.
    let firer = g.objs.alloc().expect("f");
    let ball = fire_bonfire(&mut g, firer).expect("b");
    bonfire_strat(&mut g, ball);
    let trail = g
        .objs
        .active_indices()
        .into_iter()
        .find(|&i| {
            i != ball && i != firer && g.objs.aliens[i as usize].sflags & ASF_COLLDISABLE != 0
        })
        .expect("trail");
    assert_eq!(
        g.objs.aliens[trail as usize].visual_kind,
        ObjectVisualKind::ScaledSprite
    );
    assert_eq!(g.objs.aliens[trail as usize].depthoffset, 1);
    assert_eq!(g.objs.aliens[trail as usize].tx, 0);
    for _ in 0..10 {
        bonfire_trail_strat(&mut g, trail);
    }
    assert!(g.objs.aliens[trail as usize].sbyte1 >= 10);
    bonfire_trail_strat(&mut g, trail);
    assert_eq!(g.objs.aldead, 1);
    let _ = idx;
}

#[test]
fn fire_ironball4_faster_and_aimed() {
    let mut g = Game::new();
    let _player = g.objs.alloc().expect("p");
    g.objs.aliens[0].worldx = 200;
    g.objs.aliens[0].worldz = 1000;
    let firer = g.objs.alloc().expect("f");

    let ball = fire_ironball4(&mut g, firer).expect("ib4");
    {
        let al = &g.objs.aliens[ball as usize];
        assert_eq!(al.hp, HARD_HP);
        assert_eq!(al.ap, 6);
        assert_ne!(al.sflags2 & ASF2_SFLAG1, 0);
        assert_ne!(al.sflags & ASF_SHADOW, 0);
        // Base 96..103 + 20 for sflag1 → 116..123
        assert!(al.vel >= 116 && al.vel <= 123, "vel={}", al.vel);
        assert_ne!(al.collflags & COLLTYPE_ENEMY1, 0);
        assert_eq!(al.visual_kind, ObjectVisualKind::ScaledSprite);
        assert_eq!(al.depthoffset, 0);
        assert_eq!(al.tx, 0);
    }
}

#[test]
fn ironball_falls_and_chases_player_x() {
    let mut g = Game::new();
    let _player = g.objs.alloc().expect("p");
    g.objs.aliens[0].worldx = 500;
    g.objs.aliens[0].worldz = 0;
    let idx = g.objs.alloc().expect("b");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 0;
        al.worldy = -100;
        al.worldz = 200;
        al.roty = 0;
        al.rotx = 0;
        al.vel = 100;
    }
    ironball_istrat(&mut g, idx);
    let x0 = g.objs.aliens[idx as usize].worldx;
    ironball_strat(&mut g, idx);
    // fchase toward player x=500 by 1
    assert_eq!(g.objs.aliens[idx as usize].worldx, x0.wrapping_add(1));
}

#[test]
fn ironballmissile_sprays_nine_when_close() {
    let mut g = Game::new();
    let _player = g.objs.alloc().expect("p");
    g.objs.aliens[0].worldz = 0;
    let m = g.objs.alloc().expect("m");
    {
        let al = &mut g.objs.aliens[m as usize];
        al.worldz = 500; // |dz|<1000
        al.worldx = 0;
        al.worldy = 0;
    }
    ironballmissile_istrat(&mut g, m);
    assert_eq!(g.objs.aliens[m as usize].hp, 6);
    assert_eq!(g.objs.aliens[m as usize].ap, 16);
    assert_eq!(
        g.objs.aliens[m as usize].visual_kind,
        ObjectVisualKind::ScaledSprite
    );
    assert_eq!(g.objs.aliens[m as usize].depthoffset, 0);
    assert_eq!(g.objs.aliens[m as usize].tx, 0);

    let before = g.objs.active_indices().len();
    ironballmissile_strat(&mut g, m);
    let after = g.objs.active_indices().len();
    // 9 ironballs spawned; mother killed (aldead or inactive).
    assert!(
        after >= before + 8,
        "expected ~9 ironballs, before={before} after={after}"
    );
    assert!(
        g.objs.aldead == 1 || !g.objs.aliens[m as usize].active,
        "missile should die after spray"
    );
}

#[test]
fn ironball_power_build_waits_below_threshold() {
    let mut g = Game::new();
    let _player = g.objs.alloc().expect("p");
    let idx = g.objs.alloc().expect("b");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 100;
        al.sflags2 |= ASF2_SFLAG2;
    }
    ironball_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG2;
    g.vars.shared.power_build = 127;
    let y_before = g.objs.aliens[idx as usize].worldy;
    ironball_strat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2, 0);
    assert_eq!(g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1, 0);
    assert_eq!(
        g.objs.aliens[idx as usize].worldy,
        y_before.wrapping_sub(100)
    );
}

#[test]
fn ironball_power_build_reaims_and_doubles_velocity_at_threshold() {
    let mut g = Game::new();
    let _player = g.objs.alloc().expect("p");
    g.objs.aliens[0].worldx = 300;
    g.objs.aliens[0].worldz = 2000;
    let idx = g.objs.alloc().expect("b");
    {
        let object = &mut g.objs.aliens[idx as usize];
        object.vel = 100;
        object.sflags2 |= ASF2_SFLAG2;
    }
    ironball_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG2;
    g.vars.shared.power_build = 129;
    g.vars.strategy.view_pitch = 18 << 8;
    g.vars.strategy.view_yaw = 36 << 8;
    g.vars.strategy.player_turn_rotation = 3 << 8;

    ironball_strat(&mut g, idx);

    let object = &g.objs.aliens[idx as usize];
    assert_eq!(object.sflags2 & ASF2_SFLAG2, 0);
    assert_ne!(object.sflags2 & ASF2_SFLAG1, 0);
    assert_eq!(g.vars.shared.power_build, 0);
    assert_eq!(object.rotx, 18);
    assert_eq!(object.roty, 36u8.wrapping_neg().wrapping_add(128 + 3));
    assert_eq!(object.vx & 1, 0);
    assert_eq!(object.vy & 1, 0);
    assert_eq!(object.vz & 1, 0);
}
