//! Native Black Hole shape-shuffler checks derived from the original
//! `damyscr_istrat` catalog and `damyscr` path program.

use sf_game::alien::{ACF_COLLTYPE2, ASF_SHADOW};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_path::ids::PATH_ID_DAMYSCR;
use sf_strat::common::sf_random;
use sf_strat::damyscr::{DAMYSCR_SHAPES, STRATEGY_DAMYSCR};
use sf_strat::table;

const TEST_RANDOM_STATE: [u8; 4] = [31, 73, 149, 211];
const SHAPE_CHOICE_INDEX_MASK: u16 = 62;
/// RELTOPLAYER off, ALWAYSGENVECS off, trigger registration, HP, and AP.
const INITIAL_PATH_SETUP_BYTES: u16 = 12;

#[test]
fn registered_strategy_selects_exact_catalog_entry_and_joins_path_lane() {
    let mut expected_random = Game::new();
    expected_random.vars.rng = TEST_RANDOM_STATE;
    let draw = sf_random(&mut expected_random.vars);
    let choice = (draw & SHAPE_CHOICE_INDEX_MASK) as usize / 2;

    let mut game = Game::new();
    table::register_all(&mut game);
    game.vars.rng = TEST_RANDOM_STATE;

    let object = game.objs.alloc().expect("object slot");
    strat_init_obj_vars(&mut game.objs.aliens[object as usize]);
    let strategy = game
        .world
        .find_direct_strategy(STRATEGY_DAMYSCR)
        .expect("damyscr dispatch registration");

    game.call_strat(strategy, object);

    let path_start = sf_path::literals::get_catalog().offsets[PATH_ID_DAMYSCR as usize];
    let initialized = game.objs.aliens[object as usize];
    assert_eq!(initialized.shape, DAMYSCR_SHAPES[choice] as u16);
    assert_eq!(game.vars.rng, expected_random.vars.rng);
    assert_eq!(
        initialized.sword2 as u16,
        path_start + INITIAL_PATH_SETUP_BYTES
    );
    assert!(initialized.stratptr.is_some());
    assert!(initialized.collstratptr.is_some());
    assert!(initialized.expstratptr.is_some());
    assert_ne!(initialized.collflags & ACF_COLLTYPE2, 0);
    assert_ne!(initialized.sflags & ASF_SHADOW, 0);
    assert_eq!((initialized.hp, initialized.ap), (2, 6));
    assert_eq!((initialized.rotx, initialized.roty), (8, 248));

    let path_tick = initialized.stratptr.expect("path tick");
    game.call_strat(path_tick, object);

    let advanced = game.objs.aliens[object as usize];
    assert_eq!((advanced.hp, advanced.ap), (2, 6));
    assert_eq!(advanced.rotx, 16);
    assert_eq!(advanced.roty, 240);
}

#[test]
fn disguise_catalog_has_all_thirty_two_source_entries() {
    assert_eq!(DAMYSCR_SHAPES.len(), 32);
    let unique: std::collections::BTreeSet<u16> =
        DAMYSCR_SHAPES.iter().map(|shape| *shape as u16).collect();
    assert_eq!(unique.len(), DAMYSCR_SHAPES.len());
}
