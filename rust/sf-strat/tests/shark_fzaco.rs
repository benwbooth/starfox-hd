//! ROM shark mine-drop + fzaco friend-zaco + hardenemy1/hard90yrfog + zaco3_strat.

use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::enemy_a::{
    fzaco2_init, fzaco2_strat, fzaco3_init, fzaco3_strat, fzaco_cont, fzaco_cont2, fzaco_istrat,
    fzaco_strat, hard90yrfog_istrat, hardenemy1_istrat, shark_cont, shark_cont2, shark_istrat,
    shark_strat, sharka_init, sharka_strat, zaco3_istrat, zaco3_strat, COLLTYPE_ENEMY1,
    COLLTYPE_ENEMYWEAP, COLLTYPE_ZENEMY, DEG180, DEG22, DEG5,
};

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
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].worldy = -100;
    idx
}

#[test]
fn shark_istrat_arms_and_animates() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    shark_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 4);
    assert_eq!(g.objs.aliens[idx as usize].ap, 6);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ZENEMY, 0);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1, 0);
    assert_ne!(
        g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMYWEAP,
        0
    );
    assert_eq!(g.objs.aliens[idx as usize].ptr, 0);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 0);
    // Far from player → cont2 path; notdelay 1 advances anim.
    g.vars.gameframe = 0; // frame_tick_mod(1) true
    shark_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 1);
}

#[test]
fn shark_enters_a_when_close_and_drops_mines() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    shark_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldz = 500; // |dz|<1100
    g.objs.aliens[idx as usize].vel = 10;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    sharka_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 29); // 30 then dec in first tick
                                                        // Force fire gate open (notdelay 2,al1pt) and tick again.
    g.vars.gameframe = 0;
    sharka_strat(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    // Climb advanced and/or a mine0 was spawned.
    assert!(g.objs.aliens[idx as usize].sbyte1 < 29 || after > before);
}

#[test]
fn shark_cont_scrolls_with_player_z() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].vx = 0;
    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].vz = 0;
    g.vars.pviewvelz = 12;
    let z0 = g.objs.aliens[idx as usize].worldz;
    shark_cont(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, z0.wrapping_add(12));
    // cont2 with far target aims.
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].roty = 0;
    shark_cont2(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].roty, 0);
}

#[test]
fn fzaco_istrat_offsets_and_brakes() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.player_posx = 100;
    g.vars.player_posy = -50;
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldx = 10;
    g.objs.aliens[idx as usize].worldy = -20;
    fzaco_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 4);
    assert_eq!(g.objs.aliens[idx as usize].ap, 8);
    assert_eq!(g.objs.aliens[idx as usize].vel, 50);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG22);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 0u8.wrapping_sub(DEG5));
    assert_eq!(g.objs.aliens[idx as usize].worldx, 110);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -70);
    assert_ne!(
        g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMYWEAP,
        0
    );
    let v0 = g.objs.aliens[idx as usize].vel;
    fzaco_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].vel < v0);
}

#[test]
fn fzaco2_to_fzaco3_and_orbit() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    fzaco_istrat(&mut g, idx);
    fzaco2_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 59); // 60 then first-tick dec
                                                        // Force sbyte2==0 path.
    g.objs.aliens[idx as usize].sbyte2 = 0;
    g.objs.aliens[idx as usize].roty = DEG180; // facing neg-Z
    g.vars.gameframe = 0;
    fzaco2_strat(&mut g, idx);
    // Should have entered fzaco3 (remove-behind set).
    assert_ne!(
        g.objs.aliens[idx as usize].type_ & sf_game::alien::ATZREMOVE,
        0
    );
    let s1 = g.objs.aliens[idx as usize].sbyte1;
    fzaco_cont(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, s1.wrapping_add(6));
    fzaco3_init(&mut g, idx);
    let _ = fzaco3_strat(&mut g, idx);
    fzaco_cont2(&mut g, idx);
}

#[test]
fn hardenemy1_and_hard90yrfog() {
    let mut g = Game::new();
    let idx = spawn_obj(&mut g);
    hardenemy1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, HARD_HP);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1, 0);
    assert!(g.objs.aliens[idx as usize].stratptr.is_none());

    let idx2 = spawn_obj(&mut g);
    hard90yrfog_istrat(&mut g, idx2);
    assert_eq!(g.objs.aliens[idx2 as usize].roty, DEG180);
    assert_eq!(g.objs.aliens[idx2 as usize].hp, HARD_HP);
    assert!(g.objs.aliens[idx2 as usize].stratptr.is_some());
}

#[test]
fn zaco3_strat_alias_is_public() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    // Need a houdai_0 shape nearby for zaco3_istrat — if none, stratptr cleared.
    let idx = spawn_obj(&mut g);
    zaco3_istrat(&mut g, idx);
    // Alias callable without panic even if init failed to find target.
    zaco3_strat(&mut g, idx);
}
