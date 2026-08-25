//! ROM bossBrob morph / split / demo / undead / die (GB3STRAT.ASM).

use sf_game::alien::{ASF3_NOHITAFFECT, ASF_SHADOW};
use sf_game::vars::GF_BOSSDEAD;
use sf_game::Game;
use sf_strat::bossb::{
    bossbrob2_init, bossbrob_cont, bossbrob_init, bossbrobcent_srou, bossbrobchg2_init,
    bossbrobchg2_strat, bossbrobchg3_init, bossbrobchg4_init, bossbrobchg4_strat,
    bossbrobchg_istrat, bossbrobchg_strat, bossbrobcol_istrat, bossbrobdemo_istrat,
    bossbrobdie_init, bossbrobdie_strat, bossbrobfrontplayer_srou, bossbrobfrontplayerz_srou,
    bossbrobouch_srou, bossbrobsep_init, bossbrobsepcol_istrat, bossbrobsplit2_init,
    bossbrobsplit_init, bossbrobundead_istrat, bossbrobundead_strat, bossbrobvecs_cont,
    bossbrobvecs_cont4,
};

const ANDROSS_WALKING_BODY_SHAPE: u16 = 75;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
}

fn spawn_rob(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("rob");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldy = -400;
    g.objs.aliens[idx as usize].worldz = 2500;
    idx
}

#[test]
fn init_wires_chg_expstrat() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    bossbrob_init(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].expstratptr.is_some());
    assert_eq!(g.objs.aliens[idx as usize].hp, 32);
}

#[test]
fn chg_istrat_latches_walking_and_counts_down() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    bossbrobchg_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags3 & 0x01, 0);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 39); // 40 then tick
    assert_ne!(g.objs.aliens[idx as usize].sflags3 & ASF3_NOHITAFFECT, 0);
    // Force into chg2.
    g.objs.aliens[idx as usize].sbyte1 = 0;
    bossbrobchg_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 0);
}

#[test]
fn chg2_to_chg3_when_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldz = 500; // |dz|<1000
    bossbrobchg2_init(&mut g, idx);
    // init fall-through already ran; force another tick at close range.
    g.objs.aliens[idx as usize].worldz = 500;
    bossbrobchg2_strat(&mut g, idx);
    // chg3 spins rotz.
    assert!(g.objs.aliens[idx as usize].rotz > 0 || g.objs.aliens[idx as usize].vel > 0);
}

#[test]
fn chg3_to_chg4_when_far() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldz = 2000; // |dz|>=1400
    bossbrobchg3_init(&mut g, idx);
    // chg4 sets sbyte1=30 (then may tick).
    assert!(g.objs.aliens[idx as usize].sbyte1 <= 30);
}

#[test]
fn chg4_ramps_hp_then_start() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].hp = 2;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].roty = 128; // DEG180
    bossbrobchg4_init(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].shape,
        ANDROSS_WALKING_BODY_SHAPE
    );
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 29); // 30 then tick
                                                        // Force anim/HP path: already at yaw, speed 0.
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].roty = 128;
    g.objs.aliens[idx as usize].sbyte1 = 1;
    g.vars.gameframe = 2; // notdelay(1) false → vecs_cont path still ok
    bossbrobchg4_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].hp >= 2);
}

#[test]
fn split_and_cent_recenter() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldx = 200;
    g.objs.aliens[idx as usize].worldy = -100;
    bossbrobcent_srou(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].worldx.abs() < 200);
    bossbrob2_init(&mut g, idx);
    // braking toward split.
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    // Near center → split2 spawns parts.
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -350;
    bossbrobsplit_init(&mut g, idx);
    let before = g.objs.active_indices().len();
    bossbrobsplit2_init(&mut g, idx);
    assert!(g.objs.active_indices().len() >= before);
}

#[test]
fn sep_uses_sepcol_and_ouch_cont() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    bossbrobsep_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 2);
    assert!(g.objs.aliens[idx as usize].collstratptr.is_some());
    // Ouch reaction.
    g.objs.aliens[idx as usize].sbyte3 = 16;
    g.objs.aliens[idx as usize].sbyte4 = 0;
    bossbrobouch_srou(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 15);
    bossbrob_cont(&mut g, idx);
    bossbrobvecs_cont4(&mut g, idx);
    bossbrobvecs_cont(&mut g, idx);
}

#[test]
fn col_and_sepcol_zone_routing() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].hitflags = 1; // HF1 top
    bossbrobcol_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte4, 64);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 16);
    // sepcol ignores another hit while the ROM sflag5 ouch latch is active.
    g.objs.aliens[idx as usize].sbyte2 = 1;
    g.objs.aliens[idx as usize].sflags3 &= !ASF3_NOHITAFFECT;
    g.objs.aliens[idx as usize].sflags2 &= !0x10;
    g.objs.aliens[idx as usize].hitflags = 2;
    bossbrobsepcol_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 1);
    assert_eq!(g.objs.aliens[idx as usize].hitflags, 0);

    // Once the 16-frame reaction ends, the same hit exhausts the split-form
    // counter and dispatches bossBrobnextstate.
    for _ in 0..16 {
        bossbrobouch_srou(&mut g, idx);
    }
    g.objs.aliens[idx as usize].hitflags = 2;
    bossbrobsepcol_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 0);
}

#[test]
fn frontplayer_chases_xy_and_z() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldx = 100;
    g.objs.aliens[idx as usize].worldy = -100;
    bossbrobfrontplayer_srou(&mut g, idx, 1400);
    assert!(g.objs.aliens[idx as usize].worldx.abs() < 100);
    bossbrobfrontplayerz_srou(&mut g, idx, 1400);
}

#[test]
fn demo_undead_die() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    bossbrobdemo_istrat(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].shape,
        ANDROSS_WALKING_BODY_SHAPE
    );
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -320);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 34); // 35 then tick

    let u = spawn_rob(&mut g);
    bossbrobundead_istrat(&mut g, u);
    assert_eq!(g.objs.aliens[u as usize].hp, 32);
    // istrat fall-through applies gravity (+2) onto the initial vy=-40.
    assert_eq!(g.objs.aliens[u as usize].vy, -38);
    bossbrobundead_strat(&mut g, u);

    let d = spawn_rob(&mut g);
    bossbrobdie_init(&mut g, d);
    assert_eq!(g.objs.aliens[d as usize].sbyte1, 4); // 5 then tick
    g.objs.aliens[d as usize].sbyte1 = 0;
    bossbrobdie_strat(&mut g, d);
    assert_ne!(g.vars.gameflags & GF_BOSSDEAD, 0);
}
