//! Tick 94: ClearTurn / ClearUnder / ClearEarth cutscene leaves.

use sf_game::vars::{
    GF_VIEWROT, PFM_WOBBLE, PSF3_ENGINESND, PSF_NOCTRL, PSF_NOFIRE, PSTF_INSEQ, PSTF_NOVDISTC,
};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_clear_earth2_istrat, player_clear_earth2_strat, player_clear_earth_istrat,
    player_clear_earth_strat, player_clear_turn2_strat, player_clear_turn_strat,
    player_clear_under_strat, player_sv as sv, player_under2_strat, set_player_clear_earth,
    set_player_clear_turn, set_player_clear_under,
};

const MAX_PSPEED: i16 = 85;
const ASF2_SFLAG2: u8 = 0x20;
const BG2XSCROLL: u16 = 0x1F30;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn clear_turn_init_countdown_and_phase2() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.gameflags |= GF_VIEWROT;
    g.vars.set_sv_i16(sv::OUTDIST, 150);
    g.objs.aliens[idx as usize].worldx = 80;
    g.objs.aliens[idx as usize].worldy = 0;

    set_player_clear_turn(&mut g, idx);
    assert_eq!(g.vars.psvar_word1, 270);
    assert_ne!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);
    assert_ne!(g.vars.pstratflags & (PSTF_NOVDISTC | PSTF_INSEQ), 0);
    assert_eq!(g.vars.gameflags & GF_VIEWROT, 0);
    assert_ne!(g.vars.playerflymode & PFM_WOBBLE, 0);

    assert!(!player_clear_turn_strat(&mut g, idx));
    assert_eq!(g.vars.psvar_word1, 269);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 149);
    assert!(g.objs.aliens[idx as usize].worldx.abs() < 80);

    // Force phase-2: beqdec when word1==0
    g.vars.psvar_word1 = 0;
    assert!(player_clear_turn_strat(&mut g, idx));
    assert_eq!(g.vars.sv_i16(sv::OUTVY), 0);
    // dup created for clshipTurn
    assert!(g.objs.aliens.iter().any(|a| a.active && a.sbyte1 == 46));

    player_clear_turn2_strat(&mut g, idx);
}

#[test]
fn clear_under_init_and_handoff() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.gameflags |= GF_VIEWROT;
    g.vars.set_sv_i16(sv::OUTDIST, 400);
    g.vars.set_sv_i16(sv::OUTVY, 0);
    g.vars.pshipflags3 |= PSF3_ENGINESND;
    g.objs.aliens[idx as usize].worldy = 0;
    g.objs.aliens[idx as usize].worldx = 40;

    set_player_clear_under(&mut g, idx);
    assert_eq!(g.vars.psvar_word1, 194);
    assert_eq!(g.vars.psvar_word2, 0);
    assert_eq!(g.vars.sv_u8(sv::PLAYER_TOSPEED), MAX_PSPEED as u8);
    assert_eq!(g.vars.minpmove_y, -10000);
    assert_eq!(g.vars.gameflags & GF_VIEWROT, 0);

    assert!(!player_clear_under_strat(&mut g, idx));
    assert_eq!(g.vars.psvar_word1, 193);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 404);
    assert_eq!(g.vars.sv_i16(sv::OUTVY), 169);
    assert!(g.objs.aliens[idx as usize].worldy < 0);

    g.vars.psvar_word1 = 0;
    assert!(player_clear_under_strat(&mut g, idx));
    assert_eq!(g.vars.pshipflags3 & PSF3_ENGINESND, 0);
    // dup gets sflag2
    assert!(g
        .objs
        .aliens
        .iter()
        .any(|a| a.active && a.sflags2 & ASF2_SFLAG2 != 0));

    player_under2_strat(&mut g, idx);
}

#[test]
fn clear_earth_phase2_to_clearship() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, 1000);
    g.objs.aliens[idx as usize].worldx = 50;
    g.objs.aliens[idx as usize].worldy = 0;

    set_player_clear_earth(&mut g, idx);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 20);
    assert_ne!(g.vars.pstratflags & PSTF_INSEQ, 0);

    assert!(!player_clear_earth2_strat(&mut g, idx));
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 19);

    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 0);
    assert!(player_clear_earth2_strat(&mut g, idx));
    // Earth istrat → ClearShip Icont then same-frame ClearShip_strat (sbyte3 100→99).
    // ClearShip_flymode=0 wipes PFM_WOBBLE; Earth_strat re-ORs it each frame.
    // nturn speedto may nudge vel off maxpspeed-5 in the same frame.
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 99);
    assert_eq!(g.vars.read_ext8(BG2XSCROLL), 0);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 40);

    // Earth strat: chase while byte1>0, then ClearShip body; re-OR wobble
    g.objs.aliens[idx as usize].worldx = 30;
    player_clear_earth_istrat(&mut g, idx);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 5);
    let wx_before = g.objs.aliens[idx as usize].worldx;
    player_clear_earth_strat(&mut g, idx);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 4);
    assert_ne!(g.vars.playerflymode & PFM_WOBBLE, 0);
    assert!(g.objs.aliens[idx as usize].worldx.abs() <= wx_before.abs());
}

#[test]
fn clear_earth_istrat_alone_wires_clearship() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, 500);
    player_clear_earth2_istrat(&mut g, idx);
    player_clear_earth_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 99);
    assert_ne!(g.vars.pstratflags & PSTF_NOVDISTC, 0);
}
