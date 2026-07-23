//! Tick 168–169: bossA cup / boss8 / bossbrob / boss2 home / chick —
//! obj2obj movement-aim yaw `nega`.

use sf_game::alien::{ASF3_REALOBJ, ASF_NOHITAFFECT};
use sf_game::game::Game;
use sf_strat::bosses::{boss2top_strat, chick_istrat};
use sf_strat::enemy_a::boss_attach_child_to_mother;
use sf_strat::enemy_b::bossacupperl_istrat;

const DEG0: u8 = 0;

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

/// bossA cup GO: `s_obj2obj_3dangle` into sbyte3 stores nega(Yanglexy).
#[test]
fn bossacup_go_aims_with_negated_yanglexy() {
    let mut g = Game::new();
    spawn_player(&mut g, 400, -40, 0);
    let mother = spawn(&mut g);
    let turret = spawn(&mut g);
    let cup = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, turret, 1));
    assert!(boss_attach_child_to_mother(&mut g, mother, cup, 4));
    bossacupperl_istrat(&mut g, cup);

    // GO state, already past the $66/nohitaffect entry tick.
    g.objs.aliens[cup as usize].stratstate = 2; // BOSSA_CUP_STATE_GO
    g.objs.aliens[cup as usize].sflags &= !ASF_NOHITAFFECT;
    g.objs.aliens[cup as usize].sbyte3 = DEG0;
    g.objs.aliens[cup as usize].sbyte4 = DEG0;
    g.objs.aliens[cup as usize].worldx = 0;
    g.objs.aliens[cup as usize].worldz = 2000; // |dz|=2000 >= 1000 → aim band
    g.objs.aliens[cup as usize].vel = 45;

    let raw = yanglexy(0, 2000, 400, 0);
    let neg = raw.wrapping_neg();
    let expect = achase_step(DEG0, neg, 3);
    let wrong = achase_step(DEG0, raw, 3);
    assert_ne!(raw, neg);

    let s = g.objs.aliens[cup as usize].stratptr.expect("strat");
    g.call_strat(s, cup);

    let got = g.objs.aliens[cup as usize].sbyte3;
    assert_eq!(
        got, expect,
        "cup GO sbyte3 must chase nega; raw={raw} neg={neg}"
    );
    assert_ne!(got, wrong);
}

/// Manual half-step chase used by boss2_homelaser / flingboss_hmissile1
/// (arithmetic `i8 >> shift`, not `achase_angle`'s toward-zero).
fn halfstep_chase(cur: u8, target: u8, shift: u32) -> u8 {
    let dyaw = cur.wrapping_sub(target) as i8;
    cur.wrapping_sub((dyaw >> shift) as u8)
}

/// chick_istrat: `dobj2obj3dangle_xy` stores nega(Yanglexy), then dgen3dvecs.
#[test]
fn chick_istrat_stores_negated_yanglexy() {
    let mut g = Game::new();
    spawn_player(&mut g, 400, -40, 0);
    let idx = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 0;
        al.worldy = -40;
        al.worldz = 500;
    }
    chick_istrat(&mut g, idx);
    let raw = yanglexy(0, 500, 400, 0);
    let neg = raw.wrapping_neg();
    assert_ne!(raw, neg);
    assert_eq!(
        g.objs.aliens[idx as usize].roty, neg,
        "chick roty must be nega(Yanglexy); raw={raw}"
    );
    // Velocity from n3dvecs(neg) should pull toward -X (player at +X from chick
    // at origin with yaw = -atan2(+400,+500) → fly toward player).
    assert!(
        g.objs.aliens[idx as usize].vx != 0 || g.objs.aliens[idx as usize].vz != 0,
        "chick must have nonzero 3dvecs after aim"
    );
}

/// boss2top critical fire → relelaserhome tick chases nega(Yanglexy).
#[test]
fn boss2_homelaser_chases_negated_yanglexy() {
    let mut g = Game::new();
    spawn_player(&mut g, 400, -40, 0);
    let mother = spawn(&mut g);
    let top = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, top, 1));
    {
        let al = &mut g.objs.aliens[top as usize];
        al.worldx = 0;
        al.worldy = -40;
        al.worldz = 2000;
        al.hp = 8; // ≤16 → critical home-laser band
        al.roty = DEG0;
        al.rotx = DEG0;
    }
    {
        let m = &mut g.objs.aliens[mother as usize];
        m.worldx = 0;
        m.worldy = -40;
        m.worldz = 2000;
        m.sflags2 |= 0x40; // BOSS2_SFLAG3 — enable top collide (harmless here)
    }
    g.vars.gameframe = 0; // &7 == 0 → fire
    let before = g.objs.active_indices().len();
    boss2top_strat(&mut g, top);
    assert!(
        g.objs.active_indices().len() > before,
        "critical top must fire home laser"
    );

    // Find the shot with a stratptr (the home laser).
    let shot = g
        .objs
        .active_indices()
        .into_iter()
        .find(|&i| {
            i != 0 && i != mother && i != top && g.objs.aliens[i as usize].stratptr.is_some()
        })
        .expect("home laser shot");
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = 0;
        al.worldy = -40;
        al.worldz = 2000;
        al.roty = DEG0;
        al.rotx = DEG0;
    }
    let raw = yanglexy(0, 2000, 400, 0);
    let neg = raw.wrapping_neg();
    let expect = halfstep_chase(DEG0, neg, 3);
    let wrong = halfstep_chase(DEG0, raw, 3);
    assert_ne!(raw, neg);
    assert_ne!(expect, wrong);

    let s = g.objs.aliens[shot as usize].stratptr.expect("strat");
    g.call_strat(s, shot);
    let got = g.objs.aliens[shot as usize].roty;
    assert_eq!(
        got, expect,
        "homelaser roty must chase nega; raw={raw} neg={neg}"
    );
    assert_ne!(got, wrong);
}
