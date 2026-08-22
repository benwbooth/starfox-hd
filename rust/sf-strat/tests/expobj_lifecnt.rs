//! Tick 140: makeexpobj / delayexplode lifecnt = ROM s_decbpl_lifecnt.
//!
//! makeMED/L/SML leave al_count=0 (EXPSTRAT.ASM:327-335 never sets lifecnt).
//! boss8die per-tick exps therefore explode on the first delayexplode tick.
//! When lifecnt IS set (boss8_bigexplode 1..5), survive count+1 ticks — the
//! old `if count>0{--}; if==0{die}` fired one frame early.

use sf_game::alien::ASF_HITFLASH;
use sf_game::Game;
use sf_strat::bosses::{b8_make_exp_obj, boss8_bigexplode, boss8_delayexplode_strat};
use sf_strat::enemy_a::{
    delayexplode_strat, make_large_exp_obj, make_medium_exp_obj, make_small_exp_obj,
};

const B8_EXPSHAPE_MEDIUM: u16 = 2;
const B8_EXPSHAPE_LARGE: u16 = 3;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

/// makeexpobj default: count stays 0 → explode on first delayexplode tick.
#[test]
fn makeexpobj_default_count_zero_explodes_first_tick() {
    let mut g = Game::new();
    let parent = spawn(&mut g);
    let med = make_medium_exp_obj(&mut g, parent).expect("med");
    let sml = make_small_exp_obj(&mut g, parent).expect("sml");
    let lrg = make_large_exp_obj(&mut g, parent).expect("lrg");
    assert_eq!(g.objs.aliens[med as usize].count, 0);
    assert_eq!(g.objs.aliens[sml as usize].count, 0);
    assert_eq!(g.objs.aliens[lrg as usize].count, 0);

    g.objs.aldead = 0;
    delayexplode_strat(&mut g, med);
    // Expiry applies s_kill_obj (STRATMAC.INC:2643) then runs the explosion
    // inline. Without nopolyexp the corpse MORPHS into its polygon mesh and
    // survives as a live object — removal would cut that mesh off one tick
    // early (Corneria replay tick 1733).
    assert_eq!(g.objs.aldead, 0, "expiry is a death signal, not removal");
    assert_eq!(g.objs.aliens[med as usize].hp, 0, "kill_obj zeroed HP");
    assert_eq!(g.objs.aliens[med as usize].shape, 466, "polygon mesh");
}

/// boss8 makeexpobj same default (boss8die per-tick barrage).
#[test]
fn b8_make_exp_obj_default_count_zero_explodes_first_tick() {
    let mut g = Game::new();
    let parent = spawn(&mut g);
    let e = b8_make_exp_obj(&mut g, parent, B8_EXPSHAPE_MEDIUM).expect("exp");
    assert_eq!(g.objs.aliens[e as usize].count, 0);
    g.objs.aldead = 0;
    boss8_delayexplode_strat(&mut g, e);
    assert_eq!(g.objs.aldead, 1);
    assert_ne!(g.objs.aliens[e as usize].sflags & ASF_HITFLASH, 0);
}

/// lifecnt N survives N ticks then dies on tick N+1 (entry count 0).
#[test]
fn delayexplode_lifecnt_survives_count_plus_one() {
    let mut g = Game::new();
    let parent = spawn(&mut g);
    let e = b8_make_exp_obj(&mut g, parent, B8_EXPSHAPE_LARGE).expect("exp");
    g.objs.aliens[e as usize].count = 3; // like bigexplode i+1 for i=2

    for tick in 1..=3 {
        g.objs.aldead = 0;
        boss8_delayexplode_strat(&mut g, e);
        assert_eq!(g.objs.aldead, 0, "tick {tick}: still alive (count was >0)");
        assert_eq!(g.objs.aliens[e as usize].count, 3 - tick);
    }
    // tick 4: entry count 0 → die
    g.objs.aldead = 0;
    boss8_delayexplode_strat(&mut g, e);
    assert_eq!(g.objs.aldead, 1, "tick 4: entry 0 → explode");
}

/// lifecnt=1 must NOT explode on the first tick (old bug).
#[test]
fn delayexplode_lifecnt_one_survives_first_tick() {
    let mut g = Game::new();
    let parent = spawn(&mut g);
    let e = b8_make_exp_obj(&mut g, parent, B8_EXPSHAPE_LARGE).expect("exp");
    g.objs.aliens[e as usize].count = 1; // bigexplode first child
    g.objs.aldead = 0;
    boss8_delayexplode_strat(&mut g, e);
    assert_eq!(g.objs.aldead, 0, "count 1→0: survive");
    assert_eq!(g.objs.aliens[e as usize].count, 0);
    boss8_delayexplode_strat(&mut g, e);
    assert_eq!(g.objs.aldead, 1, "count 0: die next tick");
}

/// `bigexplode_Istrat` ends by jumping through delayexplode_Istrat, whose
/// initializer installs ordinary explode_Istrat as the terminal callback.
/// Pointing expstrat back at delayexplode recurses forever when count expires.
#[test]
fn boss8_bigexplode_expires_through_the_ordinary_explosion() {
    let mut g = Game::new();
    let boss = spawn(&mut g);
    boss8_bigexplode(&mut g, boss);

    let al = g.objs.aliens[boss as usize];
    assert_ne!(al.stratptr, al.expstratptr);
    assert_eq!(al.count, 4);

    for tick in 1..=4 {
        g.objs.aldead = 0;
        boss8_delayexplode_strat(&mut g, boss);
        assert_eq!(g.objs.aldead, 0, "tick {tick}: boss still delaying");
    }
    g.objs.aldead = 0;
    boss8_delayexplode_strat(&mut g, boss);
    assert_eq!(g.objs.aldead, 1, "tick 5: ordinary explosion removes boss");
}
