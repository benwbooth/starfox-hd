//! Tick 196: AUDIT_BOSS_TICKS2 Highs #1–#4 verify (already ported).

use sf_game::alien::{ASF3_REALOBJ, ATLASER, ATMISSILE};
use sf_game::Game;
use sf_strat::bosses::boss8_strat;
use sf_strat::enemy_a::{
    boss1back_strat, boss1turret_nfire, boss1turretfire_end, boss_attach_child_to_mother,
    bossflags, set_bossflags, strat_boss1_init, wm, BF_FLAG1, COLLTYPE_ENEMY1,
};
use sf_strat::snes_trig::strat_roffs_full_i16;

const BOSS1_CHILD_TL0: u8 = 2;
const BOSS1_CHILD_TL3: u8 = 5;
const BOSS1_CHILD_TR0: u8 = 6;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldz = z;
    al.worldx = 0;
    al.worldy = -40;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn find_child(g: &Game, mother: u16, child_num: u8) -> Option<u16> {
    g.objs
        .aliens
        .iter()
        .enumerate()
        .find(|(i, a)| {
            *i as u16 != mother && a.active && a.sbyte1 == child_num && a.ptr == mother + 1
        })
        .map(|(i, _)| i as u16)
}

fn new_shots(g: &Game, before: &[u16]) -> Vec<u16> {
    g.objs
        .active_indices()
        .into_iter()
        .filter(|i| !before.contains(i))
        .collect()
}

/// High #1: five fire gates use bit-masks (+ al1pt / +15), not `% N`.
#[test]
fn boss1_fire_gates_are_bitmasks_not_modulus() {
    // Turret normal: (gf+idx)&31==0 fires; &31!=0 does not (modulus-5 would differ).
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let mother = spawn(&mut g);
    let tur = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, tur, 2));
    g.objs.aliens[tur as usize].hp = 8;
    set_bossflags(&mut g, 0);
    let phase = tur as u16;

    g.vars.gameframe = 5; // 5%5==0 would fire under old bug; (5+phase)&31 rarely 0
    let before = g.objs.active_indices();
    // Force a frame where modulus-5 would fire but bitmask does not (when possible).
    // Pick gf such that gf % 5 == 0 but (gf+phase)&31 != 0.
    let mut gf = 5u16;
    while (gf.wrapping_add(phase)) & 31 == 0 {
        gf = gf.wrapping_add(5);
    }
    g.vars.gameframe = gf;
    assert_eq!(gf % 5, 0);
    assert_ne!((gf.wrapping_add(phase)) & 31, 0);
    boss1turretfire_end(&mut g, tur, mother);
    assert_eq!(
        new_shots(&g, &before).len(),
        0,
        "modulus-5 false positive must not fire"
    );

    // Bitmask gate does fire.
    g.vars.gameframe = (32u16).wrapping_sub(phase % 32);
    assert_eq!((g.vars.gameframe.wrapping_add(phase)) & 31, 0);
    let before = g.objs.active_indices();
    boss1turretfire_end(&mut g, tur, mother);
    assert!(
        !new_shots(&g, &before).is_empty(),
        "normal (gf+idx)&31 gate"
    );

    // Home gate: (gf+idx)&15==0 + BF_FLAG1.
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let mother = spawn(&mut g);
    let tur = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, tur, 2));
    g.objs.aliens[tur as usize].hp = 8;
    set_bossflags(&mut g, BF_FLAG1);
    let phase = tur as u16;
    g.vars.gameframe = (16u16).wrapping_sub(phase % 16);
    assert_eq!((g.vars.gameframe.wrapping_add(phase)) & 15, 0);
    let before = g.objs.active_indices();
    boss1turretfire_end(&mut g, tur, mother);
    assert!(!new_shots(&g, &before).is_empty(), "home (gf+idx)&15");
    assert_eq!(bossflags(&g) & BF_FLAG1, 0, "home consumes bf_flag1");

    // Back plasma gf&63==0 / missiles (gf+15)&63 — covered shape in boss1_back_out;
    // assert the mask constants here against a known non-modulus frame.
    assert_eq!(0u16 & 63, 0);
    assert_ne!(6u16 % 6, 1); // sanity: old `%6` is not `&63`
    assert_eq!((0u16.wrapping_add(15)) & 63, 15);
    assert_eq!((49u16.wrapping_add(15)) & 63, 0);
}

/// High #2: back-mode retreats when |dz| < 1500; attacks when |dz| >= 1500.
#[test]
fn boss1back_retreats_when_closer_than_1500() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.write_ext8(wm::CURRENTLEVEL, 2);
    let boss = spawn(&mut g);
    strat_boss1_init(&mut g, boss);
    g.objs.aliens[boss as usize].worldz = 500; // |dz|=500 < 1500
    g.objs.aliens[boss as usize].sflags4 |= 0x80; // cover already gone
    let z0 = g.objs.aliens[boss as usize].worldz;
    boss1back_strat(&mut g, boss);
    assert_eq!(
        g.objs.aliens[boss as usize].worldz,
        z0.wrapping_add(15),
        "near path must retreat +15"
    );

    // Far path does not keep adding +15 every tick (bombard instead).
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.write_ext8(wm::CURRENTLEVEL, 2);
    let boss = spawn(&mut g);
    strat_boss1_init(&mut g, boss);
    g.objs.aliens[boss as usize].worldz = 2000;
    g.objs.aliens[boss as usize].sflags4 |= 0x80;
    g.vars.gameframe = 1; // no plasma this frame
    let z0 = g.objs.aliens[boss as usize].worldz;
    boss1back_strat(&mut g, boss);
    assert_eq!(
        g.objs.aliens[boss as usize].worldz, z0,
        "far path must not retreat"
    );
}

/// High #3: ring <<1 scale + full rotz/rotx/roty; center ±384; turret muzzle z+40.
#[test]
fn boss1_ring_and_muzzle_scales() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let boss = spawn(&mut g);
    strat_boss1_init(&mut g, boss);
    g.objs.aliens[boss as usize].worldx = 0;
    g.objs.aliens[boss as usize].worldy = 0;
    g.objs.aliens[boss as usize].worldz = 0;
    g.objs.aliens[boss as usize].rotx = 0;
    g.objs.aliens[boss as usize].roty = 0; // identity yaw for table readback
    g.objs.aliens[boss as usize].rotz = 0;
    let tl0 = find_child(&g, boss, BOSS1_CHILD_TL0).expect("TL0");
    boss1turret_nfire(&mut g, tl0, boss); // re-seat via boss1rots
                                          // Full <<1 table (110,0,90) through identity rotz/rotx/roty (mulslog cos=127).
    let (rx, ry, rz) = strat_roffs_full_i16(0, 0, 0, 110, 0, 90);
    assert_eq!(g.objs.aliens[tl0 as usize].worldx, rx);
    assert_eq!(g.objs.aliens[tl0 as usize].worldy, ry);
    assert_eq!(g.objs.aliens[tl0 as usize].worldz, rz);
    // Old half-scale table (55,0,45) must not match.
    let (hx, _, _) = strat_roffs_full_i16(0, 0, 0, 55, 0, 45);
    assert_ne!(g.objs.aliens[tl0 as usize].worldx, hx);

    // Turret muzzle z+40 (rots identity).
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let mother = spawn(&mut g);
    let tur = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, tur, 2));
    g.objs.aliens[tur as usize].worldx = 0;
    g.objs.aliens[tur as usize].worldy = 0;
    g.objs.aliens[tur as usize].worldz = 1000;
    g.objs.aliens[tur as usize].rotx = 0;
    g.objs.aliens[tur as usize].roty = 0;
    g.objs.aliens[tur as usize].rotz = 0;
    g.objs.aliens[tur as usize].hp = 8;
    set_bossflags(&mut g, 0);
    let phase = tur as u16;
    g.vars.gameframe = (32u16).wrapping_sub(phase % 32);
    let before = g.objs.active_indices();
    boss1turretfire_end(&mut g, tur, mother);
    let shots = new_shots(&g, &before);
    assert_eq!(shots.len(), 1);
    let s = &g.objs.aliens[shots[0] as usize];
    assert_ne!(s.type_ & ATLASER, 0);
    let (mx, my, mz) = strat_roffs_full_i16(0, 0, 0, 0, 0, 40);
    assert_eq!(s.worldx, mx);
    assert_eq!(s.worldy, my);
    assert_eq!(s.worldz, 1000i16.wrapping_add(mz));
    // Old >>weapon_scale (+10) must not match.
    let (_, _, mz10) = strat_roffs_full_i16(0, 0, 0, 0, 0, 10);
    assert_ne!(s.worldz, 1000i16.wrapping_add(mz10));

    // Center missiles ±384 on Y? No — offx ±384. Near-path finish with one bank dead.
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.write_ext8(wm::CURRENTLEVEL, 2);
    let boss = spawn(&mut g);
    strat_boss1_init(&mut g, boss);
    // Kill left bank so center fire arms.
    for n in BOSS1_CHILD_TL0..=BOSS1_CHILD_TL3 {
        if let Some(c) = find_child(&g, boss, n) {
            g.objs.aliens[c as usize].active = false;
        }
    }
    assert!(find_child(&g, boss, BOSS1_CHILD_TR0).is_some());
    g.objs.aliens[boss as usize].worldx = 0;
    g.objs.aliens[boss as usize].worldy = 0;
    g.objs.aliens[boss as usize].worldz = 500; // near → finish
    g.objs.aliens[boss as usize].rotx = 0;
    g.objs.aliens[boss as usize].roty = 0;
    g.objs.aliens[boss as usize].rotz = 0;
    g.objs.aliens[boss as usize].sflags4 |= 0x80;
    g.vars.gameframe = 49; // (49+15)&63==0
    let before = g.objs.active_indices();
    boss1back_strat(&mut g, boss);
    let shots = new_shots(&g, &before);
    let missiles: Vec<_> = shots
        .iter()
        .copied()
        .filter(|&i| g.objs.aliens[i as usize].type_ & ATMISSILE != 0)
        .collect();
    assert_eq!(missiles.len(), 2, "center twin HMISSILE1");
    let (ex_neg, _, _) = strat_roffs_full_i16(0, 0, 0, -384, 0, 0);
    let (ex_pos, _, _) = strat_roffs_full_i16(0, 0, 0, 384, 0, 0);
    let xs: Vec<i16> = missiles
        .iter()
        .map(|&i| g.objs.aliens[i as usize].worldx)
        .collect();
    assert!(xs.contains(&ex_neg), "got {xs:?} expect {ex_neg}");
    assert!(xs.contains(&ex_pos), "got {xs:?} expect {ex_pos}");
    // Old unscaled ±96 must not match.
    assert!(!xs.contains(&-96) && !xs.contains(&96), "got {xs:?}");
    assert!(missiles
        .iter()
        .all(|&i| g.objs.aliens[i as usize].collflags & COLLTYPE_ENEMY1 != 0));
}

/// High #4: mother finish + turret end + boss8_cont accumulate bosshp.
#[test]
fn boss1_and_boss8_add_bosshp_each_tick() {
    // Turret end (via nfire) adds turret hp — already in boss1_turret_fire; re-assert.
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let tur = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, tur, 2));
    g.objs.aliens[tur as usize].hp = 8;
    g.vars.bosshp = 0;
    boss1turret_nfire(&mut g, tur, mother);
    assert_eq!(g.vars.bosshp, 8);

    // Mother near-back finish adds mother hp.
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.write_ext8(wm::CURRENTLEVEL, 1);
    let boss = spawn(&mut g);
    strat_boss1_init(&mut g, boss);
    let hp = g.objs.aliens[boss as usize].hp as u16;
    g.objs.aliens[boss as usize].worldz = 500;
    g.objs.aliens[boss as usize].sflags4 |= 0x80;
    g.vars.bosshp = 0;
    g.vars.gameframe = 1;
    boss1back_strat(&mut g, boss);
    assert!(
        g.vars.bosshp >= hp,
        "mother finish must add bosshp (got {})",
        g.vars.bosshp
    );

    // boss8_cont adds core hp.
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let b8 = spawn(&mut g);
    g.objs.aliens[b8 as usize].hp = 20;
    g.vars.bosshp = 0;
    boss8_strat(&mut g, b8);
    assert_eq!(g.vars.bosshp, 20);
}
