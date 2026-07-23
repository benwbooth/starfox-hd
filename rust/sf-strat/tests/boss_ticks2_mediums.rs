//! Tick 137: AUDIT_BOSS_TICKS2 Mediums #10–#15 verify (already fixed in bosses.rs).

use sf_game::alien::{ASF3_REALOBJ, ASF_COLLDISABLE};
use sf_game::Game;
use sf_strat::bosses::{
    boss8a_strat, boss8die_istrat, bossseamon_strat, nucleuslauncher_istrat, nucleuslauncher_strat,
    seamon_strat, strat_bossseamon_init, strat_seamon_init,
};
use sf_strat::common::sf_random;

const SH_SEA_0_0: u16 = 31;
const SH_SEA_0_1_PROXY: u16 = 258;
const SEA_SFLAG1: u8 = 0x20;
const SEA_SFLAG2: u8 = 0x40;
const B8_SFLAG5: u8 = 0x01; // sflags3 — ROM asf_sflag5

fn spawn_player(g: &mut Game, x: i16, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = x;
    al.worldy = -40;
    al.worldz = z;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

/// Medium #10: state-8 splash draws vx=(rnd&7)+5 FIRST, then negate when rnd>=127.
#[test]
fn bossseamon_state8_splash_rng_order_and_coin() {
    // POS seed [0,4,0,0] → d1=250 (vx=7), d2=14 (<127) → +7
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0, 0);
        let boss = spawn(&mut g);
        strat_bossseamon_init(&mut g, boss);
        g.vars.rng = [0, 4, 0, 0];
        let al = &mut g.objs.aliens[boss as usize];
        al.stratstate = 8;
        al.worldy = 0;
        al.vy = 0;
        bossseamon_strat(&mut g, boss);
        assert_eq!(g.objs.aliens[boss as usize].stratstate, 2);
        assert_eq!(g.objs.aliens[boss as usize].vx, 7, "pos coin keeps +vx");
        assert_eq!(g.objs.aliens[boss as usize].shape, SH_SEA_0_0);
    }
    // NEG seed [0,44,0,0] → d1=210 (vx=7), d2=134 (>=127) → -7
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0, 0);
        let boss = spawn(&mut g);
        strat_bossseamon_init(&mut g, boss);
        g.vars.rng = [0, 44, 0, 0];
        let al = &mut g.objs.aliens[boss as usize];
        al.stratstate = 8;
        al.worldy = 0;
        al.vy = 0;
        bossseamon_strat(&mut g, boss);
        assert_eq!(g.objs.aliens[boss as usize].vx, -7, "neg coin flips vx");
    }
    // Draw-order sanity: first draw feeds vx, second feeds coin (same seeds as above).
    {
        let mut vars = sf_game::vars::GameVars::default();
        vars.rng = [0, 44, 0, 0];
        let d1 = sf_random(&mut vars);
        let d2 = sf_random(&mut vars);
        assert_eq!((d1 & 7) + 5, 7);
        assert!(d2 >= 127);
    }
}

/// Medium #11: post-landing band with sflag1 latched snaps worldy/vy to 0.
#[test]
fn seamon_post_landing_snaps_to_surface() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, 0);
    let fish = spawn(&mut g);
    strat_seamon_init(&mut g, fish);
    {
        let al = &mut g.objs.aliens[fish as usize];
        al.worldy = -10;
        al.vy = 0;
        al.vx = 0;
        al.vz = 0;
        al.sflags2 |= SEA_SFLAG1; // already latched
        al.sflags |= ASF_COLLDISABLE;
        al.sbyte3 = 40; // skip swim wiggle
        al.sbyte4 = 40; // skip jump countdown path side-effects
    }
    seamon_strat(&mut g, fish);
    let al = &g.objs.aliens[fish as usize];
    assert_eq!(al.worldy, 0, "snap flush to surface");
    assert_eq!(al.vy, 0, "kill upward drift");
}

/// Medium #12: swim-shape alternates only while sflag1+colldisable clear.
#[test]
fn seamon_swim_shape_byte_test_locks_after_landing() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, 0);
    let fish = spawn(&mut g);
    strat_seamon_init(&mut g, fish);

    // Pre-landing: clear latch bits; force a shape-toggle tick (sbyte3==0, sbyte1==1).
    {
        let al = &mut g.objs.aliens[fish as usize];
        al.worldy = 0;
        al.vy = 0;
        al.vx = 0;
        al.vz = 0;
        al.sflags2 = 0;
        al.sflags &= !ASF_COLLDISABLE;
        al.sbyte3 = 0;
        al.sbyte1 = 1;
        al.sbyte4 = 40;
    }
    seamon_strat(&mut g, fish);
    // sflag2 was 0 → XOR sets it → byte nonzero → forced sea_0_0
    assert_eq!(g.objs.aliens[fish as usize].shape, SH_SEA_0_0);
    assert_ne!(g.objs.aliens[fish as usize].sflags2 & SEA_SFLAG2, 0);

    // Next toggle: sflag2 set → XOR clears → byte 0 → keep sea_0_1
    {
        let al = &mut g.objs.aliens[fish as usize];
        al.sbyte3 = 0;
        al.sbyte1 = 1;
        al.sbyte4 = 40;
        al.worldy = 0;
        al.vy = 0;
        al.vx = 0;
        al.vz = 0;
    }
    seamon_strat(&mut g, fish);
    assert_eq!(
        g.objs.aliens[fish as usize].shape, SH_SEA_0_1_PROXY,
        "pre-landing alternates to sea_0_1"
    );
    assert_eq!(g.objs.aliens[fish as usize].sflags2 & SEA_SFLAG2, 0);

    // Post-landing: sflag1 + colldisable → always forced sea_0_0
    {
        let al = &mut g.objs.aliens[fish as usize];
        al.sflags2 = SEA_SFLAG1; // sflag2 clear, sflag1 set
        al.sflags |= ASF_COLLDISABLE;
        al.sbyte3 = 0;
        al.sbyte1 = 1;
        al.sbyte4 = 40;
        al.worldy = 0;
        al.vy = 0;
        al.vx = 0;
        al.vz = 0;
    }
    seamon_strat(&mut g, fish);
    assert_eq!(
        g.objs.aliens[fish as usize].shape, SH_SEA_0_0,
        "post-landing never shows sea_0_1"
    );
}

/// Medium #13: boss8a open-flap caps at 14 (no wrap).
#[test]
fn boss8a_open_flap_caps_at_fourteen() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, 0);
    g.vars.write_ext8(sf_strat::enemy_a::wm::CURRENTLEVEL, 2); // not easy short-circuit
    let boss = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[boss as usize];
        al.sflags3 |= B8_SFLAG5;
        al.animframe = 13;
        al.sbyte2 = 100;
        al.stratptr = Some(g.world.register_strategy(boss8a_strat));
    }
    g.vars.gameframe = 0; // avoid plasma frames 25/30
    boss8a_strat(&mut g, boss);
    assert_eq!(g.objs.aliens[boss as usize].animframe, 14);
    boss8a_strat(&mut g, boss);
    assert_eq!(
        g.objs.aliens[boss as usize].animframe, 14,
        "cap holds — no wrap to 0"
    );
}

/// Medium #14: nucleuslauncher arms only while player.z < launcher.z.
#[test]
fn nucleuslauncher_objinfront_gate() {
    // In front → countdown fires (sbyte3 1→0 → fire strat).
    // Pin worldx/z AFTER istrat: wallrot repositions from sword2/sbyte2.
    {
        let mut g = Game::new();
        spawn_player(&mut g, 800, 500); // player.z < launcher.z
        let launch = spawn(&mut g);
        g.vars.rng = [0, 0, 0, 0];
        nucleuslauncher_istrat(&mut g, launch);
        {
            let al = &mut g.objs.aliens[launch as usize];
            al.worldx = 800;
            al.worldz = 1000;
            al.sbyte3 = 1;
        }
        let idle = g.world.register_strategy(nucleuslauncher_strat);
        g.objs.aliens[launch as usize].stratptr = Some(idle);
        nucleuslauncher_strat(&mut g, launch);
        assert_ne!(
            g.objs.aliens[launch as usize].stratptr,
            Some(idle),
            "in-front arms fire strat"
        );
    }
    // Behind → no arm, sbyte3 unchanged.
    {
        let mut g = Game::new();
        spawn_player(&mut g, 800, 1500); // player.z >= launcher.z
        let launch = spawn(&mut g);
        g.vars.rng = [0, 0, 0, 0];
        nucleuslauncher_istrat(&mut g, launch);
        {
            let al = &mut g.objs.aliens[launch as usize];
            al.worldx = 800;
            al.worldz = 1000;
            al.sbyte3 = 3;
        }
        let idle = g.world.register_strategy(nucleuslauncher_strat);
        g.objs.aliens[launch as usize].stratptr = Some(idle);
        nucleuslauncher_strat(&mut g, launch);
        assert_eq!(
            g.objs.aliens[launch as usize].sbyte3, 3,
            "behind: no countdown"
        );
        assert_eq!(
            g.objs.aliens[launch as usize].stratptr,
            Some(idle),
            "behind stays idle"
        );
    }
}

/// Medium #15: boss8die leaves bossmaxhp alone.
#[test]
fn boss8die_preserves_bossmaxhp() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, 0);
    let boss = spawn(&mut g);
    g.vars.bossmaxhp = 120;
    boss8die_istrat(&mut g, boss);
    assert_eq!(g.vars.bossmaxhp, 120, "ROM never clears bossmaxHP on die");
}
