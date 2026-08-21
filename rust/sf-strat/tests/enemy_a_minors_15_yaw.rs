//! Tick 165: AUDIT_ENEMY_A Minor #15 — obj2obj yaw `nega(Yanglexy)` for
//! movement aim (zaco1_phase2 / strat_aim_* / para2 latch). Weapon fire keeps
//! raw `angle_xz` (fire_weapon Yanglexabs has no nega).

use sf_game::alien::ASF3_REALOBJ;
use sf_game::game::Game;
use sf_strat::enemy_a::{strat_para_init, strat_zaco1l_init, ASF2_SMFLAG1, DEG0};

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

fn run(g: &mut Game, idx: u16) {
    let s = g.objs.aliens[idx as usize].stratptr.expect("strat");
    g.call_strat(s, idx);
}

fn yanglexy(src_x: i16, src_z: i16, dst_x: i16, dst_z: i16) -> u8 {
    sf_core::aim_angle::yanglexy(dst_x.wrapping_sub(src_x), dst_z.wrapping_sub(src_z))
}

/// One Achase(shift=3) step from `cur` toward `target` (enemy_a::achase_angle).
fn achase_step(cur: u8, target: u8) -> u8 {
    if cur == target {
        return cur;
    }
    let diff = (target.wrapping_sub(cur) as i8) as i32;
    let mut step = if diff >= 0 {
        diff >> 3
    } else {
        -((-diff) >> 3)
    };
    if step == 0 {
        step = if diff > 0 { 1 } else { -1 };
    }
    cur.wrapping_add(step as u8)
}

/// zaco1_phase2 |dz|>=700 aims with nega(Yanglexy), matching phase0 / ROM.
#[test]
fn zaco1_phase2_aims_with_negated_yanglexy() {
    let mut g = Game::new();
    spawn_player(&mut g, 500, -40, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldz = 1000;
    g.objs.aliens[idx as usize].roty = 128; // avoid phase1→2 cascade on init
    strat_zaco1l_init(&mut g, idx);
    // Enter phase2: snap yaw to deg0 and tick (phase1 Achase reaches).
    g.objs.aliens[idx as usize].roty = DEG0;
    g.objs.aliens[idx as usize].worldz = 800; // |dz|=800 → aim band, not circ-only
    run(&mut g, idx); // phase1→2 fall-through + phase2 body

    // Reset yaw and run one pure phase2 aim step.
    g.objs.aliens[idx as usize].roty = DEG0;
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldz = 800;
    g.objs.aliens[0].worldx = 500;
    g.objs.aliens[0].worldz = 0;
    let raw = yanglexy(0, 800, 500, 0);
    let neg = raw.wrapping_neg();
    let expect = achase_step(DEG0, neg);
    let wrong = achase_step(DEG0, raw);
    run(&mut g, idx);
    let got = g.objs.aliens[idx as usize].roty;
    assert_ne!(neg, raw, "geometry must yield a non-self-inverse angle");
    assert_eq!(
        got, expect,
        "must Achase toward nega(Yanglexy)={neg}, not raw={raw}"
    );
    assert_ne!(got, wrong, "must not chase the non-negated Yanglexy");
}

/// para2 initface latch stores nega(Yanglexy).
#[test]
fn para2_latches_negated_yaw() {
    let mut g = Game::new();
    spawn_player(&mut g, 400, -40, 500);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -5;
    g.objs.aliens[idx as usize].worldz = 500;
    strat_para_init(&mut g, idx);
    g.objs.aliens[idx as usize].worldy = 0;
    run(&mut g, idx); // → para2, no body
    assert_eq!(g.objs.aliens[idx as usize].sflags2 & ASF2_SMFLAG1, 0);

    // Snapshot geometry the latch tick will see.
    let raw = yanglexy(
        g.objs.aliens[idx as usize].worldx,
        g.objs.aliens[idx as usize].worldz,
        g.objs.aliens[0].worldx,
        g.objs.aliens[0].worldz,
    );
    let neg = raw.wrapping_neg();
    run(&mut g, idx); // latch
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_SMFLAG1, 0);
    assert_eq!(
        g.objs.aliens[idx as usize].sbyte3, neg,
        "para2 latch must be nega(Yanglexy); raw={raw}"
    );
}
