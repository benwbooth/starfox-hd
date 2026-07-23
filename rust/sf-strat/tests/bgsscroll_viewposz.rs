//! Tick 126: bgsscrollZ ← viewposz after camera WRAM writeback.

use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::player::{strat_player, strat_spawn_player};

fn ready_player(g: &mut Game) -> u16 {
    let idx = strat_spawn_player(g).expect("player");
    g.vars.set_sv_u8(sv::STAYBLACK, (-1i8) as u8);
    g.vars.set_sv_u8(sv::DOINGWIPE, 0);
    g.vars.set_sv_u8(sv::PMOVELIMITAND, 0xFF);
    g.vars.game_mode = 0; // planet
    idx
}

#[test]
fn viewmove_copies_viewposz_to_bgsscrollz() {
    // ROM PSTRATS.ASM:1676 — after getview publishes viewposz, the next
    // viewmove copies it into bgsscrollZ (not pviewposz).
    let mut g = Game::new();
    let idx = ready_player(&mut g);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, 500);
    g.vars.set_sv_i16(sv::VIEWPOSZ, -120); // last-frame camera Z
    g.vars.set_sv_i16(sv::BGSSCROLLZ, 0);
    g.vars.pad1 = 0;

    strat_player(&mut g, idx);
    assert_eq!(
        g.vars.sv_i16(sv::BGSSCROLLZ),
        -120,
        "bgsscrollZ must follow viewposz, not pviewposz"
    );
    assert_ne!(
        g.vars.sv_i16(sv::BGSSCROLLZ),
        g.vars.sv_i16(sv::PVIEWPOSZ),
        "pviewposz advanced; bgsscrollZ stays on last camera Z"
    );
}
