//! Tick 166: remaining inline `s_obj2obj_*` / `s_face_player` movement-aim
//! sites store `nega(Yanglexy)`. Weapon_rots2obj / Yanglexabs fire stays raw.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::game::Game;
use sf_strat::enemy_a::{
    bee1a_init, cam2dash_strat, fire_bonfire, headfire_istrat, headfire_strat, helpballhome_istrat,
    strat_spacebarwalker_init, ASF2_SMFLAG1, DEG0,
};

fn spawn_player(g: &mut Game, x: i16, y: i16, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posx = x;
    g.vars.player_posy = y;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn yanglexy(src_x: i16, src_z: i16, dst_x: i16, dst_z: i16) -> u8 {
    let dx = (dst_x as i32 - src_x as i32) as f32;
    let dz = (dst_z as i32 - src_z as i32) as f32;
    let mut a = dx.atan2(dz);
    if a < 0.0 {
        a += 2.0 * 3.141_592_65_f32;
    }
    ((a * (256.0 / (2.0 * 3.141_592_65_f32))) as i32) as u8
}

fn achase_step(cur: u8, target: u8, shift: u32) -> u8 {
    if cur == target {
        return cur;
    }
    let diff = (target.wrapping_sub(cur) as i8) as i32;
    let mut step = if diff >= 0 {
        diff >> shift
    } else {
        -((-diff) >> shift)
    };
    if step == 0 {
        step = if diff > 0 { 1 } else { -1 };
    }
    cur.wrapping_add(step as u8)
}

/// headfire ground snap: `dobj2obj3dangle_xy` → roty = nega(Yanglexy).
#[test]
fn headfire_ground_snaps_negated_yaw() {
    let mut g = Game::new();
    spawn_player(&mut g, 400, 0, 0);
    let idx = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 0;
        al.worldy = -50;
        al.worldz = 200;
        al.vy = 0;
    }
    headfire_istrat(&mut g, idx);
    // One tick applies gravity while still airborne. Force the first contact,
    // then let the source bounce decay before it switches to the dash.
    headfire_strat(&mut g, idx);
    g.objs.aliens[idx as usize].worldy = -1;
    g.objs.aliens[idx as usize].vy = 8;
    for _ in 0..16 {
        headfire_strat(&mut g, idx);
        if g.objs.aliens[idx as usize].sbyte1 == 1 {
            break;
        }
    }
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 1);
    let raw = yanglexy(0, 200, 400, 0);
    let neg = raw.wrapping_neg();
    assert_ne!(raw, neg);
    assert_eq!(
        g.objs.aliens[idx as usize].roty, neg,
        "headfire ground aim must be nega(Yanglexy); raw={raw}"
    );
}

/// bonfire projectile aim uses obj2obj_3dangle (nega), not Yanglexabs.
#[test]
fn fire_bonfire_stores_negated_yaw() {
    let mut g = Game::new();
    spawn_player(&mut g, 300, 0, 900);
    let firer = spawn(&mut g);
    g.objs.aliens[firer as usize].worldx = 0;
    g.objs.aliens[firer as usize].worldz = 0;
    let ball = fire_bonfire(&mut g, firer).expect("bonfire");
    let raw = yanglexy(0, 0, 300, 900);
    let neg = raw.wrapping_neg();
    assert_ne!(raw, neg);
    assert_eq!(
        g.objs.aliens[ball as usize].roty, neg,
        "bonfire roty must be nega(Yanglexy); raw={raw}"
    );
}

/// spacebarwalker body Achase uses nega; fire stays raw (weapon_rots2obj).
#[test]
fn spacebarwalker_body_negated() {
    let mut g = Game::new();
    spawn_player(&mut g, 500, -40, 100);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldz = 800;
    g.objs.aliens[idx as usize].roty = DEG0;
    strat_spacebarwalker_init(&mut g, idx); // runs body once on spawn frame
    let raw = yanglexy(0, 800, 500, 100);
    let neg = raw.wrapping_neg();
    let expect = achase_step(DEG0, neg, 1);
    let wrong = achase_step(DEG0, raw, 1);
    let got = g.objs.aliens[idx as usize].roty;
    assert_eq!(got, expect, "body must chase nega; raw={raw} neg={neg}");
    assert_ne!(got, wrong);
}

/// bee1a face_player latch stores nega(Yanglexy) into sbyte3.
#[test]
fn bee1a_latches_negated_yaw() {
    let mut g = Game::new();
    spawn_player(&mut g, 350, -40, 400);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldz = 400;
    let raw = yanglexy(0, 400, 350, 400);
    let neg = raw.wrapping_neg();
    bee1a_init(&mut g, idx); // clears smflag1 then runs strat → latch
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_SMFLAG1, 0);
    assert_eq!(
        g.objs.aliens[idx as usize].sbyte3, neg,
        "bee1a latch must be nega(Yanglexy); raw={raw}"
    );
}

/// helpballhome snap aim into sbyte1 is nega(Yanglexy).
#[test]
fn helpballhome_snaps_negated_yaw() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, 0, 0);
    let target = spawn(&mut g);
    g.objs.aliens[target as usize].worldx = 250;
    g.objs.aliens[target as usize].worldz = 600;
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldz = 0;
    g.objs.aliens[idx as usize].ptr = target.wrapping_add(1); // index+1 encoding
    helpballhome_istrat(&mut g, idx);
    let s = g.objs.aliens[idx as usize].stratptr.expect("strat");
    g.call_strat(s, idx);
    let raw = yanglexy(0, 0, 250, 600);
    let neg = raw.wrapping_neg();
    assert_ne!(raw, neg);
    assert_eq!(
        g.objs.aliens[idx as usize].sbyte1, neg,
        "helpballhome sbyte1 must be nega; raw={raw}"
    );
}

/// cam2dash aim stash sbyte1 uses nega when |dz|>=300.
#[test]
fn cam2dash_aim_stash_negated() {
    let mut g = Game::new();
    spawn_player(&mut g, 200, 0, 0);
    let idx = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 0;
        al.worldz = 500;
        al.rotx = 128; // DEG180 — skip pitch chase
        al.sbyte1 = 0;
        al.sbyte2 = 0;
        al.vel = 30;
    }
    cam2dash_strat(&mut g, idx);
    let raw = yanglexy(0, 500, 200, 0);
    let neg = raw.wrapping_neg();
    let expect = achase_step(0, neg, 2);
    assert_eq!(
        g.objs.aliens[idx as usize].sbyte1, expect,
        "cam2dash sbyte1 must chase nega; raw={raw}"
    );
}
