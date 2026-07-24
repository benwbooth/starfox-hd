//! ROM bossBrob jump / land / farjump / kick / start (GB3STRAT.ASM).

use sf_game::alien::ASF_NOHITAFFECT;
use sf_game::Game;
use sf_strat::bossb::{
    bossbrobfarjump1_init, bossbrobfarjump1_strat, bossbrobfarjump2_strat, bossbrobfarland_init,
    bossbrobfarland_strat, bossbrobjump1_init, bossbrobjump1_strat, bossbrobjump2_init,
    bossbrobjump2_strat, bossbrobkick_init, bossbrobkick_strat, bossbrobland_init,
    bossbrobland_strat, bossbrobstart2_init, bossbrobstart_init, bossbrobstart_strat,
};
use sf_strat::common::strat_gen_vecs_nvecs;

const ANDROSS_KICK_BODY_SHAPE: u16 = 468;
const ANDROSS_KICK_FOOT_SHAPE: u16 = 469;
const ANDROSS_WALKING_BODY_SHAPE: u16 = 75;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
}

fn spawn_rob(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("rob");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldy = -400;
    g.objs.aliens[idx as usize].worldz = 2500;
    idx
}

#[test]
fn start_falls_then_enters_attack() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    bossbrobstart_init(&mut g, idx);
    // Force onto ground.
    g.objs.aliens[idx as usize].worldy = -100; // above ground (-320)
    for _ in 0..80 {
        bossbrobstart_strat(&mut g, idx);
        if g.objs.aliens[idx as usize].worldy >= -320 {
            break;
        }
    }
    // After landing, nextstate → fireP1 (stratptr set).
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    assert_eq!(g.objs.aliens[idx as usize].worldy, -320);
}

#[test]
fn start_integrates_once_and_keeps_the_authored_bounce() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    {
        let robot = &mut g.objs.aliens[idx as usize];
        robot.worldx = 100;
        robot.worldy = -400;
        robot.worldz = 2500;
        robot.vx = 7;
        robot.vy = 0;
        robot.vz = 9;
    }
    bossbrobstart_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 107);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -398);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 2509);
    assert_eq!(g.objs.aliens[idx as usize].vy, 2);

    {
        let robot = &mut g.objs.aliens[idx as usize];
        robot.worldy = -320;
        robot.vx = 0;
        robot.vy = 40;
        robot.vz = 0;
    }
    bossbrobstart_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vy, -11);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -331);
}

#[test]
fn start2_hands_off_to_fire1() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    bossbrobstart2_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 59); // 60 then fire1 tick dec
    assert_eq!(
        g.objs.aliens[idx as usize].sbyte2,
        (0i8.wrapping_sub(32)) as u8
    );
}

#[test]
fn jump1_crouch_then_jump2_launches() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldy = -500; // well above ground
    bossbrobjump1_init(&mut g, idx);
    // The initializer falls through once: active marker + authored frame 13.
    assert_eq!(g.objs.aliens[idx as usize].animframe, 128 | 13);
    for _ in 0..6 {
        bossbrobjump1_strat(&mut g, idx);
    }
    assert_eq!(g.objs.aliens[idx as usize].animframe, 128 | 19);
    assert_eq!(g.objs.aliens[idx as usize].vel, 0);
    bossbrobjump1_strat(&mut g, idx);
    // Advancing beyond frame 19 clamps and launches on that exact tick.
    assert_eq!(g.objs.aliens[idx as usize].vel, 100);
    assert!(
        g.objs.aliens[idx as usize].vy < 0,
        "launched upward, vy={}",
        g.objs.aliens[idx as usize].vy
    );
}

#[test]
fn jump2_lands_into_land_then_fire1() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldy = -400;
    bossbrobjump2_init(&mut g, idx);
    // Force below ground gate.
    g.objs.aliens[idx as usize].worldy = -300;
    g.objs.aliens[idx as usize].vy = 50;
    bossbrobjump2_strat(&mut g, idx);
    // land_init set sbyte1=10 (then land_strat may have dec'd).
    assert!(g.objs.aliens[idx as usize].sbyte1 <= 10);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -250);
    // Expire land → fire1.
    g.objs.aliens[idx as usize].sbyte1 = 0;
    bossbrobland_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].sbyte1 < 60); // fire1 armed
}

#[test]
fn jump_launch_uses_yaw_only_for_horizontal_velocity() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    {
        let robot = &mut g.objs.aliens[idx as usize];
        robot.worldy = -600;
        robot.roty = 37;
        robot.rotx = 64;
        robot.vel = 100;
    }
    let mut expected = g.objs.aliens[idx as usize];
    strat_gen_vecs_nvecs(&mut expected);

    bossbrobjump2_init(&mut g, idx);

    assert_eq!(g.objs.aliens[idx as usize].vx, expected.vx);
    assert_eq!(g.objs.aliens[idx as usize].vz, expected.vz);
}

#[test]
fn farjump_sets_nohitaffect_and_lands_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldz = 500; // close to player
    bossbrobfarjump1_init(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].shape,
        ANDROSS_WALKING_BODY_SHAPE
    );
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_NOHITAFFECT, 0);
    g.objs.aliens[idx as usize].sbyte1 = 0;
    bossbrobfarjump1_strat(&mut g, idx);
    // Force past the ground gate into farland.
    g.objs.aliens[idx as usize].worldy = -300;
    g.objs.aliens[idx as usize].vy = 50;
    bossbrobfarjump2_strat(&mut g, idx);
    // The landing state immediately advances to fireP1, whose first tick
    // chases the body nine units back toward the authored ground height.
    assert_eq!(g.objs.aliens[idx as usize].worldy, -259);
    // Close Z → farland_end clears nohitaffect + nextstate.
    bossbrobfarland_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_NOHITAFFECT, 0);
}

#[test]
fn kick_fires_midway_then_advances() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldz = 1500;
    bossbrobkick_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].shape, ANDROSS_KICK_BODY_SHAPE);
    let before = g.objs.active_indices().len();
    while g.objs.aliens[idx as usize].animframe & 127 < 14 {
        bossbrobkick_strat(&mut g, idx);
    }
    assert_eq!(g.objs.active_indices().len(), before);
    bossbrobkick_strat(&mut g, idx);
    let foot = g
        .objs
        .active_indices()
        .into_iter()
        .find(|slot| *slot != 0 && *slot != idx)
        .expect("detached kick foot");
    assert_eq!(g.objs.aliens[foot as usize].shape, ANDROSS_KICK_FOOT_SHAPE);
    // Expire.
    g.objs.aliens[idx as usize].sbyte1 = 0;
    bossbrobkick_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}

#[test]
fn land_init_zeros_vecs() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].vx = 10;
    g.objs.aliens[idx as usize].vy = 20;
    g.objs.aliens[idx as usize].vz = 30;
    bossbrobland_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vx, 0);
    assert_eq!(g.objs.aliens[idx as usize].vy, 0);
    assert_eq!(g.objs.aliens[idx as usize].vz, 0);
}

#[test]
fn farland_init_keeps_nohitaffect_until_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldz = 5000; // far
    bossbrobfarland_init(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_NOHITAFFECT, 0);
    bossbrobfarland_strat(&mut g, idx);
    // Still far → still nohitaffect.
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_NOHITAFFECT, 0);
}
