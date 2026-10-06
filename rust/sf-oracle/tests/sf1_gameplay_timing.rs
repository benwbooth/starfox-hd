//! Independent retail reference points for typed Corneria timing.

#[path = "../examples/support/mod.rs"]
mod support;
#[path = "../examples/support/sf1_timing.rs"]
mod timing_entry;

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

#[test]
fn retail_counter_boundaries_match_independent_mesen_early_scenes() {
    let Some(rom) = sf_oracle::load_retail_rom() else {
        eprintln!("skip: retail Rev 2 ROM not found at repository root");
        return;
    };
    let mut retail = sf_oracle::RetailMachine::new(rom);
    timing_entry::enter_first_corneria_interval(&mut retail)
        .expect("source-driven Corneria handoff");
    assert_eq!(retail.peek8(0x7E_0000 | sf_oracle::RETAIL_FRAMERATE), 2);
    // Independently measured with Mesen 2.1.1 on 2026-10-06, neutral input,
    // at $02:DA7E after framerate copies framec. This tests the reference
    // runner itself, not the native game's recorded timing table. It does
    // not claim exact master-clock or later-scene agreement.
    for (index, refreshes) in [3, 3, 3, 4].into_iter().enumerate() {
        assert!(retail
            .tick_until_cpu_execution(0, timing_entry::FRAME_RATE_SAMPLE_COMPLETE, 12)
            .unwrap());
        assert_eq!(
            retail.peek16(0x7E_0000 | sf_oracle::RETAIL_GAMEFRAME),
            index as u16 + 1
        );
        assert_eq!(
            retail.peek8(0x7E_0000 | sf_oracle::RETAIL_FRAMERATE),
            refreshes
        );
        assert!(retail
            .tick_until_cpu_execution(0, timing_entry::FRAME_COUNTER_RESET_COMPLETE, 12)
            .unwrap());
    }
}

#[test]
fn source_bound_entry_preserves_the_first_corneria_player_state() {
    use sf_oracle::{
        RetailMachine, AL_VX, AL_VY, AL_VZ, RETAIL_FRAMERATE, RETAIL_GAMEFRAME, RETAIL_PLAYPT,
        RETAIL_POOL, RETAIL_PVIEWPOSZ, RETAIL_PVIEWVELZ,
    };
    let Some(rom) = sf_oracle::load_retail_rom() else {
        eprintln!("skip: retail Rev 2 ROM not found at repository root");
        return;
    };
    let mut retail = RetailMachine::new(rom);
    timing_entry::enter_first_corneria_update(&mut retail).expect("original first strategy entry");
    let mut native = support::configured_shell();
    timing_entry::enter_native_corneria_update(&mut native).expect("native first strategy entry");
    let word = |address| retail.peek16(0x7E_0000 | address);
    let player_base = u32::from(word(RETAIL_PLAYPT));
    let player = &native.game.objs.aliens[native.game.player_object().expect("player") as usize];
    assert_eq!(word(RETAIL_GAMEFRAME), 0);
    assert_eq!(native.game.vars.gameframe, 0);
    for (actual, address) in [
        (player.worldx, player_base + RETAIL_POOL.al_worldx),
        (player.worldy, player_base + RETAIL_POOL.al_worldy),
        (player.worldz, player_base + RETAIL_POOL.al_worldz),
        (player.vx, player_base + AL_VX),
        (player.vy, player_base + AL_VY),
        (player.vz, player_base + AL_VZ),
        (native.game.vars.pviewvelz, RETAIL_PVIEWVELZ),
        (
            native.game.vars.strategy.player_view_position[2],
            RETAIL_PVIEWPOSZ,
        ),
    ] {
        assert_eq!(
            actual,
            word(address) as i16,
            "first-entry source field {address:04X}"
        );
    }
    assert_eq!(
        native.game.vars.strategy.frame_rate,
        retail.peek8(0x7E_0000 | RETAIL_FRAMERATE),
    );
}
