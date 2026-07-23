//! Tick 102: BossF airship heli parts — body / feet / arm / head (DSTRATS.ASM).
//! Tick 217: ENEMY1 colltype = ACF_COLLTYPE2 (0x10).

use sf_game::alien::{ACF_COLLTYPE2, ASF3_CHILDOBJ, ASF_NOHITAFFECT, ASF_SHADOW};
use sf_game::vars::{COLLTYPE_ENEMY1, HARD_AP, HARD_HP};
use sf_game::Game;
use sf_strat::bossf_heli::{
    airship_istrat, airship_strat, bossfarm_istrat, bossfarm_strat, bossfbody_istrat,
    bossfbody_strat, bossffeet_istrat, bossffeet_strat, bossfhead_istrat, bossfhead_strat,
    AIRSHIP_MODE_BOSSF_HELI, IS_AIRSHIP, SH_AIRSHIP, SH_AIRSHIP_BODY, SH_AIRSHIP_FEET,
    SH_AIRSHIP_HEAD, STRAT_ADDR_AIRSHIP,
};
use sf_strat::enemy_a::{boss_attach_child_to_mother, ASF3_SFLAG6, DEG90};

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn airship_parent_is_registered_for_both_map_address_forms() {
    let mut g = Game::new();
    sf_strat::table::register_all(&mut g);
    let row = g.world.istrats[IS_AIRSHIP].expect("airship istrat row");
    assert_eq!(g.world.find_strategy_address(STRAT_ADDR_AIRSHIP), Some(row));
    assert_eq!(
        g.world.find_strategy_address(0x020000 | IS_AIRSHIP as u32),
        Some(row)
    );
}

#[test]
fn airship_major_change_builds_the_real_three_part_graph() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[mother as usize];
        al.shape = SH_AIRSHIP;
        al.worldy = -120;
    }
    airship_istrat(&mut g, mother);
    g.objs.aliens[mother as usize].stratstate = 2; // .majorchange
    airship_strat(&mut g, mother);

    assert_eq!(
        g.objs.aliens[mother as usize].shape, 0,
        "flying shell hidden"
    );
    for shape in [SH_AIRSHIP_BODY, SH_AIRSHIP_HEAD, SH_AIRSHIP_FEET] {
        assert!(
            g.objs
                .aliens
                .iter()
                .any(|al| al.active && al.shape == shape),
            "generated child shape {shape}"
        );
    }
    assert_eq!(g.objs.aliens[mother as usize].stratstate, 3);
}

#[test]
fn airship_opening_turn_advances_from_zero_rotation() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    g.objs.aliens[player as usize].worldz = -4000;
    let mother = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[mother as usize];
        al.shape = SH_AIRSHIP;
        al.worldy = -200;
        al.worldz = 0;
        al.roty = 0;
        al.rotz = 0;
    }
    airship_istrat(&mut g, mother);
    for _ in 0..200 {
        airship_strat(&mut g, mother);
        if g.objs.aliens[mother as usize].stratstate != 0 {
            break;
        }
    }
    assert_ne!(
        g.objs.aliens[mother as usize].stratstate, 0,
        "SEC+ADC opening turn reached deg180 and advanced"
    );
}

#[test]
fn body_istrat_hard_shadow_nohit() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    bossfbody_istrat(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.hp, HARD_HP);
    assert_eq!(al.ap, HARD_AP);
    assert_ne!(al.sflags & ASF_SHADOW, 0);
    assert_ne!(al.sflags & ASF_NOHITAFFECT, 0);
    assert_eq!(al.stratstate, 0);
    assert!(al.stratptr.is_some());
    assert!(al.collstratptr.is_some());
}

#[test]
fn body_toground_chases_childy() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    bossfbody_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].stratstate = 1; // bossfbody_toground
    g.objs.aliens[idx as usize].childy = 0;
    for _ in 0..40 {
        bossfbody_strat(&mut g, idx);
        if g.objs.aliens[idx as usize].stratstate != 1 {
            break;
        }
    }
    assert_eq!(g.objs.aliens[idx as usize].childy as i8, -10);
    assert!(g.objs.aliens[idx as usize].stratstate >= 2);
}

#[test]
fn feet_istrat_sets_boss_bar_and_hp() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    bossffeet_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 80);
    assert_eq!(g.vars.bossmaxhp, 80);
    assert!(g.objs.aliens[idx as usize].expstratptr.is_some());
}

#[test]
fn feet_explode_switches_mother_to_heli() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let feet = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, feet, 3));
    assert_ne!(g.objs.aliens[feet as usize].sflags3 & ASF3_CHILDOBJ, 0);
    bossffeet_istrat(&mut g, feet);
    // Drive explode path directly.
    let exp = g.objs.aliens[feet as usize].expstratptr.expect("exp");
    g.call_strat(exp, feet);
    assert_eq!(
        g.objs.aliens[mother as usize].stratstate,
        AIRSHIP_MODE_BOSSF_HELI
    );
}

#[test]
fn feet_sflag6_launches_missile() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    bossffeet_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].sflags3 |= ASF3_SFLAG6;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    bossffeet_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sflags3 & ASF3_SFLAG6, 0);
    assert!(g.objs.aliens.iter().filter(|a| a.active).count() > before);
}

#[test]
fn feet_opening_fires_boss_homing_missile() {
    const FULL_OPEN_FLAG: u8 = 128;
    const FIRE_LATCH: u8 = 1;
    const PRE_FIRE_ANIMATION: u8 = 7;
    const BOSS_HOMING_MISSILE_SHAPE: u16 = 403;
    const IRONBALL_SHAPE: u16 = 404;

    let mut g = Game::new();
    let idx = spawn(&mut g);
    bossffeet_istrat(&mut g, idx);

    for _ in 0..256 {
        let feet = &mut g.objs.aliens[idx as usize];
        feet.sflags2 |= FULL_OPEN_FLAG;
        feet.sflags3 |= FIRE_LATCH;
        feet.animframe = 128 | PRE_FIRE_ANIMATION;
        bossffeet_strat(&mut g, idx);
        if g.objs
            .aliens
            .iter()
            .any(|object| object.active && object.shape == BOSS_HOMING_MISSILE_SHAPE)
        {
            break;
        }
    }

    assert!(g
        .objs
        .aliens
        .iter()
        .any(|object| object.active && object.shape == BOSS_HOMING_MISSILE_SHAPE));
    assert!(!g
        .objs
        .aliens
        .iter()
        .any(|object| object.active && object.shape == IRONBALL_SHAPE));
}

#[test]
fn arm_heli1_chases_child_rots() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    bossfarm_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].stratstate = 8; // bossfarm_heli1
    g.objs.aliens[idx as usize].childrotz = 0;
    g.objs.aliens[idx as usize].childroty = 0;
    for _ in 0..30 {
        bossfarm_strat(&mut g, idx);
    }
    // Achase toward -deg90 / +deg90 — should have moved off zero.
    assert_ne!(g.objs.aliens[idx as usize].childrotz, 0);
    assert_ne!(g.objs.aliens[idx as usize].childroty, 0);
    // Signs: rotz toward -90, roty toward +90.
    assert!((g.objs.aliens[idx as usize].childrotz as i8) < 0);
    assert!((g.objs.aliens[idx as usize].childroty as i8) > 0);
    let _ = DEG90;
}

#[test]
fn head_istrat_and_heli_advances() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    bossfhead_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_NOHITAFFECT, 0);
    assert_eq!(g.objs.aliens[idx as usize].hp, HARD_HP);
    g.objs.aliens[idx as usize].stratstate = 2; // heli
    g.objs.aliens[idx as usize].childroty = 8; // near 0 — achase shift5 is slow
    for _ in 0..20 {
        bossfhead_strat(&mut g, idx);
        if g.objs.aliens[idx as usize].stratstate != 2 {
            break;
        }
    }
    // Heli done → animate (anim0) → begin_rotate same tick.
    assert!(
        g.objs.aliens[idx as usize].stratstate >= 3,
        "state={} roty={}",
        g.objs.aliens[idx as usize].stratstate,
        g.objs.aliens[idx as usize].childroty
    );
}

#[test]
fn body_spin_advances_on_sbyte4() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    bossfbody_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].stratstate = 2; // spin
    g.objs.aliens[idx as usize].sbyte4 = 0;
    g.vars.gameframe = 0; // delay bit1 open on even frames
    for f in 0..50u16 {
        g.vars.gameframe = f;
        bossfbody_strat(&mut g, idx);
        if g.objs.aliens[idx as usize].stratstate != 2 {
            break;
        }
    }
    assert!(g.objs.aliens[idx as usize].stratstate >= 3);
}

#[test]
fn heli_parts_enemy1_is_colltype2() {
    let mut g = Game::new();
    for init in [
        bossfbody_istrat as fn(&mut Game, u16),
        bossffeet_istrat,
        bossfarm_istrat,
        bossfhead_istrat,
    ] {
        let idx = spawn(&mut g);
        init(&mut g, idx);
        let cf = g.objs.aliens[idx as usize].collflags;
        assert_ne!(cf & ACF_COLLTYPE2, 0);
        assert_eq!(cf & COLLTYPE_ENEMY1, 0);
    }
}
