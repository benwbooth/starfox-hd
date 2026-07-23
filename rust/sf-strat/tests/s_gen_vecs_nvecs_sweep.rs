//! Tick 172: `s_gen_vecs` sites use `nvecs_l` (not `alvelvecs` / gen_vecs_2d).

use sf_game::alien::ASF3_REALOBJ;
use sf_game::game::Game;
use sf_strat::common::strat_nvecs;
use sf_strat::enemies_ground::walking2_strat;
use sf_strat::enemy_a::{clship_underboost_istrat, clship_underboost_strat, strat_bomwing_init};

fn spawn_player(g: &mut Game) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].sflags3 |= ASF3_REALOBJ;
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

/// bomwing phase1: `s_gen_vecs` → nvecs (-roty+1), preserves vy.
#[test]
fn bomwing_phase1_uses_nvecs() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn(&mut g);
    strat_bomwing_init(&mut g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = 40;
        al.vel = 20;
        al.vy = -7;
        al.sbyte1 = 10; // stay in phase1
        al.worldx = 0;
        al.worldz = 0;
    }
    let (ex, ez) = strat_nvecs(40, 20);
    // alvelvecs (no nega) would differ for this angle.
    assert_ne!(ex, 0);

    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vx, ex);
    assert_eq!(g.objs.aliens[idx as usize].vz, ez);
    assert_eq!(g.objs.aliens[idx as usize].vy, -7, "nvecs must not zero vy");
}

/// walking2: continuous `s_gen_vecs` → nvecs.
#[test]
fn walking2_uses_nvecs() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = 64;
        al.vel = 30;
        al.vy = 5;
        al.worldx = 0;
        al.worldz = 0;
    }
    let (ex, ez) = strat_nvecs(64, 30);
    walking2_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vx, ex);
    assert_eq!(g.objs.aliens[idx as usize].vz, ez);
    // apply_velocity ran; vy untouched by nvecs (still 5 before any other write).
    // walking2 only gens+applies xz and may tweak roty/vel — vy stays.
    assert_eq!(g.objs.aliens[idx as usize].vy, 5);
}

/// clship underboost: `s_gen_vecs` → nvecs after speed_to.
#[test]
fn clship_underboost_uses_nvecs() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn(&mut g);
    clship_underboost_istrat(&mut g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = 80;
        al.vel = 40; // already at target speed
        al.vy = -3;
        al.sbyte1 = 0; // no bank
    }
    let (ex, ez) = strat_nvecs(80, 40);
    clship_underboost_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vx, ex);
    assert_eq!(g.objs.aliens[idx as usize].vz, ez);
    assert_eq!(g.objs.aliens[idx as usize].vy, -3);
}
