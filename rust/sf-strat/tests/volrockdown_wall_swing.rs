//! Tick 200: volrockdown apex scatter RNG + wallleft/right swing targets +
//! public `wallleftright_istrat` entry (closes TIER2 "public port" blockers).

use sf_game::game::Game;
use sf_game::vars::HARD_HP;
use sf_strat::common::sf_random;
use sf_strat::enemies_ground::{
    volrockdown_istrat, volrockdown_strat, wallleft_strat, wallleftright_istrat, wallright_strat,
};

const WALL1_AP: u8 = 16; // STRATEQU.INC:210 wall1AP
const DEG180: u8 = 0x80;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

/// GA2STRAT.ASM:2102-2127 — at apex (state0, worldy>=0) three RANDOM draws:
/// vx=(rnd&15)-7, vy=(rnd&7)-15, vz=(rnd&15)-7; then state1 + worldy=0 + integrate.
/// Same tick also runs `.nsbounce` falldown (gravity+2) because state is already 1.
#[test]
fn volrockdown_apex_scatter_matches_rom_rng_formula() {
    let mut g = Game::new();
    let seed = [0x12u8, 0x34, 0x56, 0x78];
    g.vars.rng = seed;

    let d0 = sf_random(&mut g.vars) as u8;
    let d1 = sf_random(&mut g.vars) as u8;
    let d2 = sf_random(&mut g.vars) as u8;
    let expect_vx = ((d0 & 15) as i16) - 7;
    let expect_vy_scatter = ((d1 & 7) as i16) - 15;
    let expect_vz = ((d2 & 15) as i16) - 7;
    // Same-tick falldown: vy += 2, worldy was set to 0 then +scatter_vy, then
    // if worldy >= 0 clamp + bounce. With vy_scatter typically negative (upward
    // pop), after integrate worldy = expect_vy_scatter (<0) so falldown only
    // adds gravity (no bounce).
    let expect_vy_after = expect_vy_scatter.wrapping_add(2);

    let idx = spawn(&mut g);
    volrockdown_istrat(&mut g, idx);
    g.vars.rng = seed;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratstate = 0;
        al.worldy = 10;
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
    }
    volrockdown_strat(&mut g, idx);

    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.stratstate, 1, "s_next_state after scatter");
    assert_eq!(al.vx, expect_vx);
    assert_eq!(al.vz, expect_vz);
    assert_eq!(
        al.vy, expect_vy_after,
        "scatter vy then same-tick gravity +2"
    );
    // worldy: set 0, integrate +scatter_vy → expect_vy_scatter (<0 typically);
    // falldown sees airborne and leaves worldy alone.
    assert_eq!(al.worldy, expect_vy_scatter);
}

#[test]
fn wallleft_strat_swings_to_192_and_stops() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].roty = DEG180; // 128
    for _ in 0..8 {
        wallleft_strat(&mut g, idx);
    }
    assert_eq!(g.objs.aliens[idx as usize].roty, 192); // -64
    wallleft_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].roty, 192, "holds at target");
}

#[test]
fn wallright_strat_swings_to_64_and_stops() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].roty = DEG180;
    for _ in 0..8 {
        wallright_strat(&mut g, idx);
    }
    assert_eq!(g.objs.aliens[idx as usize].roty, 64);
    wallright_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].roty, 64, "holds at target");
}

#[test]
fn wallleftright_istrat_is_public_indestructible_oscillator() {
    let mut g = Game::new();
    g.vars.gameframe = 1; // skip notdelay(4) lean flip on fall-through
    let idx = spawn(&mut g);
    wallleftright_istrat(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.hp, HARD_HP);
    assert_eq!(al.ap, WALL1_AP);
    assert_eq!(al.animframe & 0x7F, 0, "s_init_anim #0");
    assert!(al.stratptr.is_some(), "tick = wall2_strat");
    // Fall-through may latch wallright_strat if xzdist sees a near "player"
    // (slot0); swing targets are covered by wallleft/right_strat tests above.
}
