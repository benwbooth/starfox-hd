//! Tick 135: boss1 Mediums 6–8 verify (back-mode fire axes + beqdec out cycle)
//! + ENDSEQ nosetport3 covered in shell tests.

use sf_game::alien::{ObjectVisualKind, ASF3_REALOBJ, ASF4_INVISIBLE};
use sf_game::Game;
use sf_strat::enemy_a::{
    boss1back_strat, boss1out_strat, strat_boss1_init, wm, SH_BOUNCYBALL, SH_MISSILE,
};

const DEG180: u8 = 128;
const DEG45: u8 = 32;
const DEG11: u8 = 8;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].worldx = 0;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[0].sflags3 |= ASF3_REALOBJ;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

/// AUDIT_BOSS_TICKS2 Medium #8: `s_beqdec` tests before dec — first out-pass
/// with sbyte3=1 goes to normal (not inclose); second pass (sbyte3=0) → inclose.
#[test]
fn boss1out_beqdec_enters_inclose_on_second_pass() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.write_ext8(wm::CURRENTLEVEL, 1);
    let boss = spawn(&mut g);
    strat_boss1_init(&mut g, boss);
    // Far enough that out-strat doesn't hold at boss1_end (|dz| >= 1500).
    g.objs.aliens[boss as usize].worldz = 2000;
    g.objs.aliens[boss as usize].sbyte3 = 1;
    // Arm out strat directly.
    let out = g.world.register_strategy(boss1out_strat);
    g.objs.aliens[boss as usize].stratptr = Some(out);

    boss1out_strat(&mut g, boss);
    assert_eq!(
        g.objs.aliens[boss as usize].sbyte3, 0,
        "first pass decs 1→0"
    );
    // After first pass: normal_init (sbyte2=30), not inclose (which sets sbyte3=2).
    assert_ne!(
        g.objs.aliens[boss as usize].sbyte3, 2,
        "first pass must not enter inclose"
    );

    // Re-arm out with sbyte3=0 (as after the first out→normal→…→out cycle).
    g.objs.aliens[boss as usize].stratptr = Some(out);
    g.objs.aliens[boss as usize].sbyte3 = 0;
    g.objs.aliens[boss as usize].worldz = 2000;
    boss1out_strat(&mut g, boss);
    assert_eq!(
        g.objs.aliens[boss as usize].sbyte3, 2,
        "second pass (sbyte3==0) → inclose_init sets sbyte3=#2"
    );
}

/// AUDIT_BOSS_TICKS2 Medium #6/#7: back-mode HPLASMA uses firer rots + (rnd&15)-7
/// (no aim); HMISSILE1 pair uses pitch ±(deg45-deg11) on firer, yaw stays deg180.
#[test]
fn boss1back_fires_from_firer_rots_not_aim() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.write_ext8(wm::CURRENTLEVEL, 2); // hard: both missiles
    let boss = spawn(&mut g);
    strat_boss1_init(&mut g, boss);
    g.objs.aliens[boss as usize].worldz = 2000; // |dz| >= 1500 → attack path
    g.objs.aliens[boss as usize].rotx = 10;
    g.objs.aliens[boss as usize].roty = DEG180;
    g.objs.aliens[boss as usize].sflags4 |= 0x80; // BOSS1_PARENT_FLAG_COVER_GONE
    g.vars.gameframe = 0; // gf&63==0 → plasma; (0+15)&63!=0 → no missiles this frame

    let before: Vec<u16> = g.objs.active_indices();
    boss1back_strat(&mut g, boss);
    let shots: Vec<u16> = g
        .objs
        .active_indices()
        .into_iter()
        .filter(|i| !before.contains(i))
        .collect();
    assert_eq!(
        shots.len(),
        1,
        "exactly one HPLASMA on gf&63==0, got {shots:?}"
    );
    let s = &g.objs.aliens[shots[0] as usize];
    assert_eq!(s.shape, SH_BOUNCYBALL);
    assert_eq!(s.sflags4 & ASF4_INVISIBLE, 0);
    assert_eq!(s.visual_kind, ObjectVisualKind::ScaledSprite);
    // homingflat_Istrat (GSTRATS.ASM:1723): copy weapon rots into sbyte1/2,
    // then force visual rotx=0 / roty=deg180 for the flat sprite. Spread lives
    // in the sbytes (homingflat_strat gens vecs from them).
    assert_eq!(s.rotx, 0, "visual rotx forced 0");
    assert_eq!(s.roty, DEG180, "visual roty forced deg180");
    let dp = s.sbyte2.wrapping_sub(10) as i8; // sbyte2 = pitch
    let dy = s.sbyte1.wrapping_sub(DEG180) as i8; // sbyte1 = yaw
    assert!(
        (-7..=8).contains(&dp),
        "plasma pitch spread in sbyte2, got Δ={dp} sb2={}",
        s.sbyte2
    );
    assert!(
        (-7..=8).contains(&dy),
        "plasma yaw spread in sbyte1, got Δ={dy} sb1={}",
        s.sbyte1
    );

    // Missile frame: (gf+15)&63==0 → gf=49.
    g.vars.gameframe = 49;
    let before: Vec<u16> = g.objs.active_indices();
    boss1back_strat(&mut g, boss);
    let missiles: Vec<u16> = g
        .objs
        .active_indices()
        .into_iter()
        .filter(|i| !before.contains(i))
        .collect();
    assert_eq!(missiles.len(), 2, "hard route fires two HMISSILE1");
    assert!(
        missiles.iter().all(|&i| {
            let missile = &g.objs.aliens[i as usize];
            missile.shape == SH_MISSILE && missile.sflags4 & ASF4_INVISIBLE == 0
        }),
        "HMISSILE1 pair must use the visible missile mesh"
    );

    let pitches: Vec<u8> = missiles
        .iter()
        .map(|&i| g.objs.aliens[i as usize].rotx)
        .collect();
    let yaws: Vec<u8> = missiles
        .iter()
        .map(|&i| g.objs.aliens[i as usize].roty)
        .collect();
    let expect_hi = 10u8.wrapping_add(DEG45 - DEG11);
    let expect_lo = 10u8.wrapping_sub(DEG45 - DEG11);
    assert!(
        pitches.contains(&expect_hi) && pitches.contains(&expect_lo),
        "missile pitches = firer.rotx ±24 ({expect_lo}/{expect_hi}), got {pitches:?}"
    );
    assert!(
        yaws.iter().all(|&y| y == DEG180),
        "missile yaw stays firer deg180, got {yaws:?}"
    );
}
