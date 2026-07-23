//! ROM sfish + exit + openlr + hyperspace + pillar3f + torpedoa leaves.

use sf_game::alien::{ASF_COLLDISABLE, ASF_SHADOW};
use sf_game::Game;
use sf_strat::enemies_ground::{
    exit_istrat, exitcoll_istrat, openlr_istrat, openlr_strat, openlrcol_istrat, pillar3f_istrat,
    pillar3f_strat, pillar3ffall_strat, pillar3fstay_istrat, sfish_istrat, sfish_strat,
    torpedoa_init, torpedoa_strat,
};
use sf_strat::enemy_a::{
    hyper_istrat, hyperspace_istrat, hyperspace_strat, hyperspaceout_istrat, hyperspaceout_strat,
    phitflash_istrat, ASF2_SFLAG1, DEG180,
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
fn sfish_alone_swims_and_bounces() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].ptr = 0; // alone
    sfish_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 100);
    assert_eq!(g.objs.aliens[idx as usize].vx, 20);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 200);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    let x0 = g.objs.aliens[idx as usize].worldx;
    sfish_strat(&mut g, idx);
    // Moved by vx
    assert_ne!(g.objs.aliens[idx as usize].worldx, x0);

    // Mother path: attach pointer and orbit.
    let mom = spawn_obj(&mut g);
    let kid = spawn_obj(&mut g);
    g.objs.aliens[kid as usize].ptr = mom + 1;
    sfish_istrat(&mut g, kid);
    assert_ne!(g.objs.aliens[kid as usize].vx, 20); // random offset path
    sfish_strat(&mut g, kid);
}

#[test]
fn exit_openlr_hyperspace_pillar() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let e = spawn_obj(&mut g);
    exit_istrat(&mut g, e);
    assert_ne!(g.objs.aliens[e as usize].sflags & ASF_COLLDISABLE, 0);
    assert!(g.objs.aliens[e as usize].stratptr.is_none());
    exitcoll_istrat(&mut g, e);
    assert_eq!(g.objs.aldead, 1);
    g.objs.aldead = 0;

    let o = spawn_obj(&mut g);
    openlr_istrat(&mut g, o);
    openlr_strat(&mut g, o);
    assert_eq!(g.objs.aliens[o as usize].animframe & 0x7F, 0);
    openlrcol_istrat(&mut g, o);
    assert_ne!(g.objs.aliens[o as usize].sflags2 & ASF2_SFLAG1, 0);
    openlr_strat(&mut g, o);
    assert_ne!(g.objs.aliens[o as usize].sflags & ASF_COLLDISABLE, 0);
    assert!(g.objs.aliens[o as usize].animframe & 0x7F >= 1);

    let h = spawn_obj(&mut g);
    hyperspace_istrat(&mut g, h);
    g.vars.gameframe = 0;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    hyperspace_strat(&mut g, h);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after >= before);
    assert_eq!(g.objs.aliens[h as usize].roty, DEG180);

    let ho = spawn_obj(&mut g);
    hyperspaceout_istrat(&mut g, ho);
    assert_eq!(g.objs.aliens[ho as usize].sbyte1, 64);
    hyperspaceout_strat(&mut g, ho);
    assert_eq!(g.objs.aliens[ho as usize].sbyte1, 63);

    phitflash_istrat(&mut g, ho);
    hyper_istrat(&mut g, ho);

    let p = spawn_obj(&mut g);
    pillar3f_istrat(&mut g, p);
    assert_eq!(g.objs.aliens[p as usize].hp, 8);
    g.objs.aliens[p as usize].worldz = 100; // close → fall
    pillar3f_strat(&mut g, p);
    assert_ne!(g.objs.aliens[p as usize].sflags & ASF_SHADOW, 0);
    g.objs.aliens[p as usize].sbyte2 = 1;
    pillar3ffall_strat(&mut g, p);
    // stay
    pillar3fstay_istrat(&mut g, p);
    assert_eq!(g.objs.aliens[p as usize].sflags & ASF_SHADOW, 0);
}

#[test]
fn torpedoa_surfaces() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
    torpedoa_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    torpedoa_strat(&mut g, idx);
}
