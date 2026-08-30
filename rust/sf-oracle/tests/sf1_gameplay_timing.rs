//! Independent retail reference points for typed Corneria timing.

#[path = "../examples/support/mod.rs"]
mod support;

use sf_game::gameplay_timing::{timing_for_update, GameplayTickTiming};
use sf_map::catalog::map_id;

const MESEN_NEUTRAL_REFERENCE: &[(u16, GameplayTickTiming)] = &[
    (
        0,
        GameplayTickTiming {
            motion_refreshes: 2,
            presentation_refreshes: 3,
        },
    ),
    (
        22,
        GameplayTickTiming {
            motion_refreshes: 7,
            presentation_refreshes: 6,
        },
    ),
    (
        186,
        GameplayTickTiming {
            motion_refreshes: 4,
            presentation_refreshes: 87,
        },
    ),
    (
        315,
        GameplayTickTiming {
            motion_refreshes: 4,
            presentation_refreshes: 4,
        },
    ),
    (
        320,
        GameplayTickTiming {
            motion_refreshes: 4,
            presentation_refreshes: 5,
        },
    ),
    (
        321,
        GameplayTickTiming {
            motion_refreshes: 5,
            presentation_refreshes: 5,
        },
    ),
    (
        322,
        GameplayTickTiming {
            motion_refreshes: 5,
            presentation_refreshes: 5,
        },
    ),
    (
        948,
        GameplayTickTiming {
            motion_refreshes: 3,
            presentation_refreshes: 3,
        },
    ),
    (
        982,
        GameplayTickTiming {
            motion_refreshes: 3,
            presentation_refreshes: 3,
        },
    ),
];

#[test]
fn typed_corneria_neutral_timing_matches_independent_mesen_reference_points() {
    for &(game_frame, expected) in MESEN_NEUTRAL_REFERENCE {
        assert_eq!(
            timing_for_update(map_id::M1_1, game_frame),
            expected,
            "Corneria timing at game frame {game_frame}",
        );
    }
}
