//! ROM monolith eye explode + collide (GB3STRAT.ASM) + RebElaserCol.

use sf_game::alien::{ASF_COLLIDE, ASF_HITFLASH};
use sf_game::Game;
use sf_strat::enemy_a::{
    makelefteyeexp_srou, makerighteyeexp_srou, monolithcol_istrat, rebelasercol_istrat,
    ASF2_SFLAG1, ASF4_NOPOLYEXP, SH_FACE_0,
};

const SHAPE_ELASER2: u16 = 511;
const HF1: u8 = 0x01;
const HF2: u8 = 0x02;
const HF3: u8 = 0x04;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn lefteye_burst_spawns_15_lexp_with_face0_offset() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = SH_FACE_0;
        al.worldx = 1000;
        al.worldy = -500;
        al.worldz = 2000;
    }
    let before = g.objs.active_indices().len();
    makelefteyeexp_srou(&mut g, idx);
    let after = g.objs.active_indices().len();
    assert_eq!(after, before + 15, "15 Lexp children");
    // Spot-check one child: nopolyexp + left face_0 offset base (-20<<4, -30<<4) + rnd.
    let child = g
        .objs
        .active_indices()
        .into_iter()
        .find(|&i| i != idx)
        .expect("child");
    let c = &g.objs.aliens[child as usize];
    assert_ne!(c.sflags4 & ASF4_NOPOLYEXP, 0);
    // Base x = 1000 - 320 = 680, then ±127 rnd → in [553, 807]
    assert!(
        (553..=807).contains(&c.worldx),
        "left face0 x offset, got {}",
        c.worldx
    );
    assert_eq!(c.worldz, 2000i16.wrapping_add(-20));
}

#[test]
fn righteye_burst_non_face0_uses_smaller_offset() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = 0; // not face_0
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 100;
    }
    makerighteyeexp_srou(&mut g, idx);
    let child = g
        .objs
        .active_indices()
        .into_iter()
        .find(|&i| i != idx)
        .expect("child");
    let c = &g.objs.aliens[child as usize];
    // Base x = 0 + 15<<4 = 240 ± rnd
    assert!(
        (113..=367).contains(&c.worldx),
        "right non-face0 x, got {}",
        c.worldx
    );
}

#[test]
fn monolithcol_left_eye_expires_into_burst() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = 1;
        al.sbyte2 = 18;
        al.hitflags = HF2;
        al.sflags |= ASF_COLLIDE;
        al.shape = SH_FACE_0;
    }
    let before = g.objs.active_indices().len();
    monolithcol_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 0);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_HITFLASH, 0);
    assert_eq!(g.objs.aliens[idx as usize].hitflags, 0);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLIDE, 0);
    assert!(g.objs.active_indices().len() >= before + 15);
}

#[test]
fn monolithcol_right_eye_expires_into_burst() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = 18;
        al.sbyte2 = 1;
        al.hitflags = HF3;
        al.shape = 0;
    }
    let before = g.objs.active_indices().len();
    monolithcol_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 0);
    assert!(g.objs.active_indices().len() >= before + 15);
}

#[test]
fn monolithcol_sflag1_skips() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags2 |= ASF2_SFLAG1;
        al.sbyte1 = 5;
        al.hitflags = HF2;
        al.sflags |= ASF_COLLIDE;
    }
    let before = g.objs.active_indices().len();
    monolithcol_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 5);
    assert_eq!(g.objs.aliens[idx as usize].hitflags, 0);
    assert_eq!(g.objs.active_indices().len(), before);
}

#[test]
fn monolithcol_hf1_routes_to_rebelaser() {
    let mut g = Game::new();
    let mono = spawn(&mut g);
    let laser = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[laser as usize];
        al.shape = SHAPE_ELASER2;
        al.roty = 0;
        al.rotx = 10;
        al.vel = 60;
    }
    {
        let al = &mut g.objs.aliens[mono as usize];
        al.hitflags = HF1;
        al.collobjptr = laser;
        al.sflags |= ASF_COLLIDE;
    }
    let before = g.objs.active_indices().len();
    monolithcol_istrat(&mut g, mono);
    // Rebound shot spawned; laser restored.
    assert!(g.objs.active_indices().len() > before);
    assert_eq!(g.objs.aliens[laser as usize].roty, 0);
    assert_eq!(g.objs.aliens[laser as usize].rotx, 10);
    assert_eq!(g.objs.aliens[laser as usize].vel, 60);
}

#[test]
fn rebelasercol_reflects_elaser2() {
    let mut g = Game::new();
    let wall = spawn(&mut g);
    let laser = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[laser as usize];
        al.shape = SHAPE_ELASER2;
        al.roty = 32;
        al.rotx = 20;
        al.vel = 40;
    }
    {
        let al = &mut g.objs.aliens[wall as usize];
        al.collobjptr = laser;
        al.sflags |= ASF_COLLIDE;
    }
    let before = g.objs.active_indices().len();
    rebelasercol_istrat(&mut g, wall);
    assert!(g.objs.active_indices().len() > before);
    assert_eq!(g.objs.aliens[wall as usize].sflags & ASF_COLLIDE, 0);
    assert_eq!(g.objs.aliens[laser as usize].roty, 32);
}
