//! Tick 101: pcolRW / pendcolB / pendcolLW / pendcolRW (PSTRATS.ASM).

use sf_game::alien::{ASF2_COLLDISABLE, ASF_COLLDISABLE};
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::player::{
    pcbox_attach, pcolrw_istrat, pcolrw_strat, pendcolb_istrat, pendcollw_istrat, pendcolrw_istrat,
};

const PSF_BODYCOLL: u8 = 1;
const PSF_LWINGCOLL: u8 = 2;
const PSF_RWINGCOLL: u8 = 4;
const PSF_BRKRWING: u8 = 16;
const SCREENFLASH_WING_FRMS: u8 = 2;
const SCREENFLASH_WING_TYPE: u8 = 1;
const HALF_TURN_ANGLE: u8 = 128;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn pendcolb_clears_body_coll_flag() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    assert!(pcbox_attach(&mut g, player));
    let body = g.coldet.pcbox.body.expect("body");
    g.vars.pshipflags |= PSF_BODYCOLL;
    g.vars.set_sv_i16(sv::PCOLLOBJ_B, 7);
    pendcolb_istrat(&mut g, body);
    assert_eq!(g.vars.sv_i16(sv::PCOLLOBJ_B), 0);
    assert_eq!(g.vars.pshipflags & PSF_BODYCOLL, 0);
    assert!(g.objs.aliens[body as usize].collstratptr.is_some());
}

#[test]
fn pcolrw_sets_flags_flash_and_fx() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    g.vars.internal_playpt = player as i16;
    assert!(pcbox_attach(&mut g, player));
    let rwing = g.coldet.pcbox.rwing.expect("rwing");
    let partner = spawn(&mut g);
    g.objs.aliens[partner as usize].ap = 10;
    g.objs.aliens[rwing as usize].collobjptr = partner;
    g.objs.aliens[player as usize].rotz = 0; // positive → plrotx-8 path

    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    pcolrw_istrat(&mut g, rwing);
    assert_eq!(g.vars.sv_u8(sv::SCREENFLASHCNT), SCREENFLASH_WING_FRMS);
    assert_eq!(g.vars.sv_u8(sv::SCREENFLASHTYPE), SCREENFLASH_WING_TYPE);
    assert_ne!(g.vars.pshipflags & PSF_RWINGCOLL, 0);
    assert_eq!(g.vars.sv_i16(sv::PCOLLOBJ_RW), partner as i16);
    assert!(g.objs.aliens[rwing as usize].endcollstratptr.is_some());
    // spexplod FX spawned into sword1
    assert!(g.objs.aliens[rwing as usize].sword1 > 0);
    let effect = g.objs.aliens[rwing as usize].sword1 as usize;
    assert_eq!(g.objs.aliens[effect].roty, HALF_TURN_ANGLE);
    assert_eq!(g.objs.aliens[effect].sflags & ASF_COLLDISABLE, 0);
    assert_ne!(g.objs.aliens[effect].sflags2 & ASF2_COLLDISABLE, 0);
    assert!(g.objs.aliens.iter().filter(|a| a.active).count() > before);

    pcolrw_strat(&mut g, rwing); // scrape spark path
}

#[test]
fn pcolrw_broken_wing_bounces_to_body() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    assert!(pcbox_attach(&mut g, player));
    let rwing = g.coldet.pcbox.rwing.expect("rwing");
    let body = g.coldet.pcbox.body.expect("body");
    let partner = spawn(&mut g);
    g.objs.aliens[rwing as usize].collobjptr = partner;
    g.vars.pshipflags |= PSF_BRKRWING;
    pcolrw_strat(&mut g, rwing);
    assert_eq!(g.objs.aliens[body as usize].collobjptr, partner);
    assert_ne!(
        g.objs.aliens[body as usize].sflags & sf_game::alien::ASF_COLLIDE,
        0
    );
}

#[test]
fn pendcolrw_clears_and_removes_fx() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    assert!(pcbox_attach(&mut g, player));
    let rwing = g.coldet.pcbox.rwing.expect("rwing");
    let fx = spawn(&mut g);
    g.objs.aliens[rwing as usize].sword1 = fx as i16;
    g.vars.pshipflags |= PSF_RWINGCOLL;
    g.vars.set_sv_i16(sv::PCOLLOBJ_RW, 3);
    pendcolrw_istrat(&mut g, rwing);
    assert_eq!(g.vars.sv_i16(sv::PCOLLOBJ_RW), 0);
    assert_eq!(g.vars.pshipflags & PSF_RWINGCOLL, 0);
    assert_eq!(g.objs.aliens[rwing as usize].sword1, 0);
    assert!(!g.objs.aliens[fx as usize].active);
}

#[test]
fn pendcollw_clears_lwing_flag() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    assert!(pcbox_attach(&mut g, player));
    let lwing = g.coldet.pcbox.lwing.expect("lwing");
    g.vars.pshipflags |= PSF_LWINGCOLL;
    g.vars.set_sv_i16(sv::PCOLLOBJ_LW, 9);
    pendcollw_istrat(&mut g, lwing);
    assert_eq!(g.vars.sv_i16(sv::PCOLLOBJ_LW), 0);
    assert_eq!(g.vars.pshipflags & PSF_LWINGCOLL, 0);
}
