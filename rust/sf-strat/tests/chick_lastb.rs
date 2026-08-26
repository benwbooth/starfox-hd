//! ROM chick hatchling + lastb2/3/4 final-base doors + public amoeba aliases.

use sf_game::alien::{ASF4_INVISIBLE, ASF_COLLDISABLE, ASF_SHADOW};
use sf_game::vars::{GF_STRATDONE1, PSTF_INSEQ};
use sf_game::Game;
use sf_strat::bosses::{
    amoeba_cont, amoebacol_istrat, amoebahome_init, amoebahome_strat, amoebastick_istrat,
    amoebastick_strat, chick_istrat, chick_strat, lastb2_istrat, lastb3_istrat, lastb4_istrat,
};
use sf_strat::common::{sv, StratRam};

const WM_GAMEFLAGS2: u16 = 0x155C;
const GF2_STRATFLAG1: u8 = 1;
const GF2_STRATFLAG2: u8 = 2;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].worldx = 0;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
    g.vars.set_sv_i16(sv::VIEWTOOBJ, 0);
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].worldy = -100;
    idx
}

#[test]
fn chick_aims_and_flies() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldx = 200;
    g.objs.aliens[idx as usize].worldy = -40;
    g.objs.aliens[idx as usize].worldz = 500;
    chick_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].ap, 8);
    assert_eq!(g.objs.aliens[idx as usize].vel, 80);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);
    // dobj2obj3dangle_xy stores nega(Yanglexy) (player at 0,0 from chick 200,500).
    let raw = sf_core::aim_angle::yanglexy(-200, -500);
    assert_eq!(g.objs.aliens[idx as usize].roty, raw.wrapping_neg());
    // Aimed toward player (not still at default 0/0 unless already aligned).
    assert!(g.objs.aliens[idx as usize].vx != 0 || g.objs.aliens[idx as usize].vz != 0);
    let z0 = g.objs.aliens[idx as usize].worldz;
    g.vars.pviewvelz = 0;
    chick_strat(&mut g, idx);
    // addvecs moved it.
    let al = &g.objs.aliens[idx as usize];
    assert!(al.worldz != z0 || al.worldx != 200);
}

#[test]
fn lastb2_visibility_gates() {
    let mut g = Game::new();
    let idx = spawn_obj(&mut g);
    // Not in seq → invisible.
    g.vars.pstratflags = 0;
    g.vars.write_ext8(WM_GAMEFLAGS2, 0);
    lastb2_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags4 & ASF4_INVISIBLE, 0);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    // In seq + flag1 → visible.
    g.vars.pstratflags |= PSTF_INSEQ;
    g.vars.write_ext8(WM_GAMEFLAGS2, GF2_STRATFLAG1);
    lastb2_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sflags4 & ASF4_INVISIBLE, 0);
}

#[test]
fn lastb3_opens_and_sets_flag2() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldy = -40; // |dy| to player < 500
    g.vars.pstratflags |= PSTF_INSEQ;
    g.vars.write_ext8(WM_GAMEFLAGS2, GF2_STRATFLAG1);
    lastb3_istrat(&mut g, idx);
    assert_ne!(g.vars.read_ext8(WM_GAMEFLAGS2) & GF2_STRATFLAG2, 0);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 1);
    // Far in Y → close path resets anim.
    g.objs.aliens[idx as usize].worldy = -1000;
    lastb3_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 0);
}

#[test]
fn lastb4_opens_and_sets_stratdone1() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldy = -40; // |dy|<300
    g.vars.pstratflags |= PSTF_INSEQ;
    g.vars.write_ext8(WM_GAMEFLAGS2, GF2_STRATFLAG2);
    lastb4_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 64); // DEG90
    assert_eq!(g.objs.aliens[idx as usize].sflags4 & ASF4_INVISIBLE, 0);
    assert_ne!(g.vars.gameflags & GF_STRATDONE1, 0);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 1);
}

#[test]
fn amoeba_public_aliases_callable() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 1000;
    amoeba_cont(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 940);
    // home/stick/col aliases don't panic.
    amoebahome_init(&mut g, idx);
    amoebahome_strat(&mut g, idx);
    g.objs.aliens[idx as usize].collobjptr = 0;
    g.vars.write_ext8(0x162b, 0); // slimecount
    amoebacol_istrat(&mut g, idx);
    // Force stick path via public istrat.
    amoebastick_istrat(&mut g, idx);
    amoebastick_strat(&mut g, idx);
}
