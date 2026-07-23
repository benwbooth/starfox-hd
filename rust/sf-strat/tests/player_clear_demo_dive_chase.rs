//! Tick 95: ClearDemo / DIVE / ClearChase cutscene leaves.

use sf_game::alien::ASF_INVISIBLE;
use sf_game::vars::{
    GF_NOZREMOVE, GF_VIEWROT, PFM_WOBBLE, PSF3_ENGINESND, PSF_NOCTRL, PSF_NOFIRE, PSTF_INSEQ,
    PSTF_NOVDISTC,
};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_chase2_strat, player_clear_chase_istrat, player_clear_chase_strat,
    player_clear_demo2_strat, player_clear_demo_strat, player_dive2_strat, player_dive_istrat,
    player_dive_strat, player_sv as sv, set_player_clear_chase, set_player_clear_demo,
    set_player_dive,
};

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn clear_demo_init_countdown_and_demo2() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.gameflags |= GF_VIEWROT;
    g.vars.set_sv_i16(sv::OUTDIST, 200);
    g.objs.aliens[idx as usize].worldx = 40;
    g.objs.aliens[idx as usize].worldy = 0;

    set_player_clear_demo(&mut g, idx);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE2), 110);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE3), 180);
    assert_ne!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);
    assert_ne!(g.vars.pstratflags & (PSTF_NOVDISTC | PSTF_INSEQ), 0);
    assert_eq!(g.vars.gameflags & GF_VIEWROT, 0);

    assert!(!player_clear_demo_strat(&mut g, idx));
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE2), 109);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE3), 179);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 204);
    // playermove may rewrite tospeed; chase toward planet ViewCY is the leaf signal
    assert!(g.objs.aliens[idx as usize].worldy <= 0);

    // Force demo2
    g.vars.set_sv_u8(sv::PSVAR_BYTE2, 0);
    g.vars.set_sv_u8(sv::PSVAR_BYTE3, 180);
    assert!(player_clear_demo_strat(&mut g, idx));
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE2), 249); // init 250, then demo2 dec
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 249);

    // Demo2: plrotx -= 512, bg2Yscroll -= 1, outdist bump
    g.vars.set_sv_i16(sv::PLROTX, 0);
    g.vars.set_sv_i16(sv::BG2YSCROLL, 100);
    g.vars.set_sv_i16(sv::OUTDIST, 500);
    g.vars.set_sv_u8(sv::PSVAR_BYTE3, 5); // not yet dup
    player_clear_demo2_strat(&mut g, idx);
    assert_eq!(g.vars.sv_i16(sv::BG2YSCROLL), 99);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 504);
    // playermove achases plrotx toward 0 after -512 write
    assert!(g.vars.sv_i16(sv::PLROTX) < 0);

    // Dup when byte3 dec to 1
    g.vars.set_sv_u8(sv::PSVAR_BYTE3, 2);
    g.vars.pshipflags3 |= PSF3_ENGINESND;
    player_clear_demo2_strat(&mut g, idx);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE3), 1);
    assert_eq!(g.vars.pshipflags3 & PSF3_ENGINESND, 0);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_INVISIBLE, 0);
}

#[test]
fn dive_init_scroll_window_and_phase2() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.set_sv_i16(sv::BG2YSCROLL, 50);
    g.objs.aliens[idx as usize].worldx = 20;

    set_player_dive(&mut g, idx);
    assert_eq!(g.vars.psvar_word1, 286);
    assert_eq!(g.vars.psvar_word2, 0);
    assert_ne!(g.vars.pstratflags & PSTF_NOVDISTC, 0);
    assert_ne!(g.vars.playerflymode & PFM_WOBBLE, 0);

    assert!(!player_dive_strat(&mut g, idx));
    assert_eq!(g.vars.psvar_word1, 285);
    // outside 30..=60 → no scroll bump
    assert_eq!(g.vars.sv_i16(sv::BG2YSCROLL), 50);

    g.vars.psvar_word1 = 45;
    player_dive_istrat(&mut g, idx); // resets word1 — set again
    g.vars.psvar_word1 = 45;
    player_dive_strat(&mut g, idx);
    assert_eq!(g.vars.psvar_word1, 44);
    assert_eq!(g.vars.sv_i16(sv::BG2YSCROLL), 51);

    g.vars.psvar_word1 = 0;
    g.vars.pshipflags3 |= PSF3_ENGINESND;
    assert!(player_dive_strat(&mut g, idx));
    assert_eq!(g.vars.pshipflags3 & PSF3_ENGINESND, 0);
    // dup exists
    assert!(g.objs.aliens.iter().filter(|a| a.active).count() >= 2);

    player_dive2_strat(&mut g, idx);
}

#[test]
fn clear_chase_windows_and_chase2() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.gameflags |= GF_VIEWROT;
    g.vars.set_sv_i16(sv::VIEWCY, -60);
    g.vars.set_sv_i16(sv::OUTDIST, 400);
    g.vars.set_sv_i16(sv::OUTVX, 0);
    g.vars.set_sv_i16(sv::OUTVY, 0);
    g.vars.set_sv_i16(sv::BG2YSCROLL, 10);

    set_player_clear_chase(&mut g, idx);
    assert_eq!(g.vars.psvar_word1, 300);
    assert_eq!(g.vars.psvar_word3, 218);
    assert_eq!(g.vars.psvar_word4, 5);
    assert_ne!(g.vars.gameflags & GF_NOZREMOVE, 0);
    assert_eq!(g.vars.gameflags & GF_VIEWROT, 0);

    // word1=300: no outvx window, outvy += 218, no scroll ( >56 )
    assert!(!player_clear_chase_strat(&mut g, idx));
    assert_eq!(g.vars.psvar_word1, 299);
    assert_eq!(g.vars.sv_i16(sv::OUTVX), 0);
    assert_eq!(g.vars.sv_i16(sv::OUTVY), 218);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 404);
    assert_eq!(g.vars.sv_i16(sv::BG2YSCROLL), 10);

    // outvx subtract window
    g.vars.psvar_word1 = 200;
    g.vars.set_sv_i16(sv::OUTVX, 0);
    player_clear_chase_istrat(&mut g, idx);
    g.vars.psvar_word1 = 200;
    g.vars.set_sv_i16(sv::OUTVX, 0);
    g.vars.set_sv_i16(sv::OUTVY, 0);
    player_clear_chase_strat(&mut g, idx);
    assert_eq!(g.vars.sv_i16(sv::OUTVX), -64);

    // outvx add window
    g.vars.psvar_word1 = 80;
    g.vars.set_sv_i16(sv::OUTVX, 0);
    player_clear_chase_strat(&mut g, idx);
    assert_eq!(g.vars.sv_i16(sv::OUTVX), 64);

    // scroll window + word3/4 taper
    g.vars.psvar_word1 = 8;
    g.vars.psvar_word3 = 218;
    g.vars.psvar_word4 = 5;
    g.vars.set_sv_i16(sv::BG2YSCROLL, 10);
    g.vars.set_sv_i16(sv::OUTVY, 0);
    player_clear_chase_strat(&mut g, idx);
    assert_eq!(g.vars.psvar_word3, 218 - 21);
    assert_eq!(g.vars.psvar_word4, 5); // only <=4 decs word4
    assert_eq!(g.vars.sv_i16(sv::BG2YSCROLL), 10 + 5);

    g.vars.psvar_word1 = 0;
    g.vars.pshipflags3 |= PSF3_ENGINESND;
    assert!(player_clear_chase_strat(&mut g, idx));
    assert_eq!(g.vars.pshipflags3 & PSF3_ENGINESND, 0);

    player_chase2_strat(&mut g, idx);
}
