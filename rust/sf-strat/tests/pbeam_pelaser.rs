//! ROM `Pbeam` / `Pelaser` / `fire_playerbeam` / `fire_Elaser` / `miss_end`.

use sf_game::alien::{ObjectVisualKind, ASF_COLLDISABLE};
use sf_game::vars::{GF_BOSSDEAD, HARD_HP, PSF2_PLAYERHP0};
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::enemy_a::{
    fire_elaser, fire_playerbeam, miss_end, pbeam_istrat, pbeam_strat, pelaser_istrat,
    pelaser_strat, ASF2_SFLAG1, BF_DYING, SH_PLAYER_BEAM,
};

#[test]
fn pelaser_istrat_builds_scaled_vecs_and_anim() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("laser");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 66;
        al.sbyte1 = 0;
        al.sbyte2 = 0;
        al.sbyte3 = 40;
        al.count = 10;
    }
    pelaser_istrat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    assert_eq!(g.objs.aliens[idx as usize].animframe & 0x7F, 4);
    assert_eq!(g.objs.aliens[idx as usize].vel, 66);
    // Without firstframeLcol, colldisable is set
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
}

#[test]
fn pelaser_strat_moves_and_expires_decrementing_numplasers() {
    let mut g = Game::new();
    g.vars.set_sv_u8(sv::NUMPLASERS, 2);
    let idx = g.objs.alloc().expect("laser");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 66;
        al.sbyte3 = 0;
        al.count = 1;
        al.sflags2 |= ASF2_SFLAG1; // skip missbound
        al.vz = 10;
        al.worldz = 100;
    }
    pelaser_istrat(&mut g, idx);
    // Force count=1 after istrat
    g.objs.aliens[idx as usize].count = 1;
    g.objs.aliens[idx as usize].vz = 10;
    g.objs.aliens[idx as usize].worldz = 100;
    pelaser_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(g.vars.sv_u8(sv::NUMPLASERS), 1);
}

#[test]
fn pelaser_strat_removes_if_player_hp0() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("laser");
    g.objs.aliens[idx as usize].count = 5;
    g.vars.pshipflags2 |= PSF2_PLAYERHP0;
    pelaser_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn pbeam_strat_spins_rotz_then_pelaser() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("beam");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 66;
        al.sbyte3 = 0;
        al.count = 5;
        al.rotz = 0;
        al.sflags2 |= ASF2_SFLAG1;
    }
    pbeam_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].count = 5;
    pbeam_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotz, 24);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 0);
    assert_eq!(g.objs.aliens[idx as usize].roty, 0);
    assert_eq!(g.objs.aliens[idx as usize].count, 4);
}

#[test]
fn fire_playerbeam_and_elaser_spawn_stats() {
    let mut g = Game::new();
    let player = g.objs.alloc().expect("p");
    g.objs.aliens[player as usize].vel = 30;
    g.vars.set_sv_u8(sv::NUMPLASERS, 0);

    let beam = fire_playerbeam(&mut g, player).expect("beam");
    assert_eq!(g.objs.aliens[beam as usize].ap, 3);
    assert_eq!(g.objs.aliens[beam as usize].vel, 66);
    assert_eq!(g.objs.aliens[beam as usize].count, 10);
    assert_eq!(g.objs.aliens[beam as usize].shape, SH_PLAYER_BEAM);
    assert_eq!(
        g.objs.aliens[beam as usize].visual_kind,
        ObjectVisualKind::ScaledSprite
    );
    assert_eq!(g.vars.sv_u8(sv::NUMPLASERS), 1);

    let laser = fire_elaser(&mut g, player).expect("laser");
    assert_eq!(g.objs.aliens[laser as usize].ap, 2);
    assert_eq!(g.objs.aliens[laser as usize].shape, 511);
    assert_eq!(g.vars.sv_u8(sv::NUMPLASERS), 2);
}

#[test]
fn miss_end_kills_when_boss_dead() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("w");
    g.objs.aliens[idx as usize].hp = HARD_HP;
    g.vars.gameflags |= GF_BOSSDEAD;
    miss_end(&mut g, idx);
    // kill_istrat sets hp=0 + colldisable
    assert_eq!(g.objs.aliens[idx as usize].hp, 0);
}

#[test]
fn miss_end_noop_when_sflag1() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("w");
    g.objs.aliens[idx as usize].hp = 5;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
    miss_end(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 5);
    let _ = BF_DYING; // keep import used if needed
}
