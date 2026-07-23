//! Tick 164: AUDIT_ENEMY_A Minor #15 leftovers — zaco3die/go makesmoke +
//! szaco2 debrisshape. Tick 167: debrisshape is true `zaco_8p` (ext 283).

use sf_game::alien::ASF3_REALOBJ;
use sf_game::game::Game;
use sf_strat::enemy_a::{strat_szaco2_init, strat_zaco3_init, ASF2_RELEXPLODE, SH_ZACO_8P};

const SH_SMOKE: u16 = 358;
const SH_HOUDAI_0: u16 = 54; // zaco3 needs a houdai_0 target

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

fn count_smoke(g: &Game) -> usize {
    g.objs
        .aliens
        .iter()
        .filter(|a| a.active && a.shape == SH_SMOKE)
        .count()
}

fn arm_zaco3die(g: &mut Game) -> u16 {
    spawn_player(g, 0, -40, 0);
    let houdai = spawn(g);
    g.objs.aliens[houdai as usize].shape = SH_HOUDAI_0;
    g.objs.aliens[houdai as usize].worldz = 200;
    let idx = spawn(g);
    g.objs.aliens[idx as usize].worldz = 200;
    strat_zaco3_init(g, idx);
    let exp = g.objs.aliens[idx as usize].expstratptr.expect("exp");
    g.call_strat(exp, idx); // zaco3die_init
    idx
}

/// Minor #15: zaco3die emits smoke on even gameframe (notdelay 1).
#[test]
fn zaco3die_spawns_smoke_on_even_frame() {
    let mut g = Game::new();
    let idx = arm_zaco3die(&mut g);
    let die = g.objs.aliens[idx as usize].stratptr.expect("die");
    g.objs.aliens[idx as usize].worldy = -200; // stay in dive
    g.vars.gameframe = 0; // &1 == 0 → smoke
    let before = count_smoke(&g);
    g.call_strat(die, idx);
    assert_eq!(count_smoke(&g), before + 1, "even frame must spawn smoke");

    g.vars.gameframe = 1; // &1 != 0 → skip
    let mid = count_smoke(&g);
    g.call_strat(die, idx);
    assert_eq!(count_smoke(&g), mid, "odd frame must not spawn smoke");
}

/// Minor #15: zaco3go smoke every 4th frame with vz=40.
#[test]
fn zaco3go_smoke_sets_vz_40() {
    let mut g = Game::new();
    let idx = arm_zaco3die(&mut g);
    let die = g.objs.aliens[idx as usize].stratptr.expect("die");
    // Land into go.
    g.objs.aliens[idx as usize].worldy = -50;
    g.vars.gameframe = 1; // die tick: no smoke; go may still run
    g.call_strat(die, idx);

    let go = g.objs.aliens[idx as usize].stratptr.expect("go");
    g.vars.gameframe = 0; // &3 == 0 → smoke
    let before = count_smoke(&g);
    g.call_strat(go, idx);
    assert_eq!(count_smoke(&g), before + 1);
    let smoke = g
        .objs
        .aliens
        .iter()
        .enumerate()
        .rev()
        .find(|(_, a)| a.active && a.shape == SH_SMOKE)
        .map(|(i, _)| i)
        .expect("smoke");
    assert_eq!(g.objs.aliens[smoke].vz, 40, "ASM sets smoke al_vz,#40");

    g.vars.gameframe = 1; // &3 != 0 → skip
    let mid = count_smoke(&g);
    g.call_strat(go, idx);
    assert_eq!(count_smoke(&g), mid);
}

/// Minor #15: szaco2 sets relexplode + debrisshape = true `zaco_8p`.
#[test]
fn szaco2_init_sets_debrisshape_and_relexplode() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    strat_szaco2_init(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE, 0);
    assert_eq!(
        g.objs.aliens[idx as usize].debrisshape, SH_ZACO_8P,
        "szaco2 debris must be zaco_8p (SHAPE_EXT 283), not zaco_8 stand-in"
    );
}
