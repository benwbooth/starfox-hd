//! Tick 151: AUDIT_ENEMY_A High #5 — `s_jmp_higher` / `s_jmp_lower` worldy
//! half-spaces (smaller y = higher). Clamp/fire/land when `worldy >= v`.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::vars::PFM_SHADOWS;
use sf_game::Game;
use sf_strat::enemy_a::{
    strat_gate2_init, strat_zaco1l_init, strat_zaco3_init, strat_zacos_init, zaco2_istrat,
};

fn spawn_player(g: &mut Game) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = 0;
    al.worldy = -40;
    al.worldz = 0;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = 0;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn run_strat(g: &mut Game, idx: u16) {
    let s = g.objs.aliens[idx as usize].stratptr.expect("strat");
    g.call_strat(s, idx);
}

/// gate2: floor clamp at -60 when shadows on and worldy >= -60.
#[test]
fn gate2_clamps_floor_when_not_higher_than_neg60() {
    let mut g = Game::new();
    spawn_player(&mut g);
    g.vars.playerflymode = PFM_SHADOWS;
    // Keep the out-of-bounds chase from rewriting worldy (gate2_strat lead-in).
    g.vars.minpmove_y = -10_000;
    let idx = spawn(&mut g);
    strat_gate2_init(&mut g, idx);

    g.objs.aliens[idx as usize].worldy = -50; // >= -60 → clamp
    run_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -60);

    g.objs.aliens[idx as usize].worldy = -70; // < -60 → skip clamp
    run_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -70);
}

/// zaco2_cont: ground bounce when worldy >= 0.
#[test]
fn zaco2_cont_bounces_when_worldy_not_higher_than_zero() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn(&mut g);
    zaco2_istrat(&mut g, idx);
    // Keep sbyte1 > 0 so only zaco2_cont runs (no loop transition).
    g.objs.aliens[idx as usize].sbyte1 = 5;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].vx = 0;
    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].vz = 0;

    g.objs.aliens[idx as usize].worldy = 12;
    let rotx0 = g.objs.aliens[idx as usize].rotx;
    run_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, 0);
    assert_eq!(
        g.objs.aliens[idx as usize].rotx,
        (rotx0 as i8).wrapping_neg() as u8
    );

    g.objs.aliens[idx as usize].sbyte1 = 5;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].vx = 0;
    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].vz = 0;
    g.objs.aliens[idx as usize].worldy = -8;
    run_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -8);
}

/// zacos_phase0: pitch/fire block when worldy >= player_posy-800.
#[test]
fn zacos_phase0_fires_only_when_not_higher_than_target() {
    let mut g = Game::new();
    spawn_player(&mut g);
    g.vars.player_posy = 0; // target_y = -800
    let idx = spawn(&mut g);
    strat_zacos_init(&mut g, idx);
    g.objs.aliens[idx as usize].rotx = 0;
    g.objs.aliens[idx as usize].vel = 0;

    // Higher than target (worldy < -800): skip fire.
    g.objs.aliens[idx as usize].worldy = -900;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    let strat0 = g.objs.aliens[idx as usize].stratptr;
    run_strat(&mut g, idx);
    assert_eq!(
        g.objs.aliens.iter().filter(|a| a.active).count(),
        before,
        "must not fire while higher than target"
    );
    assert_eq!(g.objs.aliens[idx as usize].stratptr, strat0);

    // At/below target (worldy >= -800) with rotx==0: fire + phase1.
    g.objs.aliens[idx as usize].worldy = -700;
    g.objs.aliens[idx as usize].rotx = 0;
    run_strat(&mut g, idx);
    assert!(
        g.objs.aliens.iter().filter(|a| a.active).count() > before,
        "must fire when not higher than target"
    );
    assert_ne!(g.objs.aliens[idx as usize].stratptr, strat0);
}

/// zaco3die: land → zaco3go when worldy >= -100.
#[test]
fn zaco3die_lands_when_worldy_not_higher_than_neg100() {
    let mut g = Game::new();
    spawn_player(&mut g);
    // strat_zaco3_init requires a nearby houdai_0 (shape 54) target.
    let houdai = spawn(&mut g);
    g.objs.aliens[houdai as usize].shape = 54;
    g.objs.aliens[houdai as usize].worldz = 100;
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 100;
    // Start high: zaco3die_init falls through into the die body on the death
    // frame (KSTRATS.ASM), so the first dive update (+4 rotx) happens here.
    g.objs.aliens[idx as usize].worldy = -200;
    strat_zaco3_init(&mut g, idx);
    let exp = g.objs.aliens[idx as usize].expstratptr.expect("exp");
    g.call_strat(exp, idx); // zaco3die_init + inline first die tick
    let die = g.objs.aliens[idx as usize].stratptr.expect("die");
    assert_eq!(g.objs.aliens[idx as usize].rotx, 4);

    // Still high: stay in die dive (rotx increases again).
    g.objs.aliens[idx as usize].worldy = -200;
    g.objs.aliens[idx as usize].rotx = 0;
    g.objs.aliens[idx as usize].vel = 0;
    g.call_strat(die, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratptr, Some(die));
    assert_eq!(g.objs.aliens[idx as usize].rotx, 4);

    // At/below -100: transition to go.
    g.objs.aliens[idx as usize].worldy = -50;
    g.call_strat(die, idx);
    assert_ne!(g.objs.aliens[idx as usize].stratptr, Some(die));
}

/// zaco1_cont: ceiling clamp worldy=0 when worldy >= 0.
#[test]
fn zaco1_cont_clamps_ceiling_when_not_higher_than_zero() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn(&mut g);
    strat_zaco1l_init(&mut g, idx);
    g.objs.aliens[idx as usize].sword2 = 0;
    g.objs.aliens[idx as usize].ptr = 0;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].vx = 0;
    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].vz = 0;

    g.objs.aliens[idx as usize].worldy = 15;
    run_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, 0);

    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].vx = 0;
    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].vz = 0;
    g.objs.aliens[idx as usize].sword2 = 0;
    g.objs.aliens[idx as usize].ptr = 0;
    g.objs.aliens[idx as usize].worldy = -12;
    run_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -12);
}
