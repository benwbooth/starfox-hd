//! Opening integration oracles. The synthetic actor-pool test calls the
//! controller separately, outside the pool; it does not reproduce boot's slot
//! budget or prove controller dispatch order. The boot test checks that boundary.
//! No child strategy is disabled or manually scheduled.
//! Native root/controller assertions are deliberately narrower than complete
//! native scene parity; this harness is the integration oracle for that work.

use std::collections::HashSet;

use sf2_game::intro_camera::OpeningCameraCue;
use sf2_game::intro_controller::{
    IntroColor, OpeningSceneController, OpeningScenePalette, INTRO_PALETTE_COLORS,
};
use sf2_game::intro_root::OpeningSceneRoot;
use sf2_game::object::{
    active_objects, allocate, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY, FREE_LIST,
    PLAYER_ONE, SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;

const VIEW: u16 = 0x033F;
const AUX: u16 = 0x0140;
const STRATEGY: u32 = 0x7F7E1E;

#[test]
fn boot_dispatches_opening_controller_from_the_shared_actor_pool() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("opening boot verification requires the user-owned retail SF2 ROM");
    let mut machine = sf_oracle::RetailMachine::new(rom);
    machine.watch_cpu_execution(&[0x7F34E7, 0x0DBCCF]);
    let mut controller = OpeningSceneController::default();
    let mut palette = OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]);
    for expected_elapsed in 0..441 {
        assert!(machine.tick_until_cpu_execution(0, 0x0DBCCF, 240).unwrap());
        let word = |address: u16| machine.peek16(0x7E0000 + u32::from(address));
        let player = word(PLAYER_ONE);
        assert!(sf2_game::object::object_index(player).is_some());
        assert_eq!(word(CURRENT_OBJECT), player);
        assert_eq!(word(sf2_game::object::ACTIVE_LIST), player);
        let auxiliary = word(player + FIELD_PATH);
        assert_eq!(word(auxiliary.wrapping_add(0x6C13)), 0xBEDF);
        assert_eq!(word(auxiliary.wrapping_add(0x6C16)), expected_elapsed);
        assert_eq!(machine.peek8(0x7E1D72), cue_encoding(controller.cue()));
        let mut slots = HashSet::new();
        let mut cursor = word(sf2_game::object::ACTIVE_LIST);
        let mut previous = 0;
        while cursor != 0 {
            assert!(sf2_game::object::object_index(cursor).is_some());
            assert!(
                slots.insert(cursor),
                "active cycle at update {expected_elapsed}"
            );
            assert_eq!(word(cursor + 2), previous);
            previous = cursor;
            cursor = word(cursor);
        }
        cursor = word(FREE_LIST);
        while cursor != 0 {
            assert!(sf2_game::object::object_index(cursor).is_some());
            assert!(
                slots.insert(cursor),
                "live/free overlap at update {expected_elapsed}"
            );
            cursor = word(cursor);
        }
        assert_eq!(slots.len(), 60);
        // At the controller entry, this update's global actor clock has
        // already advanced. Do not substitute controller elapsed time for it.
        assert_eq!(machine.peek8(0x7E00C4), (expected_elapsed + 1) as u8);
        let entries = machine.take_cpu_execution_watch_hits();
        assert_eq!(entries.last(), Some(&0x0DBCCF));
        assert_eq!(entries.iter().filter(|&&pc| pc == 0x7F34E7).count(), 1);
        controller.tick(&mut palette);
    }
}

fn cue_encoding(cue: OpeningCameraCue) -> u8 {
    match cue {
        OpeningCameraCue::Opening => 1,
        OpeningCameraCue::FirstCut => 2,
        OpeningCameraCue::SecondCut => 3,
        OpeningCameraCue::ThirdCut => 4,
        OpeningCameraCue::FourthCut => 5,
        OpeningCameraCue::FinalCut => 6,
    }
}

#[test]
fn opening_root_and_controller_run_with_every_source_child_in_one_pool() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("whole-opening verification requires the user-owned retail SF2 ROM");
    for seed in [[0; 4], [17, 91, 211, 37]] {
        let mut exact = Game::new(rom.clone()).unwrap();
        let root = allocate(&mut exact.memory, 0).unwrap();
        exact.memory.write_word(root + FIELD_PATH, 0xFA11);
        exact.memory.write_word(root + FIELD_SHAPE, 0xBC9C);
        exact
            .memory
            .write_word(root + FIELD_STRATEGY, STRATEGY as u16);
        exact
            .memory
            .write_byte(root + FIELD_STRATEGY + 2, (STRATEGY >> 16) as u8);
        exact.memory.write_byte(root + 0x2D, 1);
        exact.memory.write_word(PLAYER_ONE, VIEW);
        exact.memory.write_word(SELECTED_OBJECT, VIEW);
        exact.memory.write_word(VIEW + FIELD_PATH, AUX);
        exact.memory.write_word(AUX + 0x6C13, 0xBEDF);
        exact.memory.write_byte(AUX + 0x6C15, 0x0D);
        exact.memory.write_byte(0x1D72, 1);
        exact.memory.write_byte(0x1AA6, 2);
        for (index, value) in seed.into_iter().enumerate() {
            exact.memory.write_byte(0xE0 + index as u16, value);
        }
        let mut controller = OpeningSceneController::default();
        let mut palette = OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]);
        let mut native_root = OpeningSceneRoot::default();
        let mut observed_shapes = HashSet::new();
        let mut maximum_live = 0;
        for update in 0..460 {
            exact.memory.write_word(0x1D16, 0);
            exact.run_retail_oracle_routine(0x0DBCCF, VIEW).unwrap();
            controller.tick(&mut palette);
            let cue = controller.cue();
            assert_eq!(
                exact.memory.read_byte(0x1D72),
                cue_encoding(cue),
                "cue update={update}"
            );
            exact.memory.write_word(CURRENT_OBJECT, root);
            exact.run_retail_oracle_routine(0x7F34E7, root).unwrap();
            exact.run_retail_oracle_routine(0x7F354A, root).unwrap();
            native_root.tick(cue);
            for (field, expected) in [12, 14, 16].into_iter().zip([
                native_root.pose.position.x,
                native_root.pose.position.y,
                native_root.pose.position.z,
            ]) {
                assert_eq!(
                    exact.memory.read_word(root + field) as i16,
                    expected,
                    "root update={update} field={field}"
                );
            }
            exact.run_retail_oracle_routine(0x7F402D, root).unwrap();
            let live = active_objects(&exact.memory);
            assert_eq!(live.first(), Some(&root));
            maximum_live = maximum_live.max(live.len());
            let mut all_slots = HashSet::new();
            for (index, actor) in live.iter().copied().enumerate() {
                assert!(
                    all_slots.insert(actor),
                    "duplicate live slot update={update}"
                );
                assert_eq!(
                    exact.memory.read_word(actor + 2),
                    if index == 0 { 0 } else { live[index - 1] }
                );
                observed_shapes.insert(exact.memory.read_word(actor + FIELD_SHAPE));
            }
            let mut free = exact.memory.read_word(FREE_LIST);
            while free != 0 {
                assert!(
                    all_slots.insert(free),
                    "live/free overlap or cycle update={update}"
                );
                free = exact.memory.read_word(free);
            }
            assert_eq!(all_slots.len(), 60, "pool conservation update={update}");
        }
        assert!(controller.transition_requested);
        for index in [64u16, 89, 338, 371, 372, 376] {
            assert!(
                observed_shapes.contains(&(0xBC9C + index * 28)),
                "missing opening shape {index}"
            );
        }
        assert!(maximum_live > 20, "logo and opening actors must coexist");
    }
}
