//! Tick 156: AUDIT_ENEMY_A Mediums #23–#24, #26–#33 — item5 HP0/specflash,
//! up1man sbyte3 gate, clship cont/warp, zaco1 spiral retain, friendexitbase
//! beqdec snd, gate2 rangexy, skillfly behind no-dec.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::vars::{PFM_SHADOWS, PSF2_PLAYERHP0};
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::enemy_a::{
    strat_clship_eartha_init, strat_clship_warpa_init, strat_friendexitbase_init, strat_gate2_init,
    strat_item5_init, strat_skillfly_init, strat_up1man_init, strat_zaco1l_init, wm,
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

/// Medium #23: item5 removes on PSF2_PLAYERHP0.
#[test]
fn item5_removes_when_player_hp0() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    strat_item5_init(&mut g, idx);
    g.vars.pshipflags2 |= PSF2_PLAYERHP0;
    run(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

/// Medium #24: collect sets specflash=30 when under bomb cap.
#[test]
fn item5_collect_sets_specflash_30() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    // Keep far during init (init falls into item5_strat same frame).
    g.objs.aliens[idx as usize].worldz = 10_000;
    strat_item5_init(&mut g, idx);
    g.vars.set_sv_u16(sv::SPECWEPCNT, 0);
    g.vars.write_ext8(wm::SPECFLASH, 0);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -40;
    g.objs.aliens[idx as usize].worldz = 0;
    run(&mut g, idx);
    assert_eq!(g.vars.read_ext8(wm::SPECFLASH), 30);
    assert_eq!(g.vars.sv_u16(sv::SPECWEPCNT), 1);
}

/// Medium #26: up1man static while sbyte3==0 (no worldz scroll).
#[test]
fn up1man_static_while_sbyte3_zero() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 100;
    strat_up1man_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 0);
    let z0 = g.objs.aliens[idx as usize].worldz;
    let rotz0 = g.objs.aliens[idx as usize].rotz;
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, z0);
    assert_eq!(g.objs.aliens[idx as usize].rotz, rotz0);

    // Once sbyte3 bumped, scroll can run (|dz|<1500).
    g.objs.aliens[idx as usize].sbyte3 = 1;
    g.objs.aliens[idx as usize].sbyte2 = 0; // no rotz spin
    run(&mut g, idx);
    assert!(
        g.objs.aliens[idx as usize].worldz > z0,
        "must scroll once sbyte3!=0"
    );
}

/// Medium #27: clship_cont space-boost countdown skips player chase.
#[test]
fn clship_cont_countdown_skips_chase() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = -1000;
    g.objs.aliens[idx as usize].worldy = -500;
    g.objs.aliens[idx as usize].worldz = 500;
    strat_clship_eartha_init(&mut g, idx);
    // Force into cont with FLAG1 + player sflag4 (0x80) + countdown.
    g.objs.aliens[idx as usize].sflags2 |= 0x10; // CLSHIP_FLAG1
    g.objs.aliens[0].sflags2 |= 0x80;
    g.objs.aliens[idx as usize].sbyte1 = 5;
    let y0 = g.objs.aliens[idx as usize].worldy;
    let z0 = g.objs.aliens[idx as usize].worldz;
    run(&mut g, idx);
    // Countdown path: only add_player_z — no Achase toward player y/z.
    assert_eq!(
        g.objs.aliens[idx as usize].worldy, y0,
        "must not chase worldy during countdown"
    );
    // worldz may get add_player_z (pviewvelz) but not the large Achase step.
    let dz = (g.objs.aliens[idx as usize].worldz as i32 - z0 as i32).abs();
    assert!(dz < 100, "countdown must not Achase worldz (dz={dz})");
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 4);
}

/// Medium #28: warp boost plays snd2=$32.
#[test]
fn clship_warp_boost_plays_sound() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    strat_clship_warpa_init(&mut g, idx);
    g.objs.aliens[idx as usize].sword1 = 0;
    g.objs.aliens[idx as usize].snd2 = 0;
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].snd2, 0x32);
    assert_eq!(g.objs.aliens[idx as usize].vel, 120);
}

/// Medium #30: leaving .circ keeps sword2/ptr spiral offsets.
#[test]
fn zaco1_phase2_retains_spiral_offsets_outside_circ() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    // Far enough for phase0 → phase1 (|dz|>1000).
    g.objs.aliens[idx as usize].worldz = 2000;
    strat_zaco1l_init(&mut g, idx);
    run(&mut g, idx); // phase0 → phase1

    // phase1: roty already at 0 → immediate transition to phase2.
    g.objs.aliens[idx as usize].roty = 0;
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[0].worldz = 0;
    run(&mut g, idx);

    // .circ band |dz|<1400 writes spiral offsets.
    g.objs.aliens[idx as usize].worldz = 100;
    g.objs.aliens[0].worldz = 0;
    g.objs.aliens[idx as usize].rotz = 64;
    g.objs.aliens[idx as usize].sbyte2 = 8;
    run(&mut g, idx);
    let sword2 = g.objs.aliens[idx as usize].sword2;
    let ptr = g.objs.aliens[idx as usize].ptr;
    assert!(
        sword2 != 0 || ptr != 0,
        "need .circ spiral offsets to test retain"
    );

    // Mid band 1400..1800 — must NOT zero sword2/ptr.
    g.objs.aliens[idx as usize].worldz = 1600;
    g.objs.aliens[0].worldz = 0;
    g.vars.gameframe = 1; // skip fire gate
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sword2, sword2);
    assert_eq!(g.objs.aliens[idx as usize].ptr, ptr);
}

/// Medium #31: beqdec — RIGHT while sbyte2>0 (dec), LEFT every frame at 0.
#[test]
fn friendexitbase_beqdec_snd_channels() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    strat_friendexitbase_init(&mut g, idx);
    // The source initializer falls through into the first strategy pass.
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 10);
    // Clear sbyte1 gate so body runs.
    g.objs.aliens[idx as usize].sbyte1 = 0;

    run(&mut g, idx); // 10→9, RIGHT
    assert_eq!(g.objs.aliens[idx as usize].snd1, 0xB1);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 9);

    // Drain to 0 (9 more RIGHT ticks: 9→8…→0).
    for _ in 0..9 {
        g.objs.aliens[idx as usize].sbyte1 = 0;
        run(&mut g, idx);
    }
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 0);
    assert_eq!(g.objs.aliens[idx as usize].snd1, 0xB1); // last dec 1→0 was RIGHT

    g.objs.aliens[idx as usize].sbyte1 = 0;
    run(&mut g, idx); // already 0 → LEFT
    assert_eq!(g.objs.aliens[idx as usize].snd1, 0x51);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 0);

    g.objs.aliens[idx as usize].sbyte1 = 0;
    run(&mut g, idx); // still LEFT (beqdec stays on .left)
    assert_eq!(g.objs.aliens[idx as usize].snd1, 0x51);
}

/// Medium #32: gate2 touch uses |dx|+|dy| < 60, not per-axis box.
#[test]
fn gate2_touch_uses_combined_rangexy() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    g.vars.playerflymode = PFM_SHADOWS;
    g.vars.minpmove_y = -10_000;
    // Heal path needs a live player collision box.
    g.vars.write_ext16(wm::PCBOXOBJ_B, 0);
    g.objs.aliens[0].hp = 20;
    let idx = spawn(&mut g);
    // Keep far during init (init falls into gate2_strat same frame).
    g.objs.aliens[idx as usize].worldz = 10_000;
    strat_gate2_init(&mut g, idx);
    g.objs.aliens[idx as usize].sflags2 &= !0x40;
    g.objs.aliens[idx as usize].worldy = -40;
    g.objs.aliens[idx as usize].worldz = 0; // |dz|<60

    // Per-axis box would touch (dx=50, dy=20 both <=60); rangexy=70 >= 60 → no.
    // (After shadows floor clamp gate worldy=-60; player at -40 → dy=20.)
    g.objs.aliens[idx as usize].worldx = 50;
    g.objs.aliens[0].worldx = 0;
    g.objs.aliens[0].worldy = -40;
    run(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].sflags2 & 0x40,
        0,
        "rangexy=70 must not touch"
    );

    // rangexy < 60 → touch. After shadows floor clamp worldy=-60:
    // dy=|-60-(-40)|=20, dx=20 → sum=40.
    g.objs.aliens[idx as usize].worldx = 20;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[idx as usize].sflags2 &= !0x40;
    run(&mut g, idx);
    assert_ne!(
        g.objs.aliens[idx as usize].sflags2 & 0x40,
        0,
        "rangexy=40 must touch"
    );
}

/// Medium #33: flew-behind removal does not decrement skillfly.
#[test]
fn skillfly_behind_removes_without_decrement() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 1000);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 500; // behind player after +1000 scroll
    strat_skillfly_init(&mut g, idx);
    let before = g.vars.read_ext8(wm::SKILLFLY);
    assert!(before >= 1);

    // Far enough that catch radius fails; after worldz+=1000, player still ahead.
    g.objs.aliens[idx as usize].worldz = 0;
    g.objs.aliens[0].worldz = 5000;
    g.objs.aliens[idx as usize].sword1 = 20;
    run(&mut g, idx);
    // After +1000 → z=1000; player 5000 >= 1000 → behind path.
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(
        g.vars.read_ext8(wm::SKILLFLY),
        before,
        "behind path must not dec skillfly"
    );
}
