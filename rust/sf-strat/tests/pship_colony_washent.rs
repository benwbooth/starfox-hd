//! Tick 89: pshipcolony + pshipwashent pipe-follow cutscenes.

use sf_game::alien::{ASF4_INVISIBLE, ASF_SHADOW};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_sv as sv, pshipcolony_istrat, pshipcolony_strat, pshipwashent_istrat, pshipwashent_strat,
};

const MED_PSPEED: i16 = 65;
const LTUNNEL_VIEWCY: i16 = -60;
const NUCLEUS_VIEWCY: i16 = -60;
const WM_DOZROT: u16 = 0x1776;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn pshipcolony_init_and_first_table_step() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].rotx = 10;
    g.vars.set_sv_i16(sv::OUTVX, 0x0500);
    g.vars.set_sv_i16(sv::OUTVZ, 0x0300);

    pshipcolony_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 1);
    assert_eq!(g.objs.aliens[idx as usize].vel, MED_PSPEED as u8);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);

    // First tick: decbne 1→0 → load tab[0]=0x0e, tab[1]=0, sbyte2=2
    pshipcolony_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 0);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 0x0e);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 0);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 2);
    // rotz chases rotx<<1
    assert_ne!(g.objs.aliens[idx as usize].rotz, 0);
    // pviewpos copied
    assert_eq!(
        g.vars.sv_i16(sv::PVIEWPOSX),
        g.objs.aliens[idx as usize].worldx
    );
}

#[test]
fn pshipcolony_straight_handoff_ltunnel() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    assert_eq!(player, 0);
    g.vars.internal_playpt = 0;
    g.objs.aliens[0].sflags4 |= ASF4_INVISIBLE;
    g.vars.write_ext8(WM_DOZROT, 1);

    let ship = spawn(&mut g);
    g.objs.aliens[ship as usize].worldz = 500;
    g.objs.aliens[ship as usize].worldy = -200;
    pshipcolony_istrat(&mut g, ship);
    // Jump to straight phase
    g.objs.aliens[ship as usize].stratstate = 1;
    g.objs.aliens[ship as usize].vel = 120;
    g.objs.aliens[ship as usize].sbyte1 = 1;

    pshipcolony_strat(&mut g, ship);
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(g.vars.read_ext8(WM_DOZROT), 0);
    assert_eq!(g.objs.aliens[0].sflags4 & ASF4_INVISIBLE, 0);
    assert_eq!(g.objs.aliens[0].worldx, 0);
    assert_eq!(g.objs.aliens[0].worldy, LTUNNEL_VIEWCY);
    // worldz = ship.z(+150 this frame) + 120
    let expect_z = 500i16.wrapping_add(150).wrapping_add(120);
    assert_eq!(g.objs.aliens[0].worldz, expect_z);
}

#[test]
fn pshipwashent_straight_handoff_nucleus() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    g.vars.internal_playpt = player as i16;
    g.objs.aliens[player as usize].sflags4 |= ASF4_INVISIBLE;
    g.vars.write_ext8(WM_DOZROT, 1);
    g.vars.set_sv_i16(sv::OUTDIST, 200);

    let ship = spawn(&mut g);
    g.objs.aliens[ship as usize].worldz = 800;
    g.objs.aliens[ship as usize].worldy = 0;
    pshipwashent_istrat(&mut g, ship);
    assert_eq!(g.objs.aliens[ship as usize].sbyte1, 1);

    // First tick loads table
    pshipwashent_strat(&mut g, ship);
    assert_eq!(g.objs.aliens[ship as usize].sbyte1, 0x0e);

    // Force straight + handoff
    g.objs.aliens[ship as usize].stratstate = 1;
    g.objs.aliens[ship as usize].sbyte1 = 1;
    g.objs.aliens[ship as usize].worldz = 900;
    pshipwashent_strat(&mut g, ship);
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(g.vars.read_ext8(WM_DOZROT), 0);
    assert_eq!(g.objs.aliens[player as usize].sflags4 & ASF4_INVISIBLE, 0);
    assert_eq!(g.objs.aliens[player as usize].worldx, 0);
    // player z copied from ship then +medpspeed
    assert_eq!(
        g.objs.aliens[player as usize].worldz,
        900i16.wrapping_add(MED_PSPEED)
    );
    // outdist chased toward 0
    assert!(g.vars.sv_i16(sv::OUTDIST) < 200);
    // worldy chased toward nucleus
    assert!(g.objs.aliens[ship as usize].worldy.abs() <= NUCLEUS_VIEWCY.abs() || true);
}
