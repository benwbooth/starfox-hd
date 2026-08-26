//! Tick 91: bossAturret L/M/R istrat + strat leaves.

use sf_game::alien::{ASF4_INVISIBLE, ASF_COLLDISABLE, ASF_NOHITAFFECT, ASF_SHADOW};
use sf_game::Game;
use sf_strat::enemy_a::boss_attach_child_to_mother;
use sf_strat::enemy_b::{
    bossaturretl_istrat, bossaturretl_strat, bossaturretm_istrat, bossaturretm_strat,
    bossaturretr_istrat, bossaturretr_strat,
};
use sf_strat::snes_trig::strat_roffs_yaw_scaled;

const BOSSA_TURRET_HP: u8 = 12;
const DEG180: u8 = 128;
const BOSSA_SCALE: u32 = 2;
const ASF2_SFLAG1: u8 = 0x10;

/// ROM byte offs × bossA_scale via yaw-only `rotate_8xz` (mulslog, not exact ×).
fn turret_world_xy(offx_byte: i8, offy_byte: i8) -> (i16, i16) {
    let (x, y, _) = strat_roffs_yaw_scaled(0, offx_byte, offy_byte, 0, BOSSA_SCALE);
    (x, y)
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn mother_with_turret(g: &mut Game, child_num: u8) -> (u16, u16) {
    let mother = spawn(g);
    g.objs.aliens[mother as usize].worldx = 0;
    g.objs.aliens[mother as usize].worldy = 0;
    g.objs.aliens[mother as usize].worldz = 1000;
    g.objs.aliens[mother as usize].roty = 0;
    let turret = spawn(g);
    assert!(boss_attach_child_to_mother(g, mother, turret, child_num));
    (mother, turret)
}

#[test]
fn bossaturret_lmr_init_common() {
    let mut g = Game::new();
    let (_m, t_l) = mother_with_turret(&mut g, 1);
    bossaturretl_istrat(&mut g, t_l);
    assert_eq!(g.objs.aliens[t_l as usize].hp, BOSSA_TURRET_HP);
    assert_eq!(g.objs.aliens[t_l as usize].sbyte2, 60);
    assert_eq!(g.objs.aliens[t_l as usize].sbyte3, 0);
    assert_ne!(g.objs.aliens[t_l as usize].sflags & ASF_COLLDISABLE, 0);
    assert_ne!(g.objs.aliens[t_l as usize].sflags & ASF_NOHITAFFECT, 0);
    assert_ne!(g.objs.aliens[t_l as usize].sflags & ASF_SHADOW, 0);
    // Offset L: (-85,-50) × bossA_scale via rotate_8xz
    let (lx, ly) = turret_world_xy(-85, -50);
    assert_eq!(g.objs.aliens[t_l as usize].worldx, lx);
    assert_eq!(g.objs.aliens[t_l as usize].worldy, ly);

    let (_m2, t_m) = mother_with_turret(&mut g, 2);
    // ROM sets sbyte3=#deg180 then Icont clears to 0
    g.objs.aliens[t_m as usize].sbyte3 = DEG180;
    bossaturretm_istrat(&mut g, t_m);
    assert_eq!(g.objs.aliens[t_m as usize].sbyte3, 0);
    let (mx, my) = turret_world_xy(0, -40);
    assert_eq!(g.objs.aliens[t_m as usize].worldx, mx);
    assert_eq!(g.objs.aliens[t_m as usize].worldy, my);

    let (_m3, t_r) = mother_with_turret(&mut g, 3);
    bossaturretr_istrat(&mut g, t_r);
    let (rx, _) = turret_world_xy(85, -50);
    assert_eq!(g.objs.aliens[t_r as usize].worldx, rx);
}

#[test]
fn bossaturret_cont_aim_and_lone_sweep() {
    let mut g = Game::new();
    let (mother, turret) = mother_with_turret(&mut g, 1);
    bossaturretl_istrat(&mut g, turret);
    // Clear nohitaffect so fire path is reachable; face player (roty≈180)
    g.objs.aliens[turret as usize].sflags &= !ASF_NOHITAFFECT;
    g.objs.aliens[turret as usize].roty = DEG180;
    g.objs.aliens[turret as usize].sbyte3 = DEG180;

    // Chase already at target
    bossaturretl_strat(&mut g, turret);
    assert_eq!(g.objs.aliens[turret as usize].roty, DEG180);
    // Still mother-relative (mulslog yaw Roffs)
    let (lx, _) = turret_world_xy(-85, -50);
    assert_eq!(g.objs.aliens[turret as usize].worldx, lx);

    // Lone-turret sweep: mother.sbyte3==2 → toggle aim after sbyte2 countdown
    g.objs.aliens[mother as usize].sbyte3 = 2;
    g.objs.aliens[turret as usize].sbyte2 = 1;
    g.objs.aliens[turret as usize].sflags2 &= !ASF2_SFLAG1;
    bossaturretl_strat(&mut g, turret);
    // decbne 1→0 → toggle sflag1 on → sbyte3=deg180, sbyte2=60
    assert_ne!(g.objs.aliens[turret as usize].sflags2 & ASF2_SFLAG1, 0);
    assert_eq!(g.objs.aliens[turret as usize].sbyte3, DEG180);
    assert_eq!(g.objs.aliens[turret as usize].sbyte2, 60);

    // Next expiry with sflag1 set → turn to 0 / 20
    g.objs.aliens[turret as usize].sbyte2 = 1;
    bossaturretl_strat(&mut g, turret);
    assert_eq!(g.objs.aliens[turret as usize].sflags2 & ASF2_SFLAG1, 0);
    assert_eq!(g.objs.aliens[turret as usize].sbyte3, 0);
    assert_eq!(g.objs.aliens[turret as usize].sbyte2, 20);

    // Invisible husk: still repositions, no crash
    g.objs.aliens[turret as usize].sflags4 |= ASF4_INVISIBLE;
    bossaturretm_strat(&mut g, turret); // same cont
    assert!(g.objs.aliens[turret as usize].active);

    bossaturretr_strat(&mut g, turret);
}
