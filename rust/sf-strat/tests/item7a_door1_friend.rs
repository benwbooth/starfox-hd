//! ROM item7a + door1 + woods + wireman leaves + friend0/1 + minumusi.

use sf_game::alien::{ASF_COLLDISABLE, ASF_SHADOW};
use sf_game::coldet::PCBOX_WING_HP;
use sf_game::Game;
use sf_strat::enemies_ground::{
    door1_istrat, door1closewait_init, door1closewait_strat, door1openwait_strat, minumusi_istrat,
    wireman2x_strat, wireman2yl_strat, wireman2yr_strat, wiremandie_istrat, wiremanup_init,
    woods_istrat, woods_strat, woodsgo_strat,
};
use sf_strat::enemy_a::{
    friend02_strat, friend0_istrat, friend0_strat, friend1_istrat, friend1_strat,
    friendkill_istrat, helpball_istrat, item7a_istrat, item7a_strat, ASF2_SFLAG3,
    COLLTYPE_ENEMYWEAP, DEG180,
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
fn item7a_spawns_helpball() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 50;
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -40;
    item7a_istrat(&mut g, idx);
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    item7a_strat(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after > before, "helpball spawned");
    assert_eq!(g.objs.aliens[idx as usize].count, 20); // flashplayer
                                                       // helpball should have been inited on a new slot
    let _ = helpball_istrat;
}

#[test]
fn item7a_restores_source_wing_health() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let left_wing = spawn_obj(&mut g);
    let right_wing = spawn_obj(&mut g);
    g.coldet.pcbox.lwing = Some(left_wing);
    g.coldet.pcbox.rwing = Some(right_wing);
    g.objs.aliens[left_wing as usize].hp = 0;
    g.objs.aliens[right_wing as usize].hp = 0;

    let pickup = spawn_obj(&mut g);
    g.objs.aliens[pickup as usize].worldx = 0;
    g.objs.aliens[pickup as usize].worldy = -40;
    g.objs.aliens[pickup as usize].worldz = 50;
    item7a_istrat(&mut g, pickup);
    item7a_strat(&mut g, pickup);

    assert_eq!(g.objs.aliens[left_wing as usize].hp, PCBOX_WING_HP);
    assert_eq!(g.objs.aliens[right_wing as usize].hp, PCBOX_WING_HP);
}

#[test]
fn door1_closes_when_player_near() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 2000;
    door1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].ap, 8);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG180);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    // Far: stay open (anim may decrement).
    door1openwait_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    // Close when player within 500z.
    g.objs.aliens[idx as usize].worldz = 100;
    door1openwait_strat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    // Animate closed frames.
    door1closewait_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].animframe > 0);
    // Far + no movers → reopen.
    g.objs.aliens[idx as usize].worldz = 2000;
    door1closewait_init(&mut g, idx);
    g.objs.aliens[idx as usize].animframe = 9;
    door1closewait_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
}

#[test]
fn woods_converts_to_woodsgo() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 500;
    woods_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 10); // converted same tick
    let _ = woods_strat;
    let _ = woodsgo_strat;
}

#[test]
fn friend0_1_and_wireman_leaves() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let f0 = spawn_obj(&mut g);
    friend0_istrat(&mut g, f0);
    assert_eq!(g.objs.aliens[f0 as usize].hp, 4);
    assert_eq!(g.objs.aliens[f0 as usize].vel, 60);
    assert_ne!(g.objs.aliens[f0 as usize].collflags & COLLTYPE_ENEMYWEAP, 0);
    // Brake toward 30.
    friend0_strat(&mut g, f0);
    assert_eq!(g.objs.aliens[f0 as usize].vel, 59);
    // Force friend02 hunt path.
    g.objs.aliens[f0 as usize].vel = 30;
    friend0_strat(&mut g, f0);
    friend02_strat(&mut g, f0);

    let f1 = spawn_obj(&mut g);
    friend1_istrat(&mut g, f1);
    assert_eq!(g.objs.aliens[f1 as usize].hp, 8);
    assert_ne!(g.objs.aliens[f1 as usize].sflags & ASF_SHADOW, 0);
    g.objs.aliens[0].sflags2 |= ASF2_SFLAG3;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    friend1_strat(&mut g, f1);
    assert_eq!(g.objs.aliens[f1 as usize].worldz, 200); // player z + 200
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after >= before); // may fire elaser

    friendkill_istrat(&mut g, f1);

    let w = spawn_obj(&mut g);
    g.objs.aliens[w as usize].sbyte1 = 5;
    wireman2x_strat(&mut g, w);
    wireman2yr_strat(&mut g, w);
    wireman2yl_strat(&mut g, w);
    wiremanup_init(&mut g, w);
    assert_eq!(g.objs.aliens[w as usize].worldy, 0);

    wiremandie_istrat(&mut g, w);
    minumusi_istrat(&mut g, w);
}
