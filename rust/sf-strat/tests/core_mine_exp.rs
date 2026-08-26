//! ROM core1exp / mcore1exp / monolithexp / mine2exp / blowcube.

use sf_game::alien::ASF4_INVISIBLE;
use sf_game::vars::{GF_BOSSDEAD, GF_STRATDONE1, HARD_HP};
use sf_game::Game;
use sf_strat::enemy_a::{
    blowcube_istrat, blowcube_strat, core1col_istrat, core1exp_istrat, gasflags, mcore1col_istrat,
    mcore1exp_istrat, mcore1exp_strat, mine2exp_istrat, monolithexp_istrat, set_gasflags,
    COLLTYPE_ENEMYWEAP,
};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
}

#[test]
fn core1exp_sets_flags_and_follows_player() {
    let mut g = Game::new();
    spawn_player(&mut g, 100);
    let idx = g.objs.alloc().expect("core");
    g.objs.aliens[idx as usize].worldz = 500;
    g.objs.aliens[idx as usize].worldy = 0;
    let before = g.objs.active_indices().len();
    core1exp_istrat(&mut g, idx);
    assert_ne!(g.vars.gameflags & GF_STRATDONE1, 0);
    assert_ne!(g.objs.aliens[idx as usize].sflags4 & ASF4_INVISIBLE, 0);
    assert_eq!(g.objs.aliens[idx as usize].count, 19); // fall-through dec
    assert!(g.objs.active_indices().len() > before, "Lexp spawned");
    // Stick to player.
    assert_eq!(g.objs.aliens[idx as usize].worldz, 100);
}

#[test]
fn core1col_sets_gasf_flag1() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("core");
    g.objs.aliens[idx as usize].hp = 10;
    set_gasflags(&mut g, 0);
    core1col_istrat(&mut g, idx);
    assert_ne!(gasflags(&g) & 0x08, 0);
}

#[test]
fn monolithexp_removes() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("m");
    monolithexp_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn blowcube_aims_and_tumbles() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = g.objs.alloc().expect("cube");
    g.objs.aliens[idx as usize].worldz = 800;
    blowcube_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, HARD_HP);
    assert_eq!(g.objs.aliens[idx as usize].ap, 4);
    assert_ne!(
        g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMYWEAP,
        0
    );
    assert_eq!(g.objs.aliens[idx as usize].vel, 80);
    let ry0 = g.objs.aliens[idx as usize].roty;
    let rx0 = g.objs.aliens[idx as usize].rotx;
    blowcube_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].roty, ry0.wrapping_add(4));
    assert_eq!(g.objs.aliens[idx as usize].rotx, rx0.wrapping_add(8));
}

#[test]
fn mcore1exp_tumbles_then_bursts_past_3000() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = g.objs.alloc().expect("mcore");
    g.objs.aliens[idx as usize].worldz = 1000;
    mcore1exp_istrat(&mut g, idx);
    // First tick: z += 60, still < 3000 → no burst.
    assert_eq!(g.objs.aliens[idx as usize].worldz, 1060);
    assert_eq!(g.vars.gameflags & GF_BOSSDEAD, 0);

    g.objs.aliens[idx as usize].worldz = 3100;
    let before = g.objs.active_indices().len();
    mcore1exp_strat(&mut g, idx);
    assert_ne!(g.vars.gameflags & GF_BOSSDEAD, 0);
    assert!(
        g.objs.active_indices().len() > before + 4,
        "FOL exp + 6 blowcubes"
    );
}

#[test]
fn mcore1col_deflects_unless_state5() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("m");
    let attacker = g.objs.alloc().expect("attacker");
    g.objs.aliens[idx as usize].stratstate = 5;
    g.objs.aliens[idx as usize].hp = 10;
    g.objs.aliens[idx as usize].collobjptr = attacker;
    g.objs.aliens[idx as usize].collcount = 1;
    g.objs.aliens[attacker as usize].ap = 1;
    mcore1col_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 9); // hitflash damage

    g.objs.aliens[idx as usize].stratstate = 0;
    g.objs.aliens[idx as usize].hp = 10;
    g.objs.aliens[idx as usize].collobjptr = 0;
    g.objs.aliens[attacker as usize].active = false;
    // No laser partner → DefElaserCol just clears collide / resumes.
    mcore1col_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 10);
}

#[test]
fn mine2exp_fires_five_beams_then_explodes() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = g.objs.alloc().expect("mine");
    g.objs.aliens[idx as usize].worldz = 500;
    let before = g.objs.active_indices().len();
    mine2exp_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    // 5 beams + particle companion.
    assert!(g.objs.active_indices().len() + 1 > before + 4);
}
