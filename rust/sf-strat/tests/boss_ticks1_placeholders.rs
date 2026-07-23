//! Tick 143: AUDIT_BOSS_TICKS remaining placeholders —
//! boss2 `particlefiredown_Istrat` leap spawn + bossg `.scrollmsg` tx scroll.

use sf_game::alien::{AFEXP, ASF3_REALOBJ, ASF_COLLDISABLE, ASF_PARTOBJ};
use sf_game::Game;
use sf_strat::bosses::{boss2_strat, strat_boss2_init, strat_bossg_init};
use sf_strat::enemy_a::wm;

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
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

/// boss2 state-1 leap spawns `particlefiredown` (type/amount/life = 3/4/9).
#[test]
fn boss2_leap_spawns_particlefiredown() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.write_ext8(wm::CURRENTLEVEL, 2);
    let boss = spawn(&mut g);
    strat_boss2_init(&mut g, boss);
    // Strip children so state machine isn't blocked; force state 1.
    for i in 0..g.objs.aliens.len() {
        if i as u16 != boss && i != 0 {
            g.objs.aliens[i].active = false;
        }
    }
    g.objs.aliens[boss as usize].stratstate = 1;
    g.objs.aliens[boss as usize].worldx = 10;
    g.objs.aliens[boss as usize].worldy = -50;
    g.objs.aliens[boss as usize].worldz = 800;

    boss2_strat(&mut g, boss);

    // State 1 advances then falls through into state 2 same-tick (ROM nextstate).
    assert_eq!(g.objs.aliens[boss as usize].stratstate, 2);
    let ptr = g.objs.aliens[boss as usize].ptr;
    assert_ne!(ptr, 0, "al_ptr links particle");
    let particle = (ptr - 1) as usize;
    let al = &g.objs.aliens[particle];
    assert!(al.active);
    assert_eq!(al.sbyte3, 3, "particle type");
    assert_eq!(al.sbyte1, 4, "amount");
    assert_eq!(al.sbyte2, 9, "life");
    assert!(al.sflags & ASF_PARTOBJ != 0);
    assert!(al.sflags & ASF_COLLDISABLE != 0);
    assert!(al.flags & AFEXP != 0);
    assert!(al.expstratptr.is_some());
    assert_eq!(al.worldx, 10);
    assert_eq!(al.worldy, -50);
    assert_eq!(al.worldz, 800);
}

/// `.scrollmsg` (D2STRATS.ASM:307-318): tx+=4 is the texture scroll; wrap → next mode.
#[test]
fn bossg_scrollmsg_advances_tx_then_mode() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let boss = spawn(&mut g);
    strat_bossg_init(&mut g, boss);
    // BOSSG_MODE_SCROLLMSG = 1
    g.objs.aliens[boss as usize].stratmem = 1;
    g.objs.aliens[boss as usize].tx = 124;
    g.objs.aliens[boss as usize].worldz = 500; // |dz| to player 0 = 500 > 140 → no z bump

    // Drive one tick via public init's strat — call through registry.
    let strat = g.objs.aliens[boss as usize].stratptr.expect("bossg strat");
    g.call_strat(strat, boss);

    assert_eq!(g.objs.aliens[boss as usize].tx, 128);
    // 128 & 127 == 0 → next mode (same-tick continue into sf9e which also advances)
    assert!(
        g.objs.aliens[boss as usize].stratmem > 1,
        "tx wrap advances mode, got {}",
        g.objs.aliens[boss as usize].stratmem
    );
}

/// `.scrollmsg` mid-scroll: tx+=4, stay in mode, add_player_z path.
#[test]
fn bossg_scrollmsg_mid_keeps_mode() {
    let mut g = Game::new();
    spawn_player(&mut g, 100);
    let boss = spawn(&mut g);
    strat_bossg_init(&mut g, boss);
    g.objs.aliens[boss as usize].stratmem = 1;
    g.objs.aliens[boss as usize].tx = 40;
    g.objs.aliens[boss as usize].worldz = 500;

    let strat = g.objs.aliens[boss as usize].stratptr.expect("bossg strat");
    g.call_strat(strat, boss);

    assert_eq!(g.objs.aliens[boss as usize].tx, 44);
    assert_eq!(g.objs.aliens[boss as usize].stratmem, 1);
}
