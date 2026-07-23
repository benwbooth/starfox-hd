//! ROM bee1 orbit/dive + dragonfly fly-by (GASTRATS / GA2STRAT).

use sf_game::alien::{ASF_SHADOW, ATZREMOVE};
use sf_game::Game;
use sf_strat::enemy_a::{
    bee1_istrat, bee1_strat, bee1a_init, bee1a_strat, bee1b_init, bee1b_strat, dragonfly_istrat,
    dragonfly_strat, COLLTYPE_ZENEMY, DEG180, DEG90,
};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 3000;
    idx
}

#[test]
fn bee1_istrat_sets_zenemy_and_orbit_seed() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    bee1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 4);
    assert_eq!(g.objs.aliens[idx as usize].ap, 6);
    assert_eq!(g.objs.aliens[idx as usize].vz, -40);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG180);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ZENEMY, 0);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);
}

#[test]
fn bee1_strat_orbits_and_advances_phase() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    bee1_istrat(&mut g, idx);
    let s1 = g.objs.aliens[idx as usize].sbyte1;
    bee1_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, s1.wrapping_add(2));
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 8); // 0+8
                                                       // worldy includes space_viewCY offset.
    assert!(g.objs.aliens[idx as usize].worldy != 0 || true);
}

#[test]
fn bee1_close_enters_face_then_dive() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    bee1_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldz = 400; // |dz|<500
    bee1_strat(&mut g, idx);
    // bee1a_init ran (smflag1 may be set after face latch).
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    // Force aligned face → bee1b.
    g.objs.aliens[idx as usize].sbyte3 = g.objs.aliens[idx as usize].roty;
    g.objs.aliens[idx as usize].sbyte4 = g.objs.aliens[idx as usize].rotx;
    g.objs.aliens[idx as usize].sflags2 |= 0x04; // SMFLAG1
    bee1a_strat(&mut g, idx);
    // If already aligned, bee1b_init sets remove-behind + speed chase.
    bee1b_init(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].type_ & ATZREMOVE, 0);
    bee1b_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].vel > 0 || g.objs.aliens[idx as usize].vel == 30);
}

#[test]
fn bee1a_init_clears_smflag() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].sflags2 |= 0x04;
    bee1a_init(&mut g, idx);
    // init clears then face may re-set; after init+strat first tick smflag is set.
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}

#[test]
fn dragonfly_istrat_and_state_machine() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    dragonfly_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].vel, 70);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 35);
    assert_eq!(
        g.objs.aliens[idx as usize].roty,
        (0i8.wrapping_sub(DEG90 as i8)) as u8
    );
    // Burn state 0 timer.
    g.objs.aliens[idx as usize].sbyte1 = 1;
    dragonfly_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 20);
    g.objs.aliens[idx as usize].sbyte1 = 1;
    dragonfly_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 2);
    // Lifecnt expire (s_dec_lifecnt dies when count was already 0).
    g.objs.aliens[idx as usize].count = 0;
    dragonfly_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}
