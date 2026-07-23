//! ROM `shrapnel_srou` / `shrapfall2_Istrat` (PCSTRATS.ASM).

use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{install, player_sv as sv, shrapfall2_tick, shrapnel_srou};

#[test]
fn shrapfall2_drifts_toward_camera() {
    let mut g = Game::new();
    let _ = install(&mut g);
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].worldz = 1000;
    shrapfall2_tick(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 970);
}

#[test]
fn shrapnel_spawns_on_delay_frames() {
    let mut g = Game::new();
    let _ = install(&mut g);
    let p = g.objs.alloc().expect("parent");
    g.objs.aliens[p as usize].worldz = 500;
    g.vars.set_sv_i16(sv::VIEWCY, -60);

    // Frame 0: notdelay(3) and notdelay(1) both fire.
    g.vars.gameframe = 0;
    let before = (0..sf_game::alien::NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active)
        .count();
    shrapnel_srou(&mut g, p);
    let after = (0..sf_game::alien::NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active)
        .count();
    // shrap1 + large exp + med exp = +3
    assert_eq!(after, before + 3);

    // Frame 1: neither delay fires.
    g.vars.gameframe = 1;
    let mid = after;
    shrapnel_srou(&mut g, p);
    let after2 = (0..sf_game::alien::NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active)
        .count();
    assert_eq!(after2, mid);
}
