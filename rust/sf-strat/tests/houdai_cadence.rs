//! Tick 147: AUDIT_ENEMY_A High #3/#4 — `s_jmp_notdelay N,...,al1pt`
//! cadence = `(gameframe+idx) & ((1<<N)-1) == 0`.

use sf_game::alien::{ASF3_REALOBJ, NUMBER_AL};
use sf_game::Game;
use sf_strat::enemy_a::{houdai_strat, strat_houdai_init, zaco0c_istrat};

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
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn active_count(g: &Game) -> usize {
    (0..NUMBER_AL).filter(|&i| g.objs.aliens[i].active).count()
}

/// High #3: houdai fires only when `(gf+idx)&0x0F==0` and player XZ ≥ 800.
#[test]
fn houdai_fires_every_16_frames_staggered() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    // Place far enough that dist_xz >= 800 (player at 0,0).
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldz = 1000;
    strat_houdai_init(&mut g, idx); // may fire if gf+idx already open

    // Closed gate: (0+idx)&15 != 0 for idx=1 → no fire on this tick.
    g.vars.gameframe = 0;
    let before = active_count(&g);
    houdai_strat(&mut g, idx);
    // idx=1: (0+1)&15=1 ≠ 0 → no fire
    assert_eq!(active_count(&g), before, "gate closed at gf=0 idx=1");

    // Open gate: need (gf+1)&15==0 → gf=15
    g.vars.gameframe = 15;
    let before = active_count(&g);
    houdai_strat(&mut g, idx);
    assert!(active_count(&g) > before, "SHORTPLASMA when (15+1)&15==0");

    // Old bug was mask 3 (every 4): gf=3 would fire with &3. Must NOT fire.
    g.vars.gameframe = 3;
    let before = active_count(&g);
    houdai_strat(&mut g, idx);
    assert_eq!(
        active_count(&g),
        before,
        "must not use old &3 cadence (gf=3)"
    );
}

/// High #3: close player suppresses fire even on open gate.
#[test]
fn houdai_holds_fire_when_player_xz_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 400; // dist_xz=400 < 800
    strat_houdai_init(&mut g, idx);
    g.vars.gameframe = 15; // open for idx=1
    let before = active_count(&g);
    houdai_strat(&mut g, idx);
    assert_eq!(active_count(&g), before);
}

/// High #4: zaco0c fire gate `(gf+idx)&3==0` (notdelay 2,al1pt).
#[test]
fn zaco0c_fire_gate_mask3_with_al1pt() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 800;
    g.objs.aliens[idx as usize].sbyte1 = 20; // stay in fire phase
                                             // zaco0c_istrat falls into zaco0_fire same tick.
    g.vars.gameframe = 0; // (0+1)&3=1 → no fire
    zaco0c_istrat(&mut g, idx);
    // Init may have run fire once; re-tick with closed gate.
    let strat = g.objs.aliens[idx as usize].stratptr.expect("fire strat");
    let before = active_count(&g);
    g.vars.gameframe = 1; // (1+1)&3=2 → closed
    g.call_strat(strat, idx);
    assert_eq!(active_count(&g), before, "closed at gf=1");

    g.vars.gameframe = 3; // (3+1)&3=0 → open
    let before = active_count(&g);
    g.call_strat(strat, idx);
    assert!(
        active_count(&g) > before,
        "laser when (3+1)&3==0; before={before}"
    );
}
