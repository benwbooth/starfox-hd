//! Tick 193: AUDIT_ENEMY_B Highs #10–#19 verify (already ported).

use sf_game::alien::ASF_COLLDISABLE;
use sf_game::Game;
use sf_strat::common::{strat_chase_proportional, strat_gen_vecs_3d};
use sf_strat::enemy_a::{achase_angle, boss_attach_child_to_mother, COLLTYPE_ENEMY1};
use sf_strat::enemy_b::{
    boss7d_strat, bossfa_istrat, bossfa_strat, bossfb_istrat, bossfb_strat, bossfc2_strat,
    bossfc_strat, strat_bossf_init,
};
use sf_strat::snes_trig::{COSTAB, SINTAB};

const ASF2_SFLAG1: u8 = 0x10;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldz = z;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn active_count(g: &Game) -> usize {
    g.objs.aliens.iter().filter(|a| a.active).count()
}

fn adiv2n(v: i16, n: u32) -> i16 {
    let mut x = v;
    for _ in 0..n {
        x /= 2; // toward zero
    }
    x
}

/// High #10: Achase toward-zero (0→100 rate 3 → +12, not +13).
#[test]
fn achase_toward_zero_0_to_100_rate3() {
    let mut cur = 0u8;
    achase_angle(&mut cur, 100, 3);
    assert_eq!(cur, 12);
}

/// High #13: bossFA `s_scale_alvar vz,1` = ASL (×2) after gen_vecs.
#[test]
fn bossfa_vz_scale_asl_x2() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let mother = spawn(&mut g);
    let fa = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, fa, 1));
    bossfa_istrat(&mut g, fa);
    // Non-combine path: gen_vecs then vz<<=1 then apply.
    g.objs.aliens[mother as usize].sflags2 &= !ASF2_SFLAG1;
    g.objs.aliens[fa as usize].worldx = 0;
    g.objs.aliens[fa as usize].worldy = 0;
    g.objs.aliens[fa as usize].worldz = 1000;
    g.objs.aliens[fa as usize].rotx = 0;
    g.objs.aliens[fa as usize].roty = 0;
    g.objs.aliens[fa as usize].vel = 40;
    g.objs.aliens[fa as usize].stratstate = 2; // skip state-0/1 intro

    let mut probe = g.objs.aliens[fa as usize];
    strat_gen_vecs_3d(&mut probe);
    let expect_dz = probe.vz << 1;

    let z0 = g.objs.aliens[fa as usize].worldz;
    bossfa_strat(&mut g, fa);
    assert_eq!(
        g.objs.aliens[fa as usize].worldz.wrapping_sub(z0),
        expect_dz,
        "vz must be ASL×2 before apply"
    );
}

/// High #14: combine path uses Achase rate-4 (strat_chase_proportional).
#[test]
fn bossfa_combine_uses_chase_rate4() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let mother = spawn(&mut g);
    g.objs.aliens[mother as usize].worldx = 100;
    g.objs.aliens[mother as usize].worldy = -200;
    g.objs.aliens[mother as usize].worldz = 3000;
    g.objs.aliens[mother as usize].sflags2 |= ASF2_SFLAG1;
    let fa = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, fa, 1));
    bossfa_istrat(&mut g, fa);
    g.objs.aliens[fa as usize].worldx = 0;
    g.objs.aliens[fa as usize].worldy = 0;
    g.objs.aliens[fa as usize].worldz = 0;

    let targ_z = 3000i16.wrapping_add(10 << 3); // BOSSF_SCALE=3
    let expect_z = strat_chase_proportional(0, targ_z, 4);
    bossfa_strat(&mut g, fa);
    assert_eq!(g.objs.aliens[fa as usize].worldz, expect_z);
}

/// High #16: boss7d loop uses sintab>>3 / costab>>1 (not sin*8/cos*2).
#[test]
fn boss7d_loop_sintab_scaled() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sbyte3 = 64; // SINTAB[64]=127, COSTAB[64]=0
    g.objs.aliens[idx as usize].sbyte2 = 0;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].worldy = 0;
    g.objs.aliens[idx as usize].worldz = 1000;
    g.objs.aliens[idx as usize].roty = 128; // DEG180 — skip achase motion noise

    let dy = adiv2n(SINTAB[64] as i16, 3);
    let dz = adiv2n(COSTAB[64] as i16, 1);
    assert_eq!(dy, adiv2n(127, 3)); // ±15 domain
    assert_eq!(dz, 0);

    boss7d_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, 0i16.wrapping_sub(dy));
    // parent_cont also moves z via vel=0 gen_vecs; only the loop dz applies here.
    assert_eq!(g.objs.aliens[idx as usize].worldz, 1000i16.wrapping_sub(dz));
}

/// High #17: FC intro 200-frame countdown gates state 0/1 (no immediate sink).
#[test]
fn bossfc_intro_countdown_gates_descent() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let boss = spawn(&mut g);
    strat_bossf_init(&mut g, boss);
    assert_eq!(g.objs.aliens[boss as usize].sbyte2, 200);
    let y0 = g.objs.aliens[boss as usize].worldy;
    let vy0 = g.objs.aliens[boss as usize].vy;

    bossfc_strat(&mut g, boss);
    assert_eq!(g.objs.aliens[boss as usize].sbyte2, 199);
    assert_eq!(g.objs.aliens[boss as usize].vy, vy0, "no state0 sink yet");
    assert_eq!(g.objs.aliens[boss as usize].worldy, y0);

    // Expire countdown: sbyte2=1 → dec to 0 → peg 1 and run state0.
    g.objs.aliens[boss as usize].sbyte2 = 1;
    g.objs.aliens[boss as usize].stratstate = 0;
    bossfc_strat(&mut g, boss);
    assert_eq!(g.objs.aliens[boss as usize].vy, -10);
}

/// High #18: FC2_cont holds smoke/Hplasma until sbyte2 >= 3.
#[test]
fn bossfc2_cont_holds_fire_until_3_turrets() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let boss = spawn(&mut g);
    // Boss ahead so objinfront turn does not fire; zdist >= 600.
    g.objs.aliens[boss as usize].worldz = 2000;
    g.objs.aliens[boss as usize].sbyte2 = 2; // < 3
    g.vars.gameframe = 10; // would be a fire frame if unlocked
    let before = active_count(&g);
    bossfc2_strat(&mut g, boss);
    assert_eq!(active_count(&g), before, "sb2<3 must not Hplasma");

    let mut g2 = Game::new();
    spawn_player(&mut g2, 0);
    let boss2 = spawn(&mut g2);
    g2.objs.aliens[boss2 as usize].worldz = 2000;
    g2.objs.aliens[boss2 as usize].sbyte2 = 3;
    g2.vars.gameframe = 10;
    let before2 = active_count(&g2);
    bossfc2_strat(&mut g2, boss2);
    assert!(
        active_count(&g2) > before2,
        "sb2>=3 on frame 10 must Hplasma"
    );
}

/// High #19: bossFB mines are live (ENEMY1 + hitflash/explode, no lifetime).
#[test]
fn bossfb_spawns_live_mines() {
    let mut g = Game::new();
    spawn_player(&mut g, 500); // zdist < 1400
    let mother = spawn(&mut g);
    g.objs.aliens[mother as usize].worldz = 1000;
    let fb = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, fb, 2));
    bossfb_istrat(&mut g, fb);
    g.objs.aliens[fb as usize].worldz = 1000;
    g.objs.aliens[fb as usize].stratstate = 2; // skip intro
    g.vars.gameframe = 0; // %4 == 0

    let before = active_count(&g);
    bossfb_strat(&mut g, fb);
    assert!(active_count(&g) > before, "mine spawned");

    let mine = (0..g.objs.aliens.len())
        .map(|i| i as u16)
        .find(|&i| {
            i != 0
                && i != mother
                && i != fb
                && g.objs.aliens[i as usize].active
                && g.objs.aliens[i as usize].hp == 2
        })
        .expect("mine slot");
    let al = &g.objs.aliens[mine as usize];
    assert_eq!(al.ap, 10);
    assert_ne!(al.collflags & COLLTYPE_ENEMY1, 0);
    assert!(al.collstratptr.is_some());
    assert!(al.expstratptr.is_some());
    assert!(al.stratptr.is_none());
    assert_eq!(
        al.sflags & ASF_COLLDISABLE,
        0,
        "must not be inert colldisable"
    );
}
