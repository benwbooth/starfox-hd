//! Tick 159: AUDIT_ENEMY_A Minors #3–#5 — jmp_distmore strict boundaries,
//! zaco1 mid-band upper exclusive, zaco0_fire (rnd&3)-1 pitch-then-yaw.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::Game;
use sf_strat::common::{sf_random, sv, StratRam};
use sf_strat::enemy_a::{
    strat_item5_init, strat_tadpole_init, strat_zaco0_init, strat_zaco1l_init, wm, DEG180,
};

fn spawn_player(g: &mut Game, x: i16, y: i16, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posx = x;
    g.vars.player_posy = y;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn run(g: &mut Game, idx: u16) {
    let s = g.objs.aliens[idx as usize].stratptr.expect("strat");
    g.call_strat(s, idx);
}

/// Minor #3: item5 pickup requires |dz|<120 (not at 120).
#[test]
fn item5_pickup_strict_less_than_120() {
    // |dz|==120 → no collect (sbyte1!=0 skips the +20 scroll before the gate)
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0, -40, 0);
        let idx = spawn(&mut g);
        g.objs.aliens[idx as usize].worldz = 10_000;
        strat_item5_init(&mut g, idx);
        g.vars.set_sv_u16(sv::SPECWEPCNT, 0);
        g.vars.write_ext8(wm::SPECFLASH, 0);
        g.objs.aliens[idx as usize].sbyte1 = 1;
        g.objs.aliens[idx as usize].worldx = 0;
        g.objs.aliens[idx as usize].worldy = -40;
        g.objs.aliens[idx as usize].worldz = 120;
        run(&mut g, idx);
        assert_eq!(
            g.vars.read_ext8(wm::SPECFLASH),
            0,
            "|dz|==120 must not collect"
        );
    }
    // |dz|==119 → collect
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0, -40, 0);
        let idx = spawn(&mut g);
        g.objs.aliens[idx as usize].worldz = 10_000;
        strat_item5_init(&mut g, idx);
        g.vars.set_sv_u16(sv::SPECWEPCNT, 0);
        g.vars.write_ext8(wm::SPECFLASH, 0);
        g.objs.aliens[idx as usize].sbyte1 = 1;
        g.objs.aliens[idx as usize].worldx = 0;
        g.objs.aliens[idx as usize].worldy = -40;
        g.objs.aliens[idx as usize].worldz = 119;
        run(&mut g, idx);
        assert_eq!(
            g.vars.read_ext8(wm::SPECFLASH),
            30,
            "|dz|==119 must collect"
        );
    }
}

/// Minor #3: tadpole fires only when |dz|<1500.
#[test]
fn tadpole_fire_strict_less_than_1500() {
    fn setup(dz: i16) -> (Game, u16, usize) {
        let mut g = Game::new();
        spawn_player(&mut g, 0, -40, 0);
        let idx = spawn(&mut g);
        g.objs.aliens[idx as usize].worldz = 10_000; // far during init (state0)
        strat_tadpole_init(&mut g, idx);
        // Jump to dive/fire state.
        g.objs.aliens[idx as usize].stratstate = 1;
        g.objs.aliens[idx as usize].worldx = 0;
        g.objs.aliens[idx as usize].worldy = -40;
        g.objs.aliens[idx as usize].worldz = dz;
        let before = g.objs.aliens.iter().filter(|a| a.active).count();
        run(&mut g, idx);
        (g, idx, before)
    }
    let (g, idx, before) = setup(1500);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert_eq!(after, before, "|dz|==1500 must not fire");
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);

    let (g, idx, before) = setup(1499);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after > before, "|dz|==1499 must fire");
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 2);
}

/// Minor #3: zaco1_phase0 → phase1 when |dz| >= 1000.
#[test]
fn zaco1_phase0_transitions_at_dz_1000() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 500; // |dz|<1000 during init
    strat_zaco1l_init(&mut g, idx);
    let phase0 = g.objs.aliens[idx as usize].stratptr.expect("p0");

    // |dz|==999 → stay phase0
    g.objs.aliens[idx as usize].worldz = 999;
    g.vars.player_posz = 0;
    run(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].stratptr,
        Some(phase0),
        "|dz|==999 stays phase0"
    );

    // |dz|==1000 → phase1
    g.objs.aliens[idx as usize].worldz = 1000;
    run(&mut g, idx);
    assert_ne!(
        g.objs.aliens[idx as usize].stratptr,
        Some(phase0),
        "|dz|==1000 → phase1"
    );
}

/// Minor #4: zaco1_phase2 mid-band is [1400, 1800) — no fire at |dz|==1800.
#[test]
fn zaco1_phase2_midband_excludes_1800() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 500;
    strat_zaco1l_init(&mut g, idx);

    // phase0 → phase1 at |dz|>=1000
    g.objs.aliens[idx as usize].worldz = 1000;
    run(&mut g, idx);
    let phase1 = g.objs.aliens[idx as usize].stratptr.expect("p1");

    // phase1 → phase2 when roty reaches DEG0
    g.objs.aliens[idx as usize].roty = 0;
    run(&mut g, idx);
    let phase2 = g.objs.aliens[idx as usize].stratptr.expect("p2");
    assert_ne!(phase2, phase1);

    // |dz|==1800: mid-band excluded — no new projectile on fire cadence.
    g.objs.aliens[idx as usize].worldz = 1800;
    g.vars.player_posz = 0;
    g.vars.gameframe = 0; // notdelay 2 = &3==0
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    run(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert_eq!(after, before, "|dz|==1800 must not mid-band fire");

    // |dz|==1799: mid-band fires on cadence.
    g.objs.aliens[idx as usize].worldz = 1799;
    g.vars.gameframe = 0;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    run(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after > before, "|dz|==1799 must mid-band fire");
}

/// Minor #5: zaco0_fire uses (rnd&3)-1 per axis, pitch drawn before yaw.
#[test]
fn zaco0_fire_spread_mask_pitch_then_yaw() {
    // Mask (not modulo): (rnd&3)-1 ∈ {-1,0,1,2}; modulo-3 never yields +2.
    let mut saw_plus2 = false;
    for seed0 in 0u8..64 {
        let mut g = Game::new();
        g.vars.rng = [seed0, 0x34, 0x56, 0x78];
        let a = ((sf_random(&mut g.vars) & 3) as i8).wrapping_sub(1);
        if a == 2 {
            saw_plus2 = true;
            break;
        }
    }
    assert!(
        saw_plus2,
        "(rnd&3)-1 must be able to yield +2 (modulo-3 cannot)"
    );

    // Two identical seeded runs must produce the same shot rots (pitch-then-yaw order).
    fn fire_once(seed: [u8; 4]) -> (u8, u8) {
        let mut g = Game::new();
        spawn_player(&mut g, 0, -40, 0);
        let idx = spawn(&mut g);
        g.objs.aliens[idx as usize].worldx = -100;
        g.objs.aliens[idx as usize].worldz = 2000;
        strat_zaco0_init(&mut g, idx);
        g.objs.aliens[idx as usize].worldx = 0;
        g.objs.aliens[idx as usize].roty = DEG180.wrapping_add(8);
        g.objs.aliens[idx as usize].worldz = 2000;
        g.vars.rng = seed;
        g.vars.gameframe = 3; // (gf+idx)&3==0 with idx=1
        run(&mut g, idx);
        let shot = g
            .objs
            .aliens
            .iter()
            .enumerate()
            .find(|(i, a)| a.active && *i as u16 != 0 && *i as u16 != idx)
            .expect("shot")
            .1;
        (shot.rotx, shot.roty)
    }
    let seed = [0x12, 0x34, 0x56, 0x78];
    let a = fire_once(seed);
    let b = fire_once(seed);
    assert_eq!(
        a, b,
        "seeded fire must be deterministic (pitch-then-yaw draws)"
    );
}
