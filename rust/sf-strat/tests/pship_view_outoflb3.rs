//! Tick 85: pshipoutoflb3 + viewoutoflb3 + viewlb3move_srou.

use sf_game::alien::ATGND;
use sf_game::vars::{GF_STRATDONE1, PSF3_ENGINESND};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_sv as sv, pshipoutoflb3_istrat, pshipoutoflb3_strat, view_lb3_move, viewlb3move_srou,
    viewoutoflb3_istrat, viewoutoflb3_strat,
};

const WM_GAMEFLAGS2: u16 = 0x155C;
const GF2_STRATFLAG1: u8 = 1;
const MED_PSPEED: i16 = 65;
const VIEWTYPE_NORM: u8 = 0;
const VIEWTYPE_FPOS: u8 = 2;
const VIEWTYPE_TOOBJ: u8 = 1;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn pshipoutoflb3_cruise_wait_boost() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 1000;
    g.vars.set_sv_i16(sv::VIEWPOSZ, 0);
    g.vars.gameflags |= GF_STRATDONE1;

    pshipoutoflb3_istrat(&mut g, idx);
    assert_eq!(g.vars.gameflags & GF_STRATDONE1, 0);
    assert_ne!(g.objs.aliens[idx as usize].type_ & ATGND, 0);
    assert_eq!(g.vars.sv_u8(sv::VIEWTYPE), VIEWTYPE_FPOS | VIEWTYPE_TOOBJ);

    let z0 = g.objs.aliens[idx as usize].worldz;
    pshipoutoflb3_strat(&mut g, idx);
    // |dz| to viewposz=0 is 1000+… > 500 → +11 extra
    assert_eq!(
        g.objs.aliens[idx as usize].worldz,
        z0.wrapping_add(MED_PSPEED.wrapping_add(19))
            .wrapping_add(11)
    );

    // Hand off when viewtype_norm
    g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_NORM);
    pshipoutoflb3_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);

    // State 1: sword1 eases toward -3 on notdelay-2
    g.objs.aliens[idx as usize].sword1 = 0;
    g.vars.gameframe = 0; // gate open
    let z1 = g.objs.aliens[idx as usize].worldz;
    pshipoutoflb3_strat(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].worldz,
        z1.wrapping_add(MED_PSPEED.wrapping_add(19))
    );
    assert_eq!(g.objs.aliens[idx as usize].sword1, -1);
    assert_eq!(g.vars.pviewvelz, MED_PSPEED.wrapping_add(16));

    // gf2_stratflag1 → state 2
    g.vars.write_ext8(WM_GAMEFLAGS2, GF2_STRATFLAG1);
    pshipoutoflb3_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 2);

    // Boost when sbyte2 hits 0
    g.vars.pshipflags3 |= PSF3_ENGINESND;
    g.objs.aliens[idx as usize].sbyte2 = 1;
    pshipoutoflb3_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 3);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 15);
    assert_eq!(g.vars.sv_i16(sv::BOOSTOBJ), idx as i16);
    assert_eq!(g.vars.pshipflags3 & PSF3_ENGINESND, 0);
}

#[test]
fn viewoutoflb3_close_then_swing() {
    let mut g = Game::new();
    let target = spawn(&mut g);
    g.objs.aliens[target as usize].worldz = 200;
    g.vars.set_sv_i16(sv::VIEWTOOBJ, target as i16);
    g.vars.write_ext8(WM_GAMEFLAGS2, GF2_STRATFLAG1);

    let cam = spawn(&mut g);
    g.objs.aliens[cam as usize].worldz = 0;
    g.objs.aliens[cam as usize].worldx = 10;
    g.objs.aliens[cam as usize].worldy = -20;
    viewoutoflb3_istrat(&mut g, cam);
    assert_eq!(g.objs.aliens[cam as usize].vel, MED_PSPEED as u8);
    assert_eq!(g.objs.aliens[cam as usize].sbyte1, 50);
    assert_eq!(g.vars.read_ext8(WM_GAMEFLAGS2) & GF2_STRATFLAG1, 0);

    // Far: just cruise + copy viewpos
    g.objs.aliens[cam as usize].worldz = 0;
    g.objs.aliens[target as usize].worldz = 5000;
    viewoutoflb3_strat(&mut g, cam);
    assert_eq!(g.objs.aliens[cam as usize].stratstate, 0);
    assert_eq!(
        g.vars.sv_i16(sv::VIEWPOSX),
        g.objs.aliens[cam as usize].worldx
    );

    // Close + countdown → state 1
    g.objs.aliens[cam as usize].worldz = 100;
    g.objs.aliens[target as usize].worldz = 200;
    g.objs.aliens[cam as usize].sbyte1 = 1;
    viewoutoflb3_strat(&mut g, cam);
    assert_eq!(g.objs.aliens[cam as usize].stratstate, 1);
    // nextstate falls into state 1 same frame → outdist 280 then +2
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 282);
    assert_eq!(g.vars.sv_u8(sv::VIEWTYPE), VIEWTYPE_NORM);
    assert_eq!(g.objs.aliens[cam as usize].sword1, 229); // 230 then same-frame dec

    // viewlb3move pins pviewpos to viewtoobj
    viewlb3move_srou(&mut g);
    assert_eq!(
        g.vars.sv_i16(sv::PVIEWPOSZ),
        g.objs.aliens[target as usize]
            .worldz
            .wrapping_add(MED_PSPEED.wrapping_add(15))
    );

    // Player scroll helper still works
    g.vars.set_sv_i16(sv::BG2YSCROLL, 200);
    view_lb3_move(&mut g);
    assert_eq!(g.vars.sv_i16(sv::BG2YSCROLL), 199);
}

#[test]
fn viewoutoflb3_zoomin_sets_gf2() {
    let mut g = Game::new();
    let target = spawn(&mut g);
    g.vars.set_sv_i16(sv::VIEWTOOBJ, target as i16);
    let cam = spawn(&mut g);
    viewoutoflb3_istrat(&mut g, cam);
    g.objs.aliens[cam as usize].stratstate = 6;
    g.objs.aliens[cam as usize].sbyte1 = 1;
    g.vars.set_sv_i16(sv::OUTDIST, 200);
    viewoutoflb3_strat(&mut g, cam);
    assert_ne!(g.vars.read_ext8(WM_GAMEFLAGS2) & GF2_STRATFLAG1, 0);
    assert_eq!(g.vars.sv_u8(sv::VIEWTYPE), VIEWTYPE_FPOS);
    assert_eq!(g.objs.aliens[cam as usize].sbyte1, 1);
}
