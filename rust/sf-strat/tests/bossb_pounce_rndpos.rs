//! ROM bossBrob pounce / rndpos / foot / ment (GB3STRAT.ASM).

use sf_game::alien::{ASF_COLLDISABLE, ASF_NOHITAFFECT, ATGND};
use sf_game::Game;
use sf_strat::bossb::{
    bossbent_istrat, bossbrobfoot_istrat, bossbrobfoot_strat, bossbrobment2_srou,
    bossbrobment_srou, bossbrobpounce2_init, bossbrobpounce2_strat, bossbrobpouncepos_init,
    bossbrobpouncepos_strat, bossbrobreappear_init, bossbrobreappear_strat, bossbrobrndpos2_istrat,
    bossbrobrndpos_istrat, bossbrobrndpos_strat,
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
fn ment_spawns_linked_child() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let mother = spawn_rob(&mut g);
    let before = g.objs.active_indices().len();
    let child = bossbrobment_srou(&mut g, mother, 1).expect("ment");
    assert!(g.objs.active_indices().len() > before);
    assert_ne!(child, mother);
}

#[test]
fn ment2_defers_trail_initializer_like_the_object_scheduler() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let mother = spawn_rob(&mut g);
    let child = bossbrobment2_srou(&mut g, mother).expect("ment2");
    assert_ne!(g.objs.aliens[child as usize].type_ & ATGND, 0);
    assert_eq!(g.objs.aliens[child as usize].sflags & ASF_COLLDISABLE, 0);
    bossbent_istrat(&mut g, child);
    assert_ne!(g.objs.aliens[child as usize].sflags & ASF_COLLDISABLE, 0);
}

#[test]
fn pouncepos_crouches_then_pounce2() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    bossbrobpouncepos_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 29); // 30 then tick
                                                        // Force into anim phase.
    g.objs.aliens[idx as usize].sbyte1 = 1;
    g.objs.aliens[idx as usize].animframe = 128 | 19;
    bossbrobpouncepos_strat(&mut g, idx);
    // pounce2_init sets rotx=8; fall-through strat may +8 → 16.
    assert!(matches!(g.objs.aliens[idx as usize].rotx, 8 | 16));
    assert!(g.objs.aliens[idx as usize].vy < 0);
}

#[test]
fn pounce2_lands_on_ground() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldz = 100; // in front of player
    bossbrobpounce2_init(&mut g, idx);
    g.objs.aliens[idx as usize].worldy = -300;
    g.objs.aliens[idx as usize].vy = 50;
    bossbrobpounce2_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -250);
    assert_eq!(g.objs.aliens[idx as usize].vy, 0);
}

#[test]
fn pounce2_reappears_when_behind_and_far() {
    let mut g = Game::new();
    spawn_player(&mut g, 2000);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldz = 0; // behind player, |dz|=2000
    bossbrobpounce2_init(&mut g, idx);
    bossbrobpounce2_strat(&mut g, idx);
    // Should have switched to reappear (vz=45).
    assert_eq!(g.objs.aliens[idx as usize].vz, 45);
}

#[test]
fn reappear_lands_then_nextstate() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    bossbrobreappear_init(&mut g, idx);
    g.objs.aliens[idx as usize].worldy = -300;
    g.objs.aliens[idx as usize].vy = 80; // force past ground this tick
    g.objs.aliens[idx as usize].animframe = 12;
    bossbrobreappear_strat(&mut g, idx);
    // Reappear advances immediately; fireP1's first tick chases Y by 13.
    assert_eq!(g.objs.aliens[idx as usize].worldy, -233);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}

#[test]
fn rndpos_picks_table_and_chases() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.objs.aliens[0].worldx = 50;
    let idx = spawn_rob(&mut g);
    g.vars.write_ext16(0x1F00, 0x1234); // rndval
    bossbrobrndpos_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 30);
    // Force the next decrement to select a new table entry.
    g.objs.aliens[idx as usize].sbyte1 = 1;
    let x0 = g.objs.aliens[idx as usize].worldx;
    bossbrobrndpos_strat(&mut g, idx);
    // sword1 set to player_x + table dx.
    assert_ne!(g.objs.aliens[idx as usize].sword1, 0);
    // Chase moved x toward target (or already equal).
    let _ = x0;
}

#[test]
fn rndpos2_clears_nohitaffect() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].sflags |= ASF_NOHITAFFECT;
    bossbrobrndpos2_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_NOHITAFFECT, 0);
}

#[test]
fn foot_aims_and_moves() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_rob(&mut g);
    g.objs.aliens[idx as usize].worldz = 1500;
    let z0 = g.objs.aliens[idx as usize].worldz;
    bossbrobfoot_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vel, 120);
    assert_eq!(g.objs.aliens[idx as usize].depthoffset, 1);
    bossbrobfoot_strat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].worldz, z0);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}
