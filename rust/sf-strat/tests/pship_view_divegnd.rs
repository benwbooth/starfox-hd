//! Tick 87: pshipdivegnd + viewdivegnd.

use sf_game::alien::{ASF4_INVISIBLE, ASF_SHADOW, ATZREMOVE};
use sf_game::vars::{GF_NOZREMOVE, OUTVIEWDIST};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_sv as sv, pshipdivegnd_istrat, pshipdivegnd_strat, viewdivegnd_istrat, viewdivegnd_strat,
};

const MED_PSPEED: i16 = 65;
const DEG90: u8 = 64;
const PLANET_VIEW_CY: i16 = -215;
const PSHIP_Y: i16 = -2853 + 105 + PLANET_VIEW_CY;
const VIEW_Y: i16 = -2692 + 105 + PLANET_VIEW_CY;
const VIEWTYPE_NORM: u8 = 0;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn pshipdivegnd_init_spin_then_level() {
    let mut g = Game::new();
    let cam = spawn(&mut g);
    g.objs.aliens[cam as usize].worldz = 0;

    let ship = spawn(&mut g);
    g.objs.aliens[ship as usize].worldz = 50; // |dz| < outviewdist
    g.objs.aliens[ship as usize].sword1 = cam as i16;

    pshipdivegnd_istrat(&mut g, ship);
    assert_eq!(g.objs.aliens[ship as usize].rotx, DEG90);
    assert_eq!(g.objs.aliens[ship as usize].vel, 50);
    assert_eq!(g.objs.aliens[ship as usize].worldy, PSHIP_Y);
    assert_ne!(g.objs.aliens[ship as usize].sflags & ASF_SHADOW, 0);
    assert_eq!(g.objs.aliens[ship as usize].type_ & ATZREMOVE, 0);
    assert_eq!(g.objs.aliens[ship as usize].sbyte1, 54);

    let rz0 = g.objs.aliens[ship as usize].rotz;
    pshipdivegnd_strat(&mut g, ship);
    assert_eq!(g.objs.aliens[ship as usize].rotz, rz0.wrapping_sub(8));
    assert_eq!(g.objs.aliens[ship as usize].sbyte1, 53);
    assert!(g.objs.aliens[ship as usize].vel <= 50);

    // Force into .fin path (sbyte1 < 20 after dec)
    g.objs.aliens[ship as usize].sbyte1 = 20;
    g.vars.gameflags |= GF_NOZREMOVE;
    pshipdivegnd_strat(&mut g, ship);
    assert_eq!(g.objs.aliens[ship as usize].sbyte1, 19);
    assert_eq!(g.vars.gameflags & GF_NOZREMOVE, 0);
}

#[test]
fn pshipdivegnd_handoff_when_far() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    assert_eq!(player, 0);
    g.vars.internal_playpt = 0;
    g.objs.aliens[0].sflags4 |= ASF4_INVISIBLE;

    let cam = spawn(&mut g);
    g.objs.aliens[cam as usize].worldz = 0;

    let ship = spawn(&mut g);
    g.objs.aliens[ship as usize].worldz = 500; // |dz| >= 120
    g.objs.aliens[ship as usize].worldx = 11;
    g.objs.aliens[ship as usize].worldy = -40;
    g.objs.aliens[ship as usize].sword1 = cam as i16;
    pshipdivegnd_istrat(&mut g, ship);

    pshipdivegnd_strat(&mut g, ship);
    assert!(!g.objs.aliens[cam as usize].active);
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(g.vars.sv_u8(sv::VIEWTYPE), VIEWTYPE_NORM);
    assert_eq!(g.vars.pviewvelz, MED_PSPEED);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), OUTVIEWDIST);
    assert_eq!(g.vars.viewdist, OUTVIEWDIST);
    assert_eq!(g.objs.aliens[0].sflags4 & ASF4_INVISIBLE, 0);
    assert_eq!(g.objs.aliens[0].worldx, 11);
    assert_eq!(g.objs.aliens[0].vel, MED_PSPEED as u8);
}

#[test]
fn viewdivegnd_tracks_and_rolls() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    g.vars.internal_playpt = player as i16;

    let ship = spawn(&mut g);
    g.objs.aliens[ship as usize].vz = 10;

    let cam = spawn(&mut g);
    g.objs.aliens[cam as usize].sword1 = ship as i16;
    g.objs.aliens[cam as usize].worldz = 100;
    viewdivegnd_istrat(&mut g, cam);
    assert_eq!(g.objs.aliens[cam as usize].worldy, VIEW_Y);
    assert_eq!(g.objs.aliens[cam as usize].vel, 20);
    assert_eq!(g.objs.aliens[cam as usize].rotx, DEG90);
    assert_eq!(g.objs.aliens[cam as usize].sbyte1, 90);
    assert_eq!(g.vars.sv_i16(sv::OUTVX), -(64 * 256));
    assert_eq!(g.vars.sv_i16(sv::OUTVZ), 0);

    let z0 = g.objs.aliens[cam as usize].worldz;
    viewdivegnd_strat(&mut g, cam);
    // +ship.vz then gen_vecs (rotx=90 → vz from vel)
    assert_ne!(g.objs.aliens[cam as usize].worldz, z0);
    assert_eq!(g.objs.aliens[cam as usize].sbyte1, 89);
    assert_eq!(g.vars.sv_i16(sv::OUTVZ), 0i16.wrapping_sub(4 * 256));
    // Player copied to cam pos
    assert_eq!(
        g.objs.aliens[player as usize].worldy,
        g.objs.aliens[cam as usize].worldy
    );

    // .fin path
    g.objs.aliens[cam as usize].sbyte1 = 40;
    viewdivegnd_strat(&mut g, cam);
    assert_eq!(g.objs.aliens[cam as usize].sbyte1, 39);
    // outvz chases toward 0
    assert!(g.vars.sv_i16(sv::OUTVZ).abs() < (4 * 256));

    // beqdec zero → still +target.vz, then .fin2 subtracts 3 from worldz
    g.objs.aliens[cam as usize].sbyte1 = 0;
    let z1 = g.objs.aliens[cam as usize].worldz;
    viewdivegnd_strat(&mut g, cam);
    assert_eq!(
        g.objs.aliens[cam as usize].worldz,
        z1.wrapping_add(10).wrapping_sub(3)
    );
}
