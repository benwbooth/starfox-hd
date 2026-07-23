//! ROM bossBrob jump / land / farjump / kick / start (GB3STRAT.ASM).

use sf_game::alien::ASF_NOHITAFFECT;
use sf_game::Game;
use sf_strat::bossb::{
    bossbrobfarjump1_init, bossbrobfarjump1_strat, bossbrobfarjump2_strat, bossbrobfarland_init,
    bossbrobfarland_strat, bossbrobjump1_init, bossbrobjump1_strat, bossbrobjump2_init,
    bossbrobjump2_strat, bossbrobkick_init, bossbrobkick_strat, bossbrobland_init,
    bossbrobland_strat, bossbrobstart2_init, bossbrobstart_init, bossbrobstart_strat,
};

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
    // init falls through into strat once → animframe 12+1.
    assert_eq!(g.objs.aliens[idx as usize].animframe, 13);
    assert!(g.objs.aliens[idx as usize].sbyte1 < 8);
    // Drain crouch.
    g.objs.aliens[idx as usize].sbyte1 = 0;
    bossbrobjump1_strat(&mut g, idx);
    // jump2 launched with upward vy.
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
    assert_eq!(g.objs.aliens[idx as usize].worldy, -320);
    // Expire land → fire1.
    g.objs.aliens[idx as usize].sbyte1 = 0;
    bossbrobland_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].sbyte1 < 60); // fire1 armed
}

#[test]
fn farjump_sets_nohitaffect_and_lands_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldz = 500; // close to player
    bossbrobfarjump1_init(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_NOHITAFFECT, 0);
    g.objs.aliens[idx as usize].sbyte1 = 0;
    bossbrobfarjump1_strat(&mut g, idx);
    // Force past the ground gate into farland.
    g.objs.aliens[idx as usize].worldy = -300;
    g.objs.aliens[idx as usize].vy = 50;
    bossbrobfarjump2_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -320);
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
    let before = g.objs.active_indices().len();
    // Drive to mid-kick fire (sbyte1==10).
    while g.objs.aliens[idx as usize].sbyte1 > 10 {
        bossbrobkick_strat(&mut g, idx);
    }
    bossbrobkick_strat(&mut g, idx); // fire at 10
    assert!(g.objs.active_indices().len() >= before);
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
