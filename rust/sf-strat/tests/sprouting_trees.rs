//! Source-boundary tests for the Route 3 sprouting trees.
//!
//! Oracle: `tree1_istrat`, `tree2_istrat`, `tree3_istrat`, and the shared
//! tree branch of `sprouty` in `reference/ultrastarfox/SF/STRAT/DSTRATS.ASM`
//! lines 1970-2416. The assertions cover the linked stalk count, alternating
//! leaves, bent tree2 crown, terminal flower, and source mesh selections.

use sf_game::alien::{ASF2_COLLDISABLE, ASF_SHADOW, NUMBER_AL};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::enemies_ground;

const IS_TREE1: usize = 203;
const IS_TREE2: usize = 204;

const SH_PLAYER: u16 = 2;
const SH_STALK: u16 = 208;
const SH_STALK_BODY: u16 = 460;
const SH_FLOWER: u16 = 442;
const SH_LEAF: u16 = 444;

const TREE_LEAF_SIDE_FLAG: u8 = 0x40;
const TREE_HAS_LEAVES_FLAG: u8 = 0x80;
const TREE_BENT_CROWN_FLAG: u8 = 0x01;
const TREE_KIND_FLAG: u8 = 0x02;
const TREE_END_LINK: u16 = u16::MAX;
const INITIAL_GROW_FRAME: u8 = 2;
const ROOT_DROP: i16 = -40;
const HALF_TURN: u8 = 128;
const TREE2_PLAYER_ALIGNED_YAW: u8 = 224;
const TREE1_SETTLE_TICKS: usize = 80;
const TREE2_SETTLE_TICKS: usize = 100;
const TREE3_SETTLE_TICKS: usize = 120;
// The bloom head itself becomes the first crown body before the source's
// second `[1, 2]` height counter is applied to subsequent children.
const TREE2_MINIMUM_CROWN_BODIES: usize = 2;
const TREE2_MAXIMUM_CROWN_BODIES: usize = 3;
const TREE_DEPTH: i16 = 1000;
const ANIMATION_FRAME_MASK: u8 = 0x7f;
const TREE3_POST_ROOT_GENERATIONS: u8 = 254;
const FALL_SPEED: u8 = 30;
const FALL_INITIAL_Y_SPEED: i16 = -10;

fn spawn(g: &mut Game, x: i16, y: i16, z: i16, shape: u16) -> u16 {
    let idx = g.objs.alloc().expect("alien pool");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    let object = &mut g.objs.aliens[idx as usize];
    object.shape = shape;
    object.worldx = x;
    object.worldy = y;
    object.worldz = z;
    idx
}

fn setup() -> Game {
    let mut game = Game::new();
    game.vars.internal_playpt = 0;
    enemies_ground::register(&mut game.world);
    let player = spawn(&mut game, 0, 0, 0, SH_PLAYER);
    game.objs.aliens[player as usize].hp = 3;
    game
}

fn place_tree(game: &mut Game, row: usize) -> u16 {
    let root = spawn(game, 0, 0, TREE_DEPTH, SH_STALK);
    game.objs.aliens[root as usize].stratptr = game.world.istrats[row];
    root
}

fn count_shape(game: &Game, shape: u16) -> usize {
    (0..NUMBER_AL)
        .filter(|&idx| game.objs.aliens[idx].active && game.objs.aliens[idx].shape == shape)
        .count()
}

fn find_shape(game: &Game, shape: u16) -> Option<usize> {
    (0..NUMBER_AL).find(|&idx| game.objs.aliens[idx].active && game.objs.aliens[idx].shape == shape)
}

fn linked_shapes(game: &Game, root: u16) -> Vec<u16> {
    let mut shapes = Vec::new();
    let mut current = root;
    for _ in 0..NUMBER_AL {
        let object = game.objs.aliens[current as usize];
        shapes.push(object.shape);
        if object.ptr == 0 || object.ptr == TREE_END_LINK {
            break;
        }
        current = object.ptr - 1;
    }
    shapes
}

#[test]
fn tree1_grows_linked_stalks_alternating_leaves_and_a_flower() {
    let mut game = setup();
    let root = place_tree(&mut game, IS_TREE1);

    game.run_strategies();
    let initialized = game.objs.aliens[root as usize];
    let expected_stalks = initialized.sbyte1 as usize + 1;
    assert_eq!(initialized.worldy, ROOT_DROP);
    assert_eq!(
        initialized.animframe & ANIMATION_FRAME_MASK,
        INITIAL_GROW_FRAME
    );
    assert_ne!(initialized.sflags2 & TREE_HAS_LEAVES_FLAG, 0);
    assert_ne!(initialized.sflags3 & TREE_KIND_FLAG, 0);
    assert_ne!(initialized.sflags2 & TREE_LEAF_SIDE_FLAG, 0);

    for _ in 0..TREE1_SETTLE_TICKS {
        game.run_strategies();
    }

    assert_eq!(count_shape(&game, SH_STALK_BODY), expected_stalks);
    assert_eq!(count_shape(&game, SH_LEAF), expected_stalks);
    assert_eq!(count_shape(&game, SH_FLOWER), 1);

    let chain = linked_shapes(&game, root);
    assert_eq!(chain.len(), expected_stalks + 1);
    assert!(chain[..expected_stalks]
        .iter()
        .all(|&shape| shape == SH_STALK_BODY));
    assert_eq!(chain[expected_stalks], SH_FLOWER);

    let leaf_yaws: Vec<u8> = (0..NUMBER_AL)
        .filter_map(|idx| {
            let object = game.objs.aliens[idx];
            (object.active && object.shape == SH_LEAF).then_some(object.roty)
        })
        .collect();
    assert!(leaf_yaws.contains(&0));
    assert!(leaf_yaws.contains(&HALF_TURN));
}

#[test]
fn tree2_adds_bent_crown_without_leaves_then_blooms() {
    let mut game = setup();
    let root = place_tree(&mut game, IS_TREE2);

    game.run_strategies();
    let initialized = game.objs.aliens[root as usize];
    let first_stalk_count = initialized.sbyte1 as usize + 1;
    assert_ne!(initialized.sflags3 & TREE_BENT_CROWN_FLAG, 0);
    assert_eq!(initialized.roty, TREE2_PLAYER_ALIGNED_YAW);

    for _ in 0..TREE2_SETTLE_TICKS {
        game.run_strategies();
    }

    let body_count = count_shape(&game, SH_STALK_BODY);
    assert!(
        (first_stalk_count + TREE2_MINIMUM_CROWN_BODIES
            ..=first_stalk_count + TREE2_MAXIMUM_CROWN_BODIES)
            .contains(&body_count),
        "first stalk count {first_stalk_count}, mature body count {body_count}"
    );
    assert_eq!(count_shape(&game, SH_LEAF), 0);
    assert_eq!(count_shape(&game, SH_FLOWER), 1);
    assert!((0..NUMBER_AL).any(|idx| {
        let object = game.objs.aliens[idx];
        object.active && object.shape == SH_STALK_BODY && object.rotz != 0
    }));

    let flower_idx = find_shape(&game, SH_FLOWER).expect("terminal flower");
    let flower = game.objs.aliens[flower_idx];
    assert_eq!(flower.sflags3 & TREE_BENT_CROWN_FLAG, 0);
    assert_eq!(flower.sflags & ASF_SHADOW, 0);
}

#[test]
fn tree3_uses_bounds_terminated_chain_instead_of_blooming() {
    let mut game = setup();
    let root = spawn(&mut game, 0, 0, TREE_DEPTH, SH_STALK);
    enemies_ground::tree3_istrat(&mut game, root);
    assert_eq!(
        game.objs.aliens[root as usize].sbyte1,
        TREE3_POST_ROOT_GENERATIONS
    );

    for _ in 0..TREE3_SETTLE_TICKS {
        game.run_strategies();
    }

    assert_eq!(count_shape(&game, SH_FLOWER), 0);
    assert_eq!(count_shape(&game, SH_LEAF), 0);
    assert!(linked_shapes(&game, root).len() > 1);
    assert!((0..NUMBER_AL).any(|idx| {
        let object = game.objs.aliens[idx];
        object.active && object.shape == SH_STALK_BODY && object.ptr == TREE_END_LINK
    }));
}

#[test]
fn tree_explosion_turns_the_linked_stalk_and_flower_into_falling_objects() {
    let mut game = setup();
    let root = place_tree(&mut game, IS_TREE1);
    for _ in 0..TREE1_SETTLE_TICKS {
        game.run_strategies();
    }

    let linked_before = linked_shapes(&game, root).len();
    assert!(linked_before > 1);
    let explode = game.objs.aliens[root as usize]
        .expstratptr
        .expect("tree chain explosion strategy");
    game.objs.aldead = 0;
    game.call_strat(explode, root);

    assert_eq!(game.objs.aldead, 0);
    assert_eq!(game.objs.aliens[root as usize].hp, 0);
    assert_ne!(
        game.objs.aliens[root as usize].sflags2 & ASF2_COLLDISABLE,
        0
    );
    let falling: Vec<_> = (0..NUMBER_AL)
        .filter_map(|idx| {
            let object = game.objs.aliens[idx];
            (object.active
                && idx != root as usize
                && object.vel == FALL_SPEED
                && object.vy == FALL_INITIAL_Y_SPEED)
                .then_some(idx)
        })
        .collect();
    assert_eq!(falling.len(), linked_before - 1);
}
