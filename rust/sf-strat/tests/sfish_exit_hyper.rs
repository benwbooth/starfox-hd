//! ROM sfish + exit + openlr + hyperspace + pillar3f + torpedoa leaves.

use sf_game::alien::{ASF_COLLDISABLE, ASF_SHADOW};
use sf_game::Game;
use sf_strat::enemies_ground::{
    exit_istrat, exitcoll_istrat, openlr_istrat, openlr_strat, openlrcol_istrat, pillar3f_istrat,
    pillar3f_strat, pillar3ffall_strat, pillar3fstay_istrat, sfish_istrat, sfish_strat,
    torpedoa_init, torpedoa_strat,
};
use sf_strat::enemy_a::{
    hyper_istrat, hyperspace_istrat, hyperspaceout_istrat, hyperspaceout_strat, phitflash_istrat,
    ASF2_SFLAG1, DEG180,
};
use sf_strat::{common::sf_random, snes_trig::strat_roffs_roll};

const HYPER_SHAPE: u16 = 408;
const HYPER2_SHAPE: u16 = 470;
const HYPER3_SHAPE: u16 = 471;
const HYPER4_SHAPE: u16 = 472;
const HYPER_WORLD_DISTANCE: i16 = 4000;
const HYPER_RANDOM_CENTER: i16 = 256;
const HYPER_ROLL_Y_OFFSET: i8 = 50;
const HYPER_Z_STEP: i16 = -80;
const SPACE_VIEW_CENTER_Y: i16 = -60;

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

    let ho = spawn_obj(&mut g);
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

fn random_hyperspace_coordinate(vars: &mut sf_game::vars::GameVars) -> i16 {
    let low = sf_random(vars) as u8;
    let high = (sf_random(vars) as u8) & 1;
    i16::from_le_bytes([low, high]).wrapping_sub(HYPER_RANDOM_CENTER)
}

#[test]
fn hyperspace_initializer_emits_exact_screen_space_streak() {
    let mut g = Game::new();
    const PLAYER_Z: i16 = 125;
    spawn_player(&mut g, PLAYER_Z);
    let emitter = spawn_obj(&mut g);
    g.objs.aliens[emitter as usize].worldx = 1200;
    g.objs.aliens[emitter as usize].worldy = 900;
    g.vars.gameframe = 0;

    let mut expected_random = Game::new();
    expected_random.vars.rng = g.vars.rng;
    let random_x = random_hyperspace_coordinate(&mut expected_random.vars);
    let random_y = random_hyperspace_coordinate(&mut expected_random.vars);
    let roll = sf_random(&mut expected_random.vars) as u8;
    let (roll_x, roll_y, _) = strat_roffs_roll(roll, 0, HYPER_ROLL_Y_OFFSET, 0);

    hyperspace_istrat(&mut g, emitter);

    let streak = emitter + 1;
    let emitted = &g.objs.aliens[streak as usize];
    assert!(emitted.active);
    assert_eq!(emitted.shape, HYPER_SHAPE);
    assert_eq!(emitted.worldx, random_x.wrapping_add(roll_x));
    assert_eq!(
        emitted.worldy,
        random_y
            .wrapping_add(roll_y)
            .wrapping_add(SPACE_VIEW_CENTER_Y)
    );
    assert_eq!(emitted.worldz, PLAYER_Z + HYPER_WORLD_DISTANCE);
    assert_eq!(emitted.rotz, roll);
    assert_ne!(emitted.sflags & ASF_COLLDISABLE, 0);
    assert!(emitted.stratptr.is_some());
    assert_eq!(g.vars.rng, expected_random.vars.rng);
    assert_eq!(g.objs.aliens[emitter as usize].roty, DEG180);
    assert_eq!(
        g.objs.aliens[emitter as usize].worldz,
        PLAYER_Z + HYPER_WORLD_DISTANCE
    );

    hyper_istrat(&mut g, streak);
    assert_eq!(
        g.objs.aliens[streak as usize].worldz,
        PLAYER_Z + HYPER_WORLD_DISTANCE + HYPER_Z_STEP
    );
}

#[test]
fn hyperspace_out_initializer_falls_through_and_selects_all_four_phases() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.gameframe = 1;
    let emitter = spawn_obj(&mut g);

    hyperspaceout_istrat(&mut g, emitter);
    assert_eq!(g.objs.aliens[emitter as usize].sbyte1, 63);
    assert_eq!(g.objs.aliens[emitter as usize].sword1 as u16, HYPER_SHAPE);

    for (before, after, shape) in [
        (49, 48, HYPER_SHAPE),
        (48, 47, HYPER2_SHAPE),
        (32, 31, HYPER3_SHAPE),
        (16, 15, HYPER4_SHAPE),
    ] {
        g.objs.aliens[emitter as usize].sbyte1 = before;
        hyperspaceout_strat(&mut g, emitter);
        assert_eq!(g.objs.aliens[emitter as usize].sbyte1, after);
        assert_eq!(g.objs.aliens[emitter as usize].sword1 as u16, shape);
    }

    g.objs.aliens[emitter as usize].sbyte1 = 0;
    hyperspaceout_strat(&mut g, emitter);
    assert_eq!(g.objs.aldead, 1);
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
