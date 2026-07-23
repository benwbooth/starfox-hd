//! ISTRATS wiring — shark@59, fzaco@112, hard90yrfog@182;
//! fix tank1a/tank0/tank1/tank3/houdai5f off-by-one (hard90yrfog was skipped).

use sf_game::alien::ASF3_REALOBJ;
use sf_game::game::Game;
use sf_strat::enemy_a::{hard90yrfog_istrat, hardenemy1_istrat, COLLTYPE_ENEMY1};
use sf_strat::table;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].sflags3 |= ASF3_REALOBJ;
    idx
}

/// Shark / fzaco / hard90yrfog occupy free ISTRATS rows.
#[test]
fn shark_fzaco_hard90yrfog_registered_at_rom_indices() {
    let mut g = Game::new();
    table::register_all(&mut g);
    assert!(g.world.istrats[59].is_some(), "IS_SHARK=59");
    assert!(g.world.istrats[112].is_some(), "IS_FZACO=112");
    assert!(g.world.istrats[182].is_some(), "IS_HARD90YRFOG=182");
}

/// hard90yrfog@182 must not be tank1a; tanks begin at 183.
#[test]
fn hard90yrfog_slot_is_not_tank1a() {
    let mut g = Game::new();
    table::register_all(&mut g);
    let fog = g.world.istrats[182].expect("hard90yrfog");
    let tank1a = g.world.istrats[183].expect("tank1a");
    assert_ne!(
        fog, tank1a,
        "182 must be hard90yrfog, distinct from tank1a@183"
    );
    assert!(g.world.istrats[186].is_some(), "tank3@186");
    assert!(g.world.istrats[187].is_some(), "houdai5f@187");
}

/// Dispatching IS 182 runs hard90yrfog (DEG180 + fog strat), not a tank.
#[test]
fn istrat_182_dispatches_hard90yrfog() {
    let mut g = Game::new();
    table::register_all(&mut g);
    let idx = spawn(&mut g);
    let s = g.world.istrats[182].expect("hard90yrfog");
    g.call_strat(s, idx);
    assert_eq!(g.objs.aliens[idx as usize].roty, 128, "DEG180");
    assert!(
        g.objs.aliens[idx as usize].stratptr.is_some(),
        "fog tick installed"
    );
}

/// Direct hard90yrfog vs hardenemy1 still diverge on colltype (sanity).
#[test]
fn hard90yrfog_has_no_enemy1_unlike_hardenemy1() {
    let mut g = Game::new();
    let a = spawn(&mut g);
    hard90yrfog_istrat(&mut g, a);
    assert_eq!(g.objs.aliens[a as usize].collflags & COLLTYPE_ENEMY1, 0);

    let b = spawn(&mut g);
    hardenemy1_istrat(&mut g, b);
    assert_ne!(g.objs.aliens[b as usize].collflags & COLLTYPE_ENEMY1, 0);
}

/// Shark istrat arms via table row 59.
#[test]
fn istrat_59_dispatches_shark() {
    let mut g = Game::new();
    table::register_all(&mut g);
    let idx = spawn(&mut g);
    let s = g.world.istrats[59].expect("shark");
    g.call_strat(s, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    assert_eq!(g.objs.aliens[idx as usize].hp, 4);
}
