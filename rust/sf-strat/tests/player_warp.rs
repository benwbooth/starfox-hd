//! Tick 96: player Warp / WarpOut cutscene leaves.

use sf_core::player_view::{PlayerViewMode, PlayerViewOptions};
use sf_game::alien::ASF_INVISIBLE;
use sf_game::vars::{
    GF_NOZREMOVE, GF_VIEWROT, PFM_WOBBLE, PSF3_ENGINESND, PSF3_NOCOLLISIONS, PSF_NOCTRL,
    PSF_NOFIRE, PSTF_INSEQ, PSTF_NOVDISTC,
};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_sv as sv, player_warp1_strat, player_warp2_strat, player_warp_istrat,
    player_warp_out_strat, player_warp_strat, set_player_warp, set_player_warp_out,
};

const PSTF_FLAG1: u8 = 2;
const OUTVIEWDIST: i16 = 120;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn warp_init_and_state0_to_1() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.gameflags |= GF_VIEWROT;
    g.vars.set_sv_i16(sv::OUTDIST, 150);
    g.objs.aliens[idx as usize].worldx = 40;
    g.objs.aliens[idx as usize].worldy = 0;

    set_player_warp(&mut g, idx);
    assert_eq!(g.vars.psvar_word1, 200);
    assert_eq!(g.vars.psvar_word2, 0);
    assert_ne!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);
    assert_ne!(g.vars.pstratflags & (PSTF_NOVDISTC | PSTF_INSEQ), 0);
    assert_ne!(g.vars.gameflags & GF_NOZREMOVE, 0);
    assert_eq!(g.vars.gameflags & GF_VIEWROT, 0);
    assert_ne!(g.vars.playerflymode & PFM_WOBBLE, 0);
    assert_ne!(g.vars.pshipflags3 & PSF3_NOCOLLISIONS, 0);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 0);

    // One tick: decbne 200→199, stay state 0
    g.vars.gameframe = 4; // notdelay 2 true
    assert!(!player_warp_strat(&mut g, idx));
    assert_eq!(g.vars.psvar_word1, 199);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 0);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 149);

    // Force advance to state 1
    g.vars.psvar_word1 = 1;
    assert!(!player_warp_strat(&mut g, idx));
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    assert_eq!(g.vars.psvar_word1, 225); // 226 then state1 decbne → 225 same frame
    assert_eq!(g.vars.playerflymode & PFM_WOBBLE, 0);
}

#[test]
fn warp_state1_hyperspace_and_state2_to_warp1() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    player_warp_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].stratstate = 1;
    g.vars.psvar_word1 = 1;
    g.vars.set_sv_i16(sv::OUTDIST, 100);
    g.vars.set_sv_i16(sv::OUTVY, 0);

    assert!(!player_warp_strat(&mut g, idx));
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 2);
    // hyperspace obj spawned + state2 ran: word1 60→59, noZremove cleared, outdist-9
    assert!(g.objs.aliens.iter().filter(|a| a.active).count() >= 2);
    assert_eq!(g.vars.gameflags & GF_NOZREMOVE, 0);
    assert_eq!(g.vars.psvar_word1, 59);

    // Force warp1 handoff
    g.vars.psvar_word1 = 0;
    g.vars.pshipflags3 |= PSF3_ENGINESND;
    assert!(player_warp_strat(&mut g, idx));
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 19); // init 20, warp1_strat decbne → 19
    assert_eq!(g.vars.pshipflags3 & PSF3_ENGINESND, 0);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_INVISIBLE, 0);
}

#[test]
fn warp1_flag1_and_zboost() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.psvar_word2 = 10;
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 1);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, 1000);
    g.objs.aliens[idx as usize].worldz = 1000;
    g.vars.dotsflag = 5;

    player_warp1_strat(&mut g, idx);
    // byte1 1→0 → flag1 path
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 1);
    assert_ne!(g.vars.pstratflags & PSTF_FLAG1, 0);
    assert_eq!(g.vars.dotsflag, 0);
    assert_eq!(g.vars.psvar_word2, 12); // +2 at start of warp1
                                        // worldz got the word2 boost before space/viewmove may retarget pview
    assert!(g.objs.aliens[idx as usize].worldz >= 1000i16.wrapping_add(12));

    player_warp2_strat(&mut g, idx);
}

#[test]
fn warp_out_countdown_and_handoff() {
    let mut g = Game::new();
    sf_strat::table::register_all(&mut g);
    let idx = spawn(&mut g);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, 500);
    g.objs.aliens[idx as usize].worldz = 500;

    set_player_warp_out(&mut g, idx);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 64);
    assert_eq!(g.vars.psvar_word2, 128);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 400);
    assert_eq!(g.objs.aliens[idx as usize].vel, 120);
    assert_ne!(g.vars.pstratflags & PSTF_FLAG1, 0);
    assert!(g.objs.aliens.iter().filter(|a| a.active).count() >= 2);
    let warp_out_tick = g.objs.aliens[idx as usize]
        .stratptr
        .expect("warp-out callback");

    // Exercise the installed callback, not just the public body.  A previous
    // ordering bug let set_player_in_space overwrite this with ordinary space
    // flight, leaving PSF_NOCTRL asserted forever in the real map.
    g.call_strat(warp_out_tick, idx);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 63);

    let z0 = g.objs.aliens[idx as usize].worldz;
    assert!(!player_warp_out_strat(&mut g, idx));
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 62);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 390);
    assert_eq!(g.vars.psvar_word2, 124);
    assert!(g.objs.aliens[idx as usize].worldz >= z0.wrapping_add(126));

    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 1);
    assert!(player_warp_out_strat(&mut g, idx));
    assert_ne!(g.vars.pstratflags & PSTF_NOVDISTC, 0);
    assert_eq!(g.vars.viewdist, OUTVIEWDIST);
    assert_eq!(g.vars.player_view_mode, PlayerViewMode::EnteringCockpit);
    assert_eq!(
        g.vars.player_view_options,
        PlayerViewOptions::ExteriorAndCockpit
    );
    let cockpit_tick = g.objs.aliens[idx as usize]
        .stratptr
        .expect("cockpit callback");
    assert_ne!(
        cockpit_tick, warp_out_tick,
        "warp-out must install its view handoff"
    );

    // The next registered tick must enter the cockpit transition rather than
    // wrapping the completed warp-out countdown from 0 back to 255.
    g.call_strat(cockpit_tick, idx);
    assert_ne!(g.vars.sv_u8(sv::PSVAR_BYTE1), 255);
}
