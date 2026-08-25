//! Tick 92: bossB core leaves + bossflash + range/pointdir helpers.

use sf_game::alien::ASF3_NOHITAFFECT;
use sf_game::vars::GF_BOSSDEAD;
use sf_game::windows::{Windows, WINDOW_MODE_DYINGRED};
use sf_game::Game;
use sf_strat::bossb::{
    bossb_cont2, bossb_cont4, bossb_init, bossb_istrat, bossb_strat, bossbaddbhp_cont,
    bossbaddpz_cont, bossbdodge_init, bossbdodge_strat, bossbdodgecol_istrat, bossbescape_istrat,
    bossbescape_strat, bossbpointdir_srou, bossbrange_srou, bossflash_l,
};

const BOSSB_AIR_HP: u8 = 40;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn bossflash_allocates_dyingred_slot() {
    let mut w = Windows::new();
    w.boss_flash();
    assert_ne!(w.windowmode & 1, 0);
    assert_eq!(w.slots[0].mode, WINDOW_MODE_DYINGRED);
    assert_eq!(w.slots[0].wm_val, 31);

    let mut g = Game::new();
    bossflash_l(&mut g); // NullHooks no-op — must not panic
}

#[test]
fn bossbrange_and_pointdir() {
    // Same point → range 0
    assert_eq!(bossbrange_srou(10, 20, 10, 20), 0);
    let r = bossbrange_srou(0, 0, 400, 0);
    assert!(r > 0);

    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = 0;
    g.objs.aliens[idx as usize].worldz = 1000;
    g.objs.aliens[idx as usize].roty = 0;
    g.objs.aliens[idx as usize].rotx = 0;
    bossbpointdir_srou(&mut g, idx, 500, 0);
    // Should yaw toward +X (roughly deg90)
    assert_ne!(g.objs.aliens[idx as usize].roty, 0);
}

#[test]
fn bossb_core_init_dodge_escape() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    g.vars.internal_playpt = player as i16;
    g.objs.aliens[player as usize].worldz = 0;

    let boss = spawn(&mut g);
    g.objs.aliens[boss as usize].worldz = 5000; // far → approach
    bossb_istrat(&mut g, boss);
    assert_eq!(g.objs.aliens[boss as usize].hp, BOSSB_AIR_HP);
    assert_eq!(g.vars.bossmaxhp, BOSSB_AIR_HP as u16);
    assert_eq!(g.objs.aliens[boss as usize].vel, 40);
    // Init sets roty=#deg90 then same-frame strat aims at player (may change).
    assert_ne!(g.objs.aliens[boss as usize].sflags3 & ASF3_NOHITAFFECT, 0);
    // cont2 ran → bosshp accumulated
    assert_eq!(g.vars.bosshp, BOSSB_AIR_HP as u16);

    // Close range → decelerate toward dodge
    g.objs.aliens[boss as usize].worldz = 1000;
    g.objs.aliens[boss as usize].vel = 1;
    bossb_strat(&mut g, boss);
    // SR_SPEEDTO reaches zero on this call but only sets carry when zero was
    // already present on entry, so the ROM branches to dodge on the next tick.
    assert_eq!(g.objs.aliens[boss as usize].vel, 0);
    bossb_strat(&mut g, boss);
    assert_eq!(g.objs.aliens[boss as usize].sflags3 & ASF3_NOHITAFFECT, 0);
    // dodge_init sets sbyte3=#1 then same-frame strat: near tab keeps 1;
    // far-from-target (.nthere) sets sbyte3=#25 (GB3STRAT.ASM:1393-1399).
    let sb3 = g.objs.aliens[boss as usize].sbyte3;
    assert!(sb3 == 1 || sb3 == 25, "dodge sbyte3={sb3}");

    // dodgecol resets timer (+ hitflash may tick HP)
    g.objs.aliens[boss as usize].sbyte3 = 9;
    let hp_before = g.objs.aliens[boss as usize].hp;
    bossbdodgecol_istrat(&mut g, boss);
    // The collision handler jumps through hitflashBOSSD into the current
    // dodge strategy in the same frame.  It resets to 1 first, then a boss
    // still far from its selected target immediately re-arms the ROM's 25
    // frame reposition timer.
    let sb3 = g.objs.aliens[boss as usize].sbyte3;
    assert!(sb3 == 1 || sb3 == 25, "post-collision dodge timer={sb3}");
    assert!(g.objs.aliens[boss as usize].hp <= hp_before);

    // addpz / addbhp
    g.vars.bosshp = 0;
    let hp = g.objs.aliens[boss as usize].hp as u16;
    bossbaddbhp_cont(&mut g, boss);
    assert_eq!(g.vars.bosshp, hp);
    bossb_cont2(&mut g, boss);
    bossb_cont4(&mut g, boss);
    bossbaddpz_cont(&mut g, boss);

    // escape
    bossbescape_istrat(&mut g, boss);
    assert_ne!(g.vars.gameflags & GF_BOSSDEAD, 0);
    bossbescape_strat(&mut g, boss);
    assert!(g.objs.aliens[boss as usize].vel > 0);

    // dodge strat smoke (hp high enough to stay in dodge)
    g.objs.aliens[boss as usize].hp = 30;
    bossbdodge_init(&mut g, boss);
    bossbdodge_strat(&mut g, boss);
}

#[test]
fn bossb_init_alias() {
    let mut g = Game::new();
    let p = spawn(&mut g);
    g.vars.internal_playpt = p as i16;
    let boss = spawn(&mut g);
    g.objs.aliens[boss as usize].worldz = 4000;
    bossb_init(&mut g, boss);
    assert_eq!(g.objs.aliens[boss as usize].hp, BOSSB_AIR_HP);
}
