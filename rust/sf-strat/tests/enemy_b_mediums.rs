//! Tick 194: AUDIT_ENEMY_B Mediums #20–#26 verify (already ported).

use sf_game::alien::ASF_COLLDISABLE;
use sf_game::vars::GameVars;
use sf_game::Game;
use sf_strat::common::{sf_random, strat_gen_vecs_3d};
use sf_strat::enemy_a::{achase_angle, boss_attach_child_to_mother, wm};
use sf_strat::enemy_b::{
    boss7d_strat, boss7hatchexp_istrat, bossa_strat, bossaexp_strat, bossfcdie2_strat,
    spacepilonP_strat, strat_spacepilon_init,
};
use sf_strat::ground;
use sf_strat::snes_trig::{COSTAB, SINTAB};

/// ROM `SPACEPILON_PILON_RELPOSY_TGT` = -100/8.
const SPACEPILON_RELPOSY_TGT: u8 = (-12i8) as u8;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldz = z;
}

fn adiv2n(v: i16, n: u32) -> i16 {
    let mut x = v;
    for _ in 0..n {
        x /= 2;
    }
    x
}

fn rnd2pos_scatter(vars: &mut GameVars) -> (i16, i16, i16) {
    // s_add_rnd2pos x,255,255,255,2,2,1
    let rx = (((sf_random(vars) & 0xFF) as i16) - 127) << 2;
    let ry = (((sf_random(vars) & 0xFF) as i16) - 127) << 2;
    let rz = (((sf_random(vars) & 0xFF) as i16) - 127) << 1;
    (rx, ry, rz)
}

/// Medium #20: spacepilon init scatter is RNG ((rnd&255)-127)<<2/<<2/<<1.
#[test]
fn spacepilon_scatter_rnd2pos_scales() {
    let mut g = Game::new();
    g.vars.rng = [0x12, 0x34, 0x56, 0x78];
    // Keep slot 0 inactive so `player()` is None (else the pilon would be
    // treated as the player and world pos overwritten by SPAWN_ZOFF).
    let _slot0 = g.objs.alloc().expect("slot0");
    g.objs.aliens[0].active = false;
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 1000;
    g.objs.aliens[idx as usize].worldy = 2000;
    g.objs.aliens[idx as usize].worldz = 3000;

    let mut predict_vars = GameVars::default();
    predict_vars.rng = g.vars.rng;
    let (rx, ry, rz) = rnd2pos_scatter(&mut predict_vars);

    strat_spacepilon_init(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    // No player → world keeps scatter; vx/vy/vz snapshot the scattered pos.
    assert_eq!(al.worldx, 1000i16.wrapping_add(rx));
    assert_eq!(al.worldy, 2000i16.wrapping_add(ry));
    assert_eq!(al.worldz, 3000i16.wrapping_add(rz));
    assert_eq!(al.vx, al.worldx);
    assert_eq!(al.vy, al.worldy);
    assert_eq!(al.vz, al.worldz);
    // Not the old deterministic idx hash.
    assert_ne!(rx, ((idx as i16).wrapping_mul(37)) << 2);
}

/// Medium #21: spacepilonP state-0 uses Achase (toward-zero), not >>3 floor.
#[test]
fn spacepilonp_state0_achase_relposy() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let child = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, child, 1));
    g.objs.aliens[child as usize].stratstate = 0;
    g.objs.aliens[child as usize].relposy = 0;
    g.objs.aliens[child as usize].sbyte2 = 0;

    let mut expect = 0u8;
    achase_angle(&mut expect, SPACEPILON_RELPOSY_TGT, 3);

    spacepilonP_strat(&mut g, child);
    assert_eq!(g.objs.aliens[child as usize].relposy, expect);
    assert_ne!(expect, 0u8.wrapping_sub(0) >> 3); // not a no-op floor path
}

/// Medium #22: boss7fall detach + bossA 3-piece breakup present.
#[test]
fn death_sequences_boss7fall_and_bossa_breakup() {
    // boss7 hatch exp → fall init (bounce count 2, colldisable, detached).
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let hatch = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, hatch, 1));
    g.objs.aliens[hatch as usize].roty = 0;
    g.objs.aliens[hatch as usize].worldy = -100;
    boss7hatchexp_istrat(&mut g, hatch);
    {
        let al = &g.objs.aliens[hatch as usize];
        assert_ne!(al.sflags & ASF_COLLDISABLE, 0);
        assert_eq!(al.vel, 10);
        assert_eq!(al.sbyte2, 2); // bounce count after fall init
        assert!(al.expstratptr.is_some());
    }

    // bossAexp sbyte2==0 → 3-piece breakup (mother + L + R pods).
    let mut g2 = Game::new();
    let tank = spawn(&mut g2);
    g2.objs.aliens[tank as usize].sbyte2 = 0;
    g2.objs.aliens[tank as usize].worldx = 50;
    let before = g2.objs.aliens.iter().filter(|a| a.active).count();
    bossaexp_strat(&mut g2, tank);
    let after = g2.objs.aliens.iter().filter(|a| a.active).count();
    assert!(
        after >= before + 2,
        "bossAexp2 must spawn L/R breakup pieces (before={before} after={after})"
    );
}

/// Medium #23: bossa intro roty+1 every 2 frames; vx decel only when worldx<=210.
#[test]
fn bossa_intro_roty_and_vx_decel() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].roty = 10;
    g.objs.aliens[idx as usize].vx = -40;
    g.objs.aliens[idx as usize].worldx = 300; // >210: no decel yet
    g.vars.gameframe = 0; // even → roty++
    bossa_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].roty, 11);
    assert_eq!(g.objs.aliens[idx as usize].vx, -40);

    g.objs.aliens[idx as usize].worldx = 200; // <=210
    g.vars.gameframe = 1; // odd → no roty++
    bossa_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].roty, 11);
    assert_eq!(g.objs.aliens[idx as usize].vx, -39);
}

/// Medium #24: staydist re-runs every tick (already covered in ea_units; re-assert).
#[test]
fn staydist_tracks_pviewposz_each_tick() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.write_ext16(wm::PVIEWPOSZ, 1200u16);
    g.objs.aliens[idx as usize].sword1 = -200;
    ground::strat_staydist_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 1000);
    let sid = g.objs.aliens[idx as usize]
        .stratptr
        .expect("staydist keeps tick strat");
    g.vars.write_ext16(wm::PVIEWPOSZ, 1300u16);
    g.call_strat(sid, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 1100);
}

/// Medium #25: bossFCdie2 rubble X offset <<1.
#[test]
fn bossfcdie2_rubble_x_offset_asl() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 500;
    g.objs.aliens[idx as usize].worldy = 0;
    g.objs.aliens[idx as usize].worldz = 1000;
    g.vars.rng = [0xAB, 0xCD, 0xEF, 0x01];
    g.vars.gameframe = 1; // odd: skip rotz achase branch noise

    let mut predict = GameVars::default();
    predict.rng = g.vars.rng;
    let rx = (((sf_random(&mut predict) & 0xFF) as i16) - 127) << 1;
    let ry = ((sf_random(&mut predict) & 0xFF) as i16) - 127;
    let rz = ((sf_random(&mut predict) & 0xFF) as i16) - 127;

    bossfcdie2_strat(&mut g, idx);
    let exp = g
        .objs
        .aliens
        .iter()
        .enumerate()
        .find(|(i, a)| *i != idx as usize && a.active)
        .expect("rubble exp");
    assert_eq!(exp.1.worldx, 500i16.wrapping_add(rx));
    assert_eq!(exp.1.worldy, 0i16.wrapping_add(ry));
    assert_eq!(exp.1.worldz, 1000i16.wrapping_add(rz));
    // X must be the <<1 scale (not unscaled rnd-127 alone).
    let unscaled_x = rx >> 1;
    assert_ne!(rx, unscaled_x);
}

/// Medium #26: boss7 parent motion yaw from sbyte2, not roty.
#[test]
fn boss7_parent_yaw_from_sbyte2_not_roty() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sbyte2 = 0; // motion yaw
    g.objs.aliens[idx as usize].roty = 64; // hatch-facing only
    g.objs.aliens[idx as usize].rotx = 0;
    g.objs.aliens[idx as usize].vel = 50;
    g.objs.aliens[idx as usize].sbyte3 = 0; // sintab[0]=0, costab[0]=127
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = 0;
    g.objs.aliens[idx as usize].worldz = 2000;
    g.vars.pviewvelz = 0;

    let mut yaw0 = g.objs.aliens[idx as usize];
    yaw0.roty = 0;
    strat_gen_vecs_3d(&mut yaw0);

    let mut yaw64 = g.objs.aliens[idx as usize];
    yaw64.roty = 64;
    strat_gen_vecs_3d(&mut yaw64);
    assert_ne!(yaw0.vx, yaw64.vx, "fixture: yaw 0 vs 64 must differ");

    let dy = adiv2n(SINTAB[0] as i16, 3);
    let dz = adiv2n(COSTAB[0] as i16, 1);
    let x0 = g.objs.aliens[idx as usize].worldx;
    let y0 = g.objs.aliens[idx as usize].worldy;
    let z0 = g.objs.aliens[idx as usize].worldz;

    boss7d_strat(&mut g, idx);

    // boss7d: world -= sintab scales, then parent_cont gen_vecs(sbyte2)+apply.
    // No children → alldead skips add_player_z after velocity apply.
    assert_eq!(g.objs.aliens[idx as usize].worldx, x0.wrapping_add(yaw0.vx));
    assert_eq!(
        g.objs.aliens[idx as usize].worldy,
        y0.wrapping_sub(dy).wrapping_add(yaw0.vy)
    );
    assert_eq!(
        g.objs.aliens[idx as usize].worldz,
        z0.wrapping_sub(dz).wrapping_add(yaw0.vz)
    );
}
