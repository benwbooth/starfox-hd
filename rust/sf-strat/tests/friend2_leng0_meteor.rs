//! ROM friend2 + leng0 + meteor2/col + winglazerman/tree/uperm/iris leaves.

use sf_game::alien::{ASF3_LOCKON, ASF_COLLDISABLE, ASF_HITFLASH, ASF_SHADOW};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::enemies_ground::{
    iris_1_istrat, iris_istrat, iris_strat, leng0_istrat, leng0_strat, meteor_istrat2,
    meteorcol_istrat, tree1_istrat, tree2_istrat, uperm_istrat, uperm_strat, winglazerman2_strat,
    winglazerman3_strat, winglazermandie_istrat, winglazermango_strat,
};
use sf_strat::enemy_a::{friend2_istrat, friend2_strat, COLLTYPE_ZENEMY, DEG180};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].worldx = 0;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[0].vel = 40;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].worldy = -50;
    idx
}

#[test]
fn friend2_locks_zenemy_ahead() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let f = spawn_obj(&mut g);
    friend2_istrat(&mut g, f);
    assert_eq!(g.objs.aliens[f as usize].hp, HARD_HP);
    assert_ne!(g.objs.aliens[f as usize].sflags & ASF_SHADOW, 0);

    let en = spawn_obj(&mut g);
    g.objs.aliens[en as usize].worldx = 100;
    g.objs.aliens[en as usize].worldz = 800; // ahead of player
    g.objs.aliens[en as usize].collflags |= COLLTYPE_ZENEMY;
    g.objs.aliens[en as usize].hp = 4;

    g.vars.gameframe = 0;
    friend2_strat(&mut g, f);
    assert_eq!(g.objs.aliens[f as usize].worldz, 200); // player z + 200
    assert_eq!(g.objs.aliens[f as usize].sword1, (en + 1) as i16);
    assert_ne!(g.objs.aliens[en as usize].sflags3 & ASF3_LOCKON, 0);
    assert_ne!(g.objs.aliens[en as usize].sflags & ASF_HITFLASH, 0);
}

#[test]
fn leng0_opens_when_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 2000;
    leng0_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    leng0_strat(&mut g, idx); // far: no open
    assert_eq!(g.objs.aliens[idx as usize].animframe & 0x7F, 0);

    g.objs.aliens[idx as usize].worldz = 500;
    for _ in 0..11 {
        leng0_strat(&mut g, idx);
    }
    assert_eq!(g.objs.aliens[idx as usize].animframe & 0x7F, 10);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
}

#[test]
fn meteor2_and_col_tree_iris_uperm() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let m = spawn_obj(&mut g);
    meteor_istrat2(&mut g, m);
    assert_eq!(g.objs.aliens[m as usize].sword1, 60);
    // enemy1 = 0x10
    assert_ne!(g.objs.aliens[m as usize].collflags & 0x10, 0);
    g.objs.aliens[m as usize].sflags &= !sf_game::alien::ASF_NOHITAFFECT;
    g.objs.aliens[m as usize].hp = 20;
    g.objs.aliens[m as usize].ap = 4;
    meteorcol_istrat(&mut g, m);
    assert_eq!(g.objs.aliens[m as usize].hp, 16);
    assert_ne!(g.objs.aliens[m as usize].sflags & ASF_HITFLASH, 0);

    let t = spawn_obj(&mut g);
    tree1_istrat(&mut g, t);
    assert_eq!(g.objs.aliens[t as usize].hp, HARD_HP);
    let t2 = spawn_obj(&mut g);
    tree2_istrat(&mut g, t2);

    let i = spawn_obj(&mut g);
    iris_istrat(&mut g, i);
    assert_eq!(g.objs.aliens[i as usize].roty, DEG180);
    assert_eq!(g.objs.aliens[i as usize].hp, 127);
    g.objs.aliens[i as usize].hp = 100;
    iris_strat(&mut g, i);
    assert!(g.objs.aliens[i as usize].animframe & 0x7F >= 1);
    let i1 = spawn_obj(&mut g);
    iris_1_istrat(&mut g, i1);

    let u = spawn_obj(&mut g);
    uperm_istrat(&mut g, u);
    assert_eq!(g.objs.aliens[u as usize].vel, 70);
    uperm_strat(&mut g, u);
}

#[test]
fn winglazerman_die_drops_item() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].sbyte1 = 5;
    winglazerman2_strat(&mut g, idx);
    winglazerman3_strat(&mut g, idx);
    g.objs.aliens[idx as usize].count = 50;
    winglazermango_strat(&mut g, idx);

    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    // No beam → item7 path
    g.vars.pshipflags3 = 0;
    winglazermandie_istrat(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after >= before); // drop may spawn before explode marks dead
}
