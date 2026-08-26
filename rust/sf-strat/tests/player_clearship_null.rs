//! Tick 93: ClearShip / ClearShip2 / playernull cutscene leaves.

use sf_game::alien::ASF4_INVISIBLE;
use sf_game::vars::{
    GF_NOZREMOVE, GF_STAGEDONE, GF_VIEWROT, PSF_NOCTRL, PSF_NOFIRE, PSTF_INSEQ, PSTF_NOTDIE,
    PSTF_NOVDISTC,
};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_clear_ship2_strat, player_clear_ship_istrat, player_clear_ship_strat, player_sv as sv,
    playernull_istrat, set_player_clear_ship, set_player_clear_ship2,
};

const MED_PSPEED: i16 = 65;
const MAX_PSPEED: i16 = 85;
const SPACE_VIEWCY: i16 = -60;
const BG2XSCROLL: u16 = 0x1F30;
const BG2SCROLL: u16 = 0x1F32;
const ASF2_SFLAG4: u8 = 0x80;
const VIEWTYPE_FPOS: u8 = 2;
const VIEWTYPE_TOOBJ: u8 = 1;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn playernull_advances_z_and_view() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 1000;
    g.vars.set_sv_i16(sv::PVIEWPOSZ, 500);

    playernull_istrat(&mut g, idx);
    assert_eq!(g.vars.pviewvelz, MED_PSPEED);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 1000 + MED_PSPEED);
    assert_eq!(g.vars.sv_i16(sv::PVIEWPOSZ), 500 + MED_PSPEED);
}

#[test]
fn clearship_init_and_scroll_ramp() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, 2000);
    g.vars.gameflags |= GF_VIEWROT | GF_STAGEDONE;
    g.vars.set_sv_i16(sv::OUTDIST, 80);
    g.vars.set_sv_u8(sv::PLAYER_ZTILT, 40);

    set_player_clear_ship(&mut g, idx);
    assert_ne!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);
    assert_ne!(g.vars.pstratflags & PSTF_INSEQ, 0);
    assert_ne!(g.vars.pstratflags & PSTF_NOVDISTC, 0);
    assert_ne!(g.vars.pstratflags & PSTF_NOTDIE, 0);
    assert_eq!(g.vars.gameflags & GF_STAGEDONE, 0);
    assert_eq!(g.vars.gameflags & GF_VIEWROT, 0);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 0);
    assert_eq!(
        g.objs.aliens[idx as usize].worldy,
        SPACE_VIEWCY.wrapping_sub(40)
    );
    assert_eq!(g.objs.aliens[idx as usize].worldz, 1800);
    assert_eq!(g.objs.aliens[idx as usize].vel, MAX_PSPEED as u8);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 100);
    assert_eq!(g.vars.read_ext8(BG2XSCROLL), 0);

    // Countdown path: sbyte3 100→99, outdist 80→79, bg2scroll stamped
    player_clear_ship_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 99);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 79);
    assert_eq!(g.vars.read_ext16(BG2SCROLL), 232);
    assert_eq!(g.vars.sv_i16(sv::PVIEWPOSZ), 2000 + MED_PSPEED);

    // Force turn path: sbyte3=1 → dec to 0 → scroll += 2
    g.objs.aliens[idx as usize].sbyte3 = 1;
    g.vars.write_ext8(BG2XSCROLL, 0);
    g.vars.gameflags &= !GF_STAGEDONE;
    player_clear_ship_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 1);
    assert_eq!(g.vars.read_ext8(BG2XSCROLL), 2);
    assert_eq!(g.vars.gameflags & GF_STAGEDONE, 0);

    // Boost threshold scroll==222 → sflag4 + stagedone
    g.objs.aliens[idx as usize].sbyte3 = 1;
    g.vars.write_ext8(BG2XSCROLL, 220);
    g.vars.set_sv_i16(sv::PLROTY, 0);
    g.vars.set_sv_i16(sv::PLROTX, 0);
    let z_before = g.objs.aliens[idx as usize].worldz;
    player_clear_ship_strat(&mut g, idx);
    assert_eq!(g.vars.read_ext8(BG2XSCROLL), 222);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG4, 0);
    assert_ne!(g.vars.gameflags & GF_STAGEDONE, 0);
    // maxsc bumps Z by +100 before playermove/vecs; exact Z includes velocity.
    assert!(g.objs.aliens[idx as usize].worldz >= z_before.wrapping_add(100));
    // playermove_srou achases plroty/plrotx toward 0 (rate 3) after maxsc write
    assert_eq!(g.vars.sv_i16(sv::PLROTY), 7); // 8 + (0-8)>>3
    assert_eq!(g.vars.sv_i16(sv::PLROTX), -224); // -256 + (0-(-256))>>3 = -256+32
}

#[test]
fn clearship2_state_machine() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].shape = 42;
    g.objs.aliens[idx as usize].worldz = 100;
    g.vars.set_sv_i16(sv::VIEWPOSX, 0);
    g.vars.set_sv_i16(sv::VIEWPOSY, 0);

    set_player_clear_ship2(&mut g, idx);
    assert_ne!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);
    assert_ne!(g.vars.pstratflags & PSTF_INSEQ, 0);
    assert_ne!(g.vars.gameflags & GF_NOZREMOVE, 0);
    assert_eq!(g.vars.sv_u8(sv::VIEWTYPE), VIEWTYPE_FPOS | VIEWTYPE_TOOBJ);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 95);
    assert_eq!(g.vars.psvar_word1, 42);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 0);
    assert_eq!(g.objs.aliens[idx as usize].worldy, SPACE_VIEWCY);
    assert_eq!(g.vars.sv_i16(sv::VIEWPOSZ), 100 + 1832);
    assert_eq!(g.vars.sv_i16(sv::VIEWPOSX), -20);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 0);

    // Drain state 0 → black shape; same-frame state1 decbne 15→14
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 1);
    player_clear_ship2_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 14);
    // Exact black intact-player mesh (`Bmyship_4`).
    assert_eq!(g.objs.aliens[idx as usize].shape, 372);

    // Drain state 1 → restore + scroll lock; same-frame state2 decbne 200→199
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 1);
    player_clear_ship2_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 2);
    assert_eq!(g.objs.aliens[idx as usize].shape, 42);
    assert_eq!(g.vars.read_ext8(BG2XSCROLL), 254);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 199);

    // Drain state 2 → boost; same-frame state3 decbne 85→84
    // state2 +10, state3 +10, always med-15
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 1);
    let vz_before = g.vars.sv_i16(sv::VIEWPOSZ);
    player_clear_ship2_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 3);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG4, 0);
    assert_eq!(g.vars.sv_u8(sv::VIEWTYPE), VIEWTYPE_FPOS);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 84);
    assert_eq!(
        g.vars.sv_i16(sv::VIEWPOSZ),
        vz_before
            .wrapping_add(10)
            .wrapping_add(10)
            .wrapping_add(MED_PSPEED.wrapping_sub(15))
    );

    // Drain state 3 → stagedone
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 1);
    g.vars.gameflags &= !GF_STAGEDONE;
    player_clear_ship2_strat(&mut g, idx);
    assert_ne!(g.vars.gameflags & GF_STAGEDONE, 0);
}

#[test]
fn clearship_far_z_goes_invisible() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    player_clear_ship_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldz = 10_000;
    g.vars.set_sv_i16(sv::PVIEWPOSZ, 1000);
    g.objs.aliens[idx as usize].sbyte3 = 50; // stay in nturn
    player_clear_ship_strat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags4 & ASF4_INVISIBLE, 0);
}
