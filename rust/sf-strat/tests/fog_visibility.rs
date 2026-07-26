//! Exact KSTRATS `s_initfog` / `s_dofog` visibility behavior.

use sf_game::alien::ACF_COLLTYPE2;
use sf_game::game::Game;
use sf_game::vars::{HARD_AP, HARD_HP};
use sf_strat::enemies_ground::{houdai5f_istrat, pillar3f_istrat, tank0_istrat, tank1_goforward};
use sf_strat::enemy_a::{fog_strat, hard180yrfog_istrat, DEG180};

const SOURCE_SHAPE: u16 = 197;
const FOG_DISTANCE: i16 = 2000;
const PLAYER_Z: i16 = 0;

fn spawn_player(game: &mut Game) {
    let player = game.objs.alloc().expect("player slot");
    assert_eq!(player, 0);
    game.objs.aliens[player as usize].worldz = PLAYER_Z;
    game.vars.internal_playpt = player as i16;
}

fn spawn_object(game: &mut Game, shape: u16, worldz: i16) -> u16 {
    let object = game.objs.alloc().expect("object slot");
    let state = &mut game.objs.aliens[object as usize];
    state.shape = shape;
    state.worldz = worldz;
    object
}

#[test]
fn hard_fog_object_hides_at_the_inclusive_boundary_and_restores_inside() {
    let mut game = Game::new();
    spawn_player(&mut game);
    let object = spawn_object(&mut game, SOURCE_SHAPE, FOG_DISTANCE);

    hard180yrfog_istrat(&mut game, object);
    let state = game.objs.aliens[object as usize];
    assert_eq!(state.sword1 as u16, SOURCE_SHAPE);
    assert_eq!(state.roty, DEG180);
    assert_eq!([state.hp, state.ap], [HARD_HP, HARD_AP]);
    assert_eq!(
        state.collflags & ACF_COLLTYPE2,
        0,
        "KSTRATS fog scenery does not inherit GSTRATS enemy collision"
    );
    assert!(state.stratptr.is_some());

    game.vars.map.in_fog = 1;
    fog_strat(&mut game, object);
    assert_eq!(
        game.objs.aliens[object as usize].shape, 0,
        "the retail Zdistmore boundary is inclusive"
    );

    game.objs.aliens[object as usize].worldz = FOG_DISTANCE - 1;
    fog_strat(&mut game, object);
    assert_eq!(game.objs.aliens[object as usize].shape, SOURCE_SHAPE);
}

#[test]
fn clearing_the_map_fog_flag_leaves_the_current_shape_untouched() {
    let mut game = Game::new();
    spawn_player(&mut game);
    let object = spawn_object(&mut game, SOURCE_SHAPE, FOG_DISTANCE);
    hard180yrfog_istrat(&mut game, object);

    game.vars.map.in_fog = 1;
    fog_strat(&mut game, object);
    assert_eq!(game.objs.aliens[object as usize].shape, 0);

    game.vars.map.in_fog = 0;
    game.objs.aliens[object as usize].worldz = FOG_DISTANCE - 1;
    fog_strat(&mut game, object);
    assert_eq!(
        game.objs.aliens[object as usize].shape, 0,
        "the macro exits before restoring when fog is disabled"
    );

    game.vars.map.in_fog = 1;
    fog_strat(&mut game, object);
    assert_eq!(game.objs.aliens[object as usize].shape, SOURCE_SHAPE);
}

#[test]
fn tank_turret_and_pillar_use_the_same_retained_shape_cutoff() {
    let mut game = Game::new();
    spawn_player(&mut game);
    game.vars.map.in_fog = 1;
    game.vars.gameframe = 1;

    let tank = spawn_object(&mut game, SOURCE_SHAPE, FOG_DISTANCE + 500);
    tank0_istrat(&mut game, tank);
    assert_eq!(game.objs.aliens[tank as usize].sword1 as u16, SOURCE_SHAPE);
    tank1_goforward(&mut game, tank);
    assert_eq!(game.objs.aliens[tank as usize].shape, 0);

    let turret = spawn_object(&mut game, SOURCE_SHAPE + 1, FOG_DISTANCE + 500);
    houdai5f_istrat(&mut game, turret);
    assert_eq!(
        game.objs.aliens[turret as usize].sword1 as u16,
        SOURCE_SHAPE + 1
    );
    assert_eq!(game.objs.aliens[turret as usize].shape, 0);

    let pillar = spawn_object(&mut game, SOURCE_SHAPE + 2, FOG_DISTANCE + 500);
    pillar3f_istrat(&mut game, pillar);
    assert_eq!(
        game.objs.aliens[pillar as usize].sword1 as u16,
        SOURCE_SHAPE + 2
    );
    assert_eq!(game.objs.aliens[pillar as usize].shape, 0);
}
