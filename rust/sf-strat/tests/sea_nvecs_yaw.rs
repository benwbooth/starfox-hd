//! Tick 170: sea `s_gen_vecs` → `nvecs_l` + flyingfish/bossseamon obj2obj yaw `nega`.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::game::Game;
use sf_strat::bosses::{
    bossseamon_strat, flyingfish_init, sea_gen_vecs_angle, strat_bossseamon_init,
};
use sf_strat::common::strat_nvecs;
use sf_strat::snes_trig::{mulslog, COSTAB, SINTAB};

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

fn nvecs_expect(angle: u8, vel: u8) -> (i16, i16) {
    let idx = angle.wrapping_neg().wrapping_add(1) as usize;
    let v = vel as i8 as i32;
    (
        mulslog(v, SINTAB[idx] as i32) as i16,
        mulslog(v, COSTAB[idx] as i32) as i16,
    )
}

/// `sea_gen_vecs_angle` matches ROM `nvecs_l` (-angle+1), preserves vy.
#[test]
fn sea_gen_vecs_matches_nvecs() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].vel = 70;
    g.objs.aliens[idx as usize].vy = -15;
    let angle = 40u8;
    let (ex, ez) = nvecs_expect(angle, 70);
    // Float sin(angle) without nega would differ for this angle.
    let float_wrong_vx =
        ((70.0f32) * ((angle as f32) * (2.0 * std::f32::consts::PI / 256.0)).sin()) as i16;
    assert_ne!(ex, float_wrong_vx, "nvecs must differ from raw float sin");

    sea_gen_vecs_angle(&mut g, idx, angle);
    assert_eq!(g.objs.aliens[idx as usize].vx, ex);
    assert_eq!(g.objs.aliens[idx as usize].vz, ez);
    assert_eq!(g.objs.aliens[idx as usize].vy, -15);
    assert_eq!(strat_nvecs(angle, 70), (ex, ez));
}

/// bossseamon state 7: `s_obj2obj_angle` into sbyte2 stores nega(Yanglexy).
#[test]
fn bossseamon_state7_aims_with_negated_yanglexy() {
    let mut g = Game::new();
    spawn_player(&mut g, 400, 0, 0);
    let boss = spawn(&mut g);
    strat_bossseamon_init(&mut g, boss);
    {
        let al = &mut g.objs.aliens[boss as usize];
        al.stratstate = 7;
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 2000; // |dz|>=200 → aim band
        al.vel = 0;
        al.sbyte2 = 0;
    }
    let raw = yanglexy(0, 2000, 400, 0);
    let neg = raw.wrapping_neg();
    assert_ne!(raw, neg);

    bossseamon_strat(&mut g, boss);
    assert_eq!(g.objs.aliens[boss as usize].stratstate, 5);
    assert_eq!(
        g.objs.aliens[boss as usize].sbyte2, neg,
        "sbyte2 must be nega(Yanglexy); raw={raw}"
    );
    let (ex, ez) = nvecs_expect(neg, 20);
    assert_eq!(g.objs.aliens[boss as usize].vx, ex);
    assert_eq!(g.objs.aliens[boss as usize].vz, ez);
}

/// flyingfish .jumping: roty = nega(Yanglexy), then nvecs.
#[test]
fn flyingfish_jump_aims_with_negated_yanglexy() {
    let mut g = Game::new();
    spawn_player(&mut g, 400, 0, 0);
    let fish = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[fish as usize];
        al.worldx = -200; // already at chase target → .jumping
        al.worldy = 0;
        al.worldz = 2000;
        al.roty = 0;
    }
    flyingfish_init(&mut g, fish);
    // init adds deg180 to roty; reset after.
    g.objs.aliens[fish as usize].roty = 0;
    g.objs.aliens[fish as usize].worldx = -200;

    let s = g.objs.aliens[fish as usize].stratptr.expect("strat");
    g.call_strat(s, fish);

    let raw = yanglexy(-200, 2000, 400, 0);
    let neg = raw.wrapping_neg();
    assert_ne!(raw, neg);
    assert_eq!(
        g.objs.aliens[fish as usize].roty, neg,
        "flyingfish roty must be nega; raw={raw}"
    );
    let (ex, ez) = nvecs_expect(neg, 70);
    assert_eq!(g.objs.aliens[fish as usize].vx, ex);
    assert_eq!(g.objs.aliens[fish as usize].vz, ez);
    assert_eq!(g.objs.aliens[fish as usize].vy, -15 + 2); // set -15 then flying tick +2
}
