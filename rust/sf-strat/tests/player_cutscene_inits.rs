//! ROM cutscene phase-2 inits (PCSTRATS / PSTRATS).

use sf_game::vars::PSF3_ENGINESND;
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_chase2_init, player_clear_demo2_init, player_clear_demo2_strat, player_clear_turn2_init,
    player_dive2_init, player_move_init, player_start_init, player_sv as sv, player_under2_init,
    player_warp1_init, player_warp2_init,
};

#[test]
fn player_start_and_move_init() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.pshipflags = 0xff;
    player_start_init(&mut g);
    assert_eq!(g.vars.pshipflags, 0);
    assert_eq!(g.vars.sv_u16(sv::SPECWEPCNT), 3);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPE), 2);

    player_move_init(&mut g, p);
    assert_eq!(g.vars.viewdist, 120);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 120);
    assert_eq!(g.vars.internal_playpt, p as i16);
}

#[test]
fn phase2_dup_inits() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.objs.aliens[p as usize].shape = 2;
    g.vars.pshipflags3 |= PSF3_ENGINESND;

    let d = player_chase2_init(&mut g, p).expect("dup");
    assert_ne!(d, p);
    assert_eq!(g.vars.pshipflags3 & PSF3_ENGINESND, 0);

    let p2 = g.objs.alloc().expect("p2");
    let d2 = player_clear_turn2_init(&mut g, p2).expect("dup2");
    assert_eq!(g.objs.aliens[d2 as usize].sbyte1, 46);
    assert_eq!(g.vars.sv_i16(sv::OUTVY), 0);

    let p3 = g.objs.alloc().expect("p3");
    let _ = player_under2_init(&mut g, p3).expect("under");
    let p4 = g.objs.alloc().expect("p4");
    let d4 = player_warp1_init(&mut g, p4).expect("warp1");
    // `s_set_strat ... clshipboostnosnd_Istrat` falls through to its strategy
    // in the same frame, so the ROM's initial 19 is already decremented to 18.
    assert_eq!(g.objs.aliens[d4 as usize].sbyte2, 18);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 20);

    player_warp2_init(&mut g, p4);
    let p5 = g.objs.alloc().expect("p5");
    let _ = player_dive2_init(&mut g, p5).expect("dive2");
}

#[test]
fn clear_demo2_init_and_strat() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    player_clear_demo2_init(&mut g, p);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 0);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE2), 250);

    g.vars.set_sv_i16(sv::OUTVY, 0);
    player_clear_demo2_strat(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::OUTVY), -32);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE2), 249);
}
