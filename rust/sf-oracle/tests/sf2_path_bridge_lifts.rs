//! Differential proof for lifted SF2 path operations and branches.
//!
//! Each retail handler is executed from the reset-copied runtime block after
//! replacing only its dispatcher tail with a return. The typed operation then
//! runs against an independently seeded compatibility state, and the semantic
//! fields are compared directly.

use sf2_game::object::*;
use sf2_game::oracle_compat::Game;
use sf2_path::{PathAddress, PathVm, Sf2PathHost, Sf2PathOperation};

const SELECTED_SLOT: u16 = 18;
const LINKED_SLOT: u16 = 20;
const PLAYER_OBJECT: u16 = 0x033F;
const RETAIL_PATH_POINTER: u16 = 0x00F9;
const RETAIL_PATH_BANK: u8 = 0x44;

fn retail_sf2() -> Option<Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)?;
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc")).ok()
}

fn seeded_game(rom: Vec<u8>) -> (Game, u16) {
    let mut game = Game::new(rom).expect("oracle game");
    let current = allocate(&mut game.memory, 0).expect("current object");
    let linked = allocate(&mut game.memory, current).expect("linked object");
    let selected = allocate(&mut game.memory, linked).expect("selected object");
    game.memory.write_word(CURRENT_OBJECT, current);
    game.memory.write_word(SELECTED_OBJECT, selected);
    game.memory.write_word(current + 0x06, linked);
    game.memory.write_word(linked + FIELD_PATH, LINKED_SLOT);
    game.memory.write_word(selected + FIELD_PATH, SELECTED_SLOT);
    (game, current)
}

fn run_exact(game: &mut Game, current: u16, handler: u32, dispatcher_tail: u32) {
    // The retail handler has already completed its semantic mutation when it
    // reaches this tail. Returning here isolates that handler from the path
    // dispatcher's next command without changing any gameplay operation.
    game.memory.write_long_byte(dispatcher_tail, 0x6B);
    game.run_retail_oracle_routine(handler, current)
        .expect("retail handler");
}

fn run_typed_path(game: &mut Game, address: u16) -> PathVm {
    let mut path = PathVm::new(PathAddress { offset: address });
    path.run(game, 1).expect("typed path command");
    path
}

fn run_exact_inline(game: &mut Game, current: u16, address: u16) {
    game.memory.write_word(current + FIELD_PATH, address);
    run_exact(game, current, 0x7F_A2F2, 0x7F_A31D);
}

fn is_oracle_rotation_scratch(offset: usize) -> bool {
    (0x0002..=0x000B).contains(&offset)
        || (0x0097..=0x0098).contains(&offset)
        || (0x00E4..=0x00E9).contains(&offset)
}

fn assert_inline_state_matches(rom: &[u8], address: u16, seed: impl Fn(&mut Game, u16)) {
    let (mut exact, current) = seeded_game(rom.to_vec());
    let (mut typed, _) = seeded_game(rom.to_vec());
    seed(&mut exact, current);
    seed(&mut typed, current);
    let initial = exact.memory.main_state().to_vec();
    assert_eq!(typed.memory.main_state(), initial.as_slice());

    run_exact_inline(&mut exact, current, address);
    let typed_path = run_typed_path(&mut typed, address);
    assert_eq!(
        typed_path.cursor().offset,
        exact.memory.read_word(current + FIELD_PATH),
        "inline service cursor ${address:04X}"
    );

    let changed: Vec<_> = (0..initial.len())
        .filter(|&offset| {
            let state_address = offset as u16;
            offset != 0x1A31D
                && !is_oracle_rotation_scratch(offset)
                && ![current + FIELD_PATH, current + FIELD_PATH + 1].contains(&state_address)
                && (exact.memory.main_state()[offset] != initial[offset]
                    || typed.memory.main_state()[offset] != initial[offset])
        })
        .collect();
    for offset in changed {
        assert_eq!(
            typed.memory.main_state()[offset],
            exact.memory.main_state()[offset],
            "inline service ${address:04X} state ${offset:04X}"
        );
    }
}

fn select_retail_path(game: &mut Game, address: u16, prefix_size: u16) {
    game.memory
        .write_word(RETAIL_PATH_POINTER, address.wrapping_add(prefix_size));
    game.memory
        .write_byte(RETAIL_PATH_POINTER + 2, RETAIL_PATH_BANK);
}

#[test]
fn lifted_auxiliary_operations_match_the_retail_handlers() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            game.memory.write_byte(0x6B77 + SELECTED_SLOT, 0xFF);
            game.memory.write_byte(0x6A61 + SELECTED_SLOT, 0xA5);
        }
        run_exact(&mut exact, current, 0x7F_B660, 0x7F_B67A);
        Sf2PathHost::perform_path_operation(
            &mut typed,
            Sf2PathOperation::ResetSelectedAuxiliaryMotion,
        )
        .unwrap();
        assert_eq!(typed.memory.read_byte(0x6B77 + SELECTED_SLOT), 0xFB);
        assert_eq!(typed.memory.read_byte(0x6A61 + SELECTED_SLOT), 0);
        assert_eq!(
            typed.memory.read_byte(0x6B77 + SELECTED_SLOT),
            exact.memory.read_byte(0x6B77 + SELECTED_SLOT)
        );
        assert_eq!(
            typed.memory.read_byte(0x6A61 + SELECTED_SLOT),
            exact.memory.read_byte(0x6A61 + SELECTED_SLOT)
        );
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        exact.memory.write_word(0x6C03 + LINKED_SLOT, 0xFFFF);
        typed.memory.write_word(0x6C03 + LINKED_SLOT, 0xFFFF);
        run_exact(&mut exact, current, 0x7F_BDE5, 0x7F_BDF5);
        Sf2PathHost::perform_path_operation(
            &mut typed,
            Sf2PathOperation::IncrementLinkedAuxiliaryCounter,
        )
        .unwrap();
        assert_eq!(typed.memory.read_word(0x6C03 + LINKED_SLOT), 0xFF00);
        assert_eq!(
            typed.memory.read_word(0x6C03 + LINKED_SLOT),
            exact.memory.read_word(0x6C03 + LINKED_SLOT)
        );
    }

    for initial in [0, 5] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        exact.memory.write_word(0x6C03 + LINKED_SLOT, initial);
        typed.memory.write_word(0x6C03 + LINKED_SLOT, initial);
        run_exact(&mut exact, current, 0x7F_BDC9, 0x7F_BDE2);
        Sf2PathHost::perform_path_operation(
            &mut typed,
            Sf2PathOperation::DecrementLinkedAuxiliaryCounter,
        )
        .unwrap();
        assert_eq!(
            typed.memory.read_word(0x6C03 + LINKED_SLOT),
            exact.memory.read_word(0x6C03 + LINKED_SLOT)
        );
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, typed_current) = seeded_game(rom.clone());
        run_exact(&mut exact, current, 0x7F_BE2D, 0x7F_BE30);
        Sf2PathHost::perform_path_operation(
            &mut typed,
            Sf2PathOperation::SelectCurrentAsRotationTarget,
        )
        .unwrap();
        assert_eq!(typed.memory.read_word(0x1DFF), typed_current);
        assert_eq!(
            typed.memory.read_word(0x1DFF),
            exact.memory.read_word(0x1DFF)
        );
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        exact.memory.write_byte(0x6B77 + SELECTED_SLOT, 0xFF);
        typed.memory.write_byte(0x6B77 + SELECTED_SLOT, 0xFF);
        run_exact(&mut exact, current, 0x7F_B77A, 0x7F_B78F);
        Sf2PathHost::perform_path_operation(
            &mut typed,
            Sf2PathOperation::ClearSelectedAuxiliaryFlag01,
        )
        .unwrap();
        assert_eq!(typed.memory.read_byte(0x6B77 + SELECTED_SLOT), 0xFE);
        assert_eq!(
            typed.memory.read_byte(0x6B77 + SELECTED_SLOT),
            exact.memory.read_byte(0x6B77 + SELECTED_SLOT)
        );
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom);
        exact.memory.write_byte(0x6AA1 + SELECTED_SLOT, 0xBC);
        typed.memory.write_byte(0x6AA1 + SELECTED_SLOT, 0xBC);
        run_exact(&mut exact, current, 0x7F_B081, 0x7F_B098);
        Sf2PathHost::perform_path_operation(
            &mut typed,
            Sf2PathOperation::SetSelectedSlotLowNibble1,
        )
        .unwrap();
        assert_eq!(typed.memory.read_byte(0x6AA1 + SELECTED_SLOT), 0xB1);
        assert_eq!(
            typed.memory.read_byte(0x6AA1 + SELECTED_SLOT),
            exact.memory.read_byte(0x6AA1 + SELECTED_SLOT)
        );
    }
}

#[test]
fn lifted_cursor_and_transform_operations_match_the_retail_handlers() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        select_retail_path(&mut exact, 0xCF1E, 1);
        run_exact(&mut exact, current, 0x7F_BF05, 0x7F_BF0B);
        let path = run_typed_path(&mut typed, 0xCF1E);
        assert_eq!(path.cursor().offset, 0xCF21);
        assert_eq!(typed.memory.read_byte(0x1D72), 2);
        assert_eq!(
            typed.memory.read_byte(0x1D72),
            exact.memory.read_byte(0x1D72)
        );
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            game.memory.write_byte(0x1D77, 5);
            game.memory.write_word(0x1D78, 0x2468);
        }
        select_retail_path(&mut exact, 0xB8DF, 1);
        run_exact(&mut exact, current, 0x7F_BF3D, 0x7F_BF4B);
        run_typed_path(&mut typed, 0xB8DF);
        assert_eq!(
            typed.memory.read_byte(0x192E),
            exact.memory.read_byte(0x192E)
        );
        assert_eq!(
            typed.memory.read_word(0x1657),
            exact.memory.read_word(0x1657)
        );
    }

    for selector in [2, 3] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            game.memory.write_byte(0x1D72, selector);
        }
        select_retail_path(&mut exact, 0xCF35, 1);
        exact
            .memory
            .write_word(current + FIELD_PATH, 0xCF35u16.wrapping_add(1));
        run_exact(&mut exact, current, 0x7F_BF27, 0x7F_7E75);
        let typed_path = run_typed_path(&mut typed, 0xCF35);
        assert_eq!(
            typed_path.cursor().offset,
            exact.memory.read_word(current + FIELD_PATH)
        );
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            for (field, current_value, player_value) in [
                (FIELD_X, 400i16, -120i16),
                (FIELD_Y, -256, 800),
                (FIELD_Z, 32, 33),
            ] {
                game.memory
                    .write_word(current + field, current_value as u16);
                game.memory
                    .write_word(PLAYER_OBJECT + field, player_value as u16);
            }
        }
        select_retail_path(&mut exact, 0xD17C, 1);
        run_exact(&mut exact, current, 0x7F_C028, 0x7F_C066);
        Sf2PathHost::perform_path_operation(
            &mut typed,
            Sf2PathOperation::ChaseObjectPositionTowardCurrent(PLAYER_OBJECT),
        )
        .unwrap();
        for field in [FIELD_X, FIELD_Y, FIELD_Z] {
            assert_eq!(
                typed.memory.read_word(PLAYER_OBJECT + field),
                exact.memory.read_word(PLAYER_OBJECT + field)
            );
        }
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            let selected = game.memory.read_word(SELECTED_OBJECT);
            for (field, angle) in [(FIELD_ROT_X, 17), (FIELD_ROT_Y, 129), (FIELD_ROT_Z, 246)] {
                game.memory.write_byte(selected + field, angle);
            }
        }
        run_exact(&mut exact, current, 0x7F_BF4E, 0x7F_BF55);
        Sf2PathHost::perform_path_operation(&mut typed, Sf2PathOperation::CopySelectedRotation)
            .unwrap();
        for field in [FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z] {
            assert_eq!(
                typed.memory.read_byte(current + field),
                exact.memory.read_byte(current + field)
            );
        }
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            for (field, value) in [(FIELD_X, 0x1234), (FIELD_Y, 0xFEDC), (FIELD_Z, 0x8001)] {
                game.memory.write_word(current + field, value);
            }
        }
        select_retail_path(&mut exact, 0xB911, 1);
        run_exact(&mut exact, current, 0x7F_BFF6, 0x7F_C002);
        Sf2PathHost::perform_path_operation(
            &mut typed,
            Sf2PathOperation::CopyPositionToObject(PLAYER_OBJECT),
        )
        .unwrap();
        for field in [FIELD_X, FIELD_Y, FIELD_Z] {
            assert_eq!(
                typed.memory.read_word(PLAYER_OBJECT + field),
                exact.memory.read_word(PLAYER_OBJECT + field)
            );
        }
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom);
        for game in [&mut exact, &mut typed] {
            for (field, angle) in [(FIELD_ROT_X, 17), (FIELD_ROT_Y, 129), (FIELD_ROT_Z, 246)] {
                game.memory.write_byte(current + field, angle);
            }
        }
        select_retail_path(&mut exact, 0xB930, 1);
        run_exact(&mut exact, current, 0x7F_C005, 0x7F_C025);
        Sf2PathHost::perform_path_operation(
            &mut typed,
            Sf2PathOperation::CopyRotationToObjectFixed(PLAYER_OBJECT),
        )
        .unwrap();
        for field in [FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z] {
            assert_eq!(
                typed.memory.read_word(PLAYER_OBJECT + field),
                exact.memory.read_word(PLAYER_OBJECT + field)
            );
        }
    }
}

#[test]
fn final_path_stack_rotation_and_auxiliary_lifts_match_retail() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            game.memory.write_word(0xB26B, 0xA1A1);
            assert!(push_path_stack(&mut game.memory, current, 0x1111));
            game.memory.write_word(0xB26B, 0xB2B2);
            assert!(push_path_stack(&mut game.memory, current, 0x2222));
        }
        run_exact(&mut exact, current, 0x7F_9760, 0x7F_9788);
        Sf2PathHost::perform_path_operation(&mut typed, Sf2PathOperation::PopPathStackPair)
            .unwrap();
        let block = typed.memory.read_word(current.wrapping_add(0x1CDE));
        assert_eq!(typed.memory.read_byte(0x6A61u16.wrapping_add(block)), 0);
        for address in [0x16B1, 0xB269, 0xB26B] {
            assert_eq!(
                typed.memory.read_word(address),
                exact.memory.read_word(address)
            );
        }
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            let player = game.memory.read_word(SELECTED_OBJECT);
            game.memory.write_word(PLAYER_ONE, player);
            game.memory.write_word(player + FIELD_PATH, SELECTED_SLOT);
            for (field, value) in [(FIELD_X, 300), (FIELD_Y, -120), (FIELD_Z, 900)] {
                game.memory.write_word(current + field, value as u16);
            }
        }
        select_retail_path(&mut exact, 0xF38E, 0);
        run_exact(&mut exact, current, 0x7F_C13B, 0x7F_C152);
        Sf2PathHost::perform_path_operation(
            &mut typed,
            Sf2PathOperation::ConfigurePlayerAuxiliary(0xFFF8),
        )
        .unwrap();
        for address in [
            0x6A8C, 0x6A8D, 0x6A8E, 0x6A8F, 0x6A90, 0x6A92, 0x6A94, 0x6A96, 0x6A98, 0x6C1C, 0x6C24,
            0x6C26, 0x6C28, 0x6C29, 0x6C2A, 0x6C2B, 0x6C2C,
        ] {
            let address = address + SELECTED_SLOT;
            assert_eq!(
                typed.memory.read_byte(address),
                exact.memory.read_byte(address),
                "auxiliary byte ${address:04X}"
            );
        }
    }

    for (operation, handler, tail, path_address) in [
        (
            Sf2PathOperation::SetObjectRotationTowardTarget {
                object: PLAYER_OBJECT,
                shift: 1,
            },
            0x7F_BE38,
            0x7F_BE89,
            0xB915,
        ),
        (
            Sf2PathOperation::ChaseObjectRotationTowardTarget {
                object: PLAYER_OBJECT,
                shift: 1,
            },
            0x7F_BE8C,
            0x7F_BF02,
            0xD052,
        ),
    ] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            game.memory.write_word(0x1DFF, current);
            for (field, target_value, object_value) in [
                (FIELD_X, 640i16, -320i16),
                (FIELD_Y, -280, 480),
                (FIELD_Z, 900, -100),
            ] {
                game.memory.write_word(current + field, target_value as u16);
                game.memory
                    .write_word(PLAYER_OBJECT + field, object_value as u16);
            }
            for (field, value) in [
                (FIELD_ROT_X, 0x2400),
                (FIELD_ROT_Y, 0xA000),
                (FIELD_ROT_Z, 0x0180),
            ] {
                game.memory.write_word(PLAYER_OBJECT + field, value);
            }
        }
        select_retail_path(&mut exact, path_address, 1);
        run_exact(&mut exact, current, handler, tail);
        Sf2PathHost::perform_path_operation(&mut typed, operation).unwrap();
        for field in [FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z] {
            assert_eq!(
                typed.memory.read_word(PLAYER_OBJECT + field),
                exact.memory.read_word(PLAYER_OBJECT + field),
                "rotation field ${field:02X}"
            );
        }
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            let player = game.memory.read_word(SELECTED_OBJECT);
            game.memory.write_word(PLAYER_ONE, player);
            game.memory.write_word(player + FIELD_PATH, SELECTED_SLOT);
            game.memory.write_word(0x6A98 + SELECTED_SLOT, current);
            for (field, value) in [(FIELD_X, 0x1357), (FIELD_Y, 0x2468), (FIELD_Z, 0xABCD)] {
                game.memory.write_word(current + field, value);
            }
        }
        run_exact(&mut exact, current, 0x7F_C189, 0x7F_C199);
        Sf2PathHost::perform_path_operation(
            &mut typed,
            Sf2PathOperation::RefreshOwnedPlayerAuxiliaryOrigin,
        )
        .unwrap();
        for address in [0x6A92, 0x6A94, 0x6A96] {
            let address = address + SELECTED_SLOT;
            assert_eq!(
                typed.memory.read_word(address),
                exact.memory.read_word(address)
            );
        }
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            game.memory.write_byte(current.wrapping_add(0x1CC7), 0xA5);
            game.memory.write_word(current + FIELD_PATH, 0xAFDF);
        }
        select_retail_path(&mut exact, 0xAFDE, 1);
        run_exact(&mut exact, current, 0x7F_99F6, 0x7F_9A12);
        Sf2PathHost::perform_path_operation(
            &mut typed,
            Sf2PathOperation::InstallStrategyAndStop {
                strategy: 0xAFE4,
                state: 9,
            },
        )
        .unwrap();
        for address in [
            current + FIELD_STRATEGY,
            current + FIELD_STRATEGY + 1,
            current + FIELD_STRATEGY + 2,
            current.wrapping_add(0x1CC7),
            current + FIELD_PATH,
            current + FIELD_PATH + 1,
        ] {
            assert_eq!(
                typed.memory.read_byte(address),
                exact.memory.read_byte(address)
            );
        }
    }
}

#[test]
fn final_selected_auxiliary_branches_match_retail_pointer_control() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for (occupied, expected) in [(false, 0xE9AE), (true, 0xEAC3)] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            let selected = game.memory.read_word(SELECTED_OBJECT);
            game.memory.write_word(selected + FIELD_PATH, SELECTED_SLOT);
            game.memory
                .write_byte(0x6BEB + SELECTED_SLOT, if occupied { 0 } else { 0x80 });
            game.memory.write_word(current + FIELD_X, 0);
            game.memory.write_word(current + FIELD_Z, 0);
            game.memory.write_byte(0xCF36, 0xFF);
        }
        select_retail_path(&mut exact, 0xE9AA, 1);
        exact.memory.write_word(current + FIELD_PATH, 0xE9AB);
        run_exact(&mut exact, current, 0x7F_B73E, 0x7F_7E75);
        let typed_path = run_typed_path(&mut typed, 0xE9AA);
        assert_eq!(typed_path.cursor().offset, expected);
        assert_eq!(
            typed_path.cursor().offset,
            exact.memory.read_word(current + FIELD_PATH)
        );
    }

    for (flag, expected) in [(0x04, 0xD1F2), (0, 0xD204)] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            let selected = game.memory.read_word(SELECTED_OBJECT);
            game.memory.write_word(selected + FIELD_PATH, SELECTED_SLOT);
            game.memory.write_byte(0x6B77 + SELECTED_SLOT, flag);
        }
        select_retail_path(&mut exact, 0xD1EE, 1);
        exact.memory.write_word(current + FIELD_PATH, 0xD1EF);
        run_exact(&mut exact, current, 0x7F_B702, 0x7F_7E75);
        let typed_path = run_typed_path(&mut typed, 0xD1EE);
        assert_eq!(typed_path.cursor().offset, expected);
        assert_eq!(
            typed_path.cursor().offset,
            exact.memory.read_word(current + FIELD_PATH)
        );
    }
}

#[test]
fn selected_auxiliary_motion_capture_matches_retail() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    let (mut exact, current) = seeded_game(rom.clone());
    let (mut typed, _) = seeded_game(rom);
    for game in [&mut exact, &mut typed] {
        let selected = game.memory.read_word(SELECTED_OBJECT);
        game.memory.write_byte(0x005E, 0xFF);
        game.memory.write_byte(current + FIELD_ROT_Y, 0xA7);
        game.memory.write_byte(0x00A7, 0x19);
        game.memory.write_word(0x0004, 0x1234);
        game.memory.write_word(0x000A, 0xFEDC);
        game.memory.write_word(0x00E4, 128);
        for (field, current_value, selected_value) in [
            (FIELD_X, 1_000i16, 1_040i16),
            (FIELD_Y, -300, 700),
            (FIELD_Z, 2_000, 2_050),
        ] {
            game.memory
                .write_word(current + field, current_value as u16);
            game.memory
                .write_word(selected + field, selected_value as u16);
        }
        game.memory.write_byte(0x6B77 + SELECTED_SLOT, 0x84);
    }

    run_exact(&mut exact, current, 0x7F_B6EE, 0x7F_CAE8);
    let typed_path = run_typed_path(&mut typed, 0xD1E5);
    assert_eq!(typed_path.cursor().offset, 0xD1E6);

    for address in [0x005E, 0x00A7, 0x16BF, 0x6B77 + SELECTED_SLOT] {
        assert_eq!(
            typed.memory.read_byte(address),
            exact.memory.read_byte(address),
            "capture byte ${address:04X}"
        );
    }
    for address in [
        0x0002,
        0x0008,
        0x0097,
        0x16B7,
        0x16B9,
        0x16BB,
        0x6AAF + SELECTED_SLOT,
        0x6AB1 + SELECTED_SLOT,
        0x6AB3 + SELECTED_SLOT,
        0x6AB5 + SELECTED_SLOT,
        0x6AB7 + SELECTED_SLOT,
    ] {
        assert_eq!(
            typed.memory.read_word(address),
            exact.memory.read_word(address),
            "capture word ${address:04X}"
        );
    }
    assert_eq!(
        typed.memory.read_byte(0x6AAE + SELECTED_SLOT),
        exact.memory.read_byte(0x6AAE + SELECTED_SLOT)
    );
}

#[test]
fn selected_auxiliary_capture_eligibility_matches_retail_boundaries() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for (angle, selected_x, selected_y, selected_z, radius, vertical_radius) in [
        (0u8, 1_020i16, 0i16, 2_000i16, 50i16, 20i16),
        (128, 1_200, 0, 2_000, 50, 20),
        (64, 1_000, 0, 2_020, 50, 20),
        (192, 1_000, 0, 2_200, 50, 20),
        (32, 1_050, 0, 2_000, 100, 20),
        (32, 1_200, 0, 2_000, 100, 20),
        (160, 1_200, 0, 2_000, 100, 20),
        (160, 1_050, 0, 2_000, 100, 20),
        (96, 1_000, 0, 2_000, 100, 20),
        (96, 1_200, 0, 2_000, 100, 20),
        (224, 800, 0, 2_000, 100, 20),
        (224, 1_000, 0, 2_000, 100, 20),
        (0, 1_020, 100, 2_000, 50, 20),
        (0, 1_020, 100, 2_000, 50, -1),
    ] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            let selected = game.memory.read_word(SELECTED_OBJECT);
            let external = game.memory.read_word(current + 0x06);
            game.memory.write_word(0x14D6, external);
            game.memory.write_byte(current + FIELD_ROT_Y, 0);
            game.memory.write_byte(0x00A7, angle);
            game.memory.write_word(0x0004, radius as u16);
            game.memory.write_word(0x000A, vertical_radius as u16);
            game.memory.write_word(0x00E4, 30_000);
            game.memory.write_word(current + FIELD_X, 1_000);
            game.memory.write_word(current + FIELD_Y, 0);
            game.memory.write_word(current + FIELD_Z, 2_000);
            game.memory
                .write_word(selected + FIELD_X, selected_x as u16);
            game.memory
                .write_word(selected + FIELD_Y, selected_y as u16);
            game.memory
                .write_word(selected + FIELD_Z, selected_z as u16);
            game.memory.write_byte(external + 0x20, 0xFF);
            game.memory.write_byte(0x6B77 + SELECTED_SLOT, 0);
            for address in [0x1DB2, 0x1DB6, 0x1DB8, 0x1DBC, 0x1DBE, 0x1DC0] {
                game.memory.write_word(address, 0xA55A);
            }
        }

        run_exact(&mut exact, current, 0x7F_B6EE, 0x7F_CAE8);
        let typed_path = run_typed_path(&mut typed, 0xD1E5);
        let external = typed.memory.read_word(current + 0x06);
        for address in [
            external + FIELD_X,
            external + FIELD_X + 1,
            external + FIELD_Y,
            external + FIELD_Y + 1,
            external + FIELD_Z,
            external + FIELD_Z + 1,
            external + 0x20,
            0x1DB2,
            0x1DB3,
            0x1DB6,
            0x1DB7,
            0x1DB8,
            0x1DB9,
            0x1DBC,
            0x1DBD,
            0x1DBE,
            0x1DBF,
            0x1DC0,
            0x1DC1,
            0x6B77 + SELECTED_SLOT,
            0x6AAE + SELECTED_SLOT,
            0x6AAF + SELECTED_SLOT,
            0x6AB0 + SELECTED_SLOT,
            0x6AB1 + SELECTED_SLOT,
            0x6AB2 + SELECTED_SLOT,
            0x6AB3 + SELECTED_SLOT,
            0x6AB4 + SELECTED_SLOT,
        ] {
            assert_eq!(
                typed.memory.read_byte(address),
                exact.memory.read_byte(address),
                "capture boundary ${address:04X}, angle={angle}, selected=({selected_x},{selected_y},{selected_z}), radius={radius}, vertical_radius={vertical_radius}"
            );
        }
        assert_eq!(typed_path.cursor().offset, 0xD1E6);
    }
}

#[derive(Debug, Clone, Copy)]
enum DirectContactSeed {
    NoObject,
    OrdinaryObject,
    OtherObject,
}

fn seed_direct_contact(game: &mut Game, current: u16, seed: DirectContactSeed) {
    const LINK_RECORD: u16 = 0x3000;
    let target = match seed {
        DirectContactSeed::NoObject => 0,
        DirectContactSeed::OrdinaryObject | DirectContactSeed::OtherObject => {
            game.memory.read_word(SELECTED_OBJECT)
        }
    };
    game.memory.write_byte(0x005E, 0xFF);
    game.memory.write_byte(current + 0x20, 0x80);
    game.memory.write_word(current + 0x1E, LINK_RECORD);
    game.memory.write_word(LINK_RECORD + 4, target);
    if target == 0 {
        return;
    }

    game.memory.write_byte(target + 0x2D, 1);
    let (flags, kind, value) = match seed {
        DirectContactSeed::NoObject => unreachable!(),
        DirectContactSeed::OrdinaryObject => (0, 0x0B, 0x6C),
        DirectContactSeed::OtherObject => (0x08, 0x0D, 0x92),
    };
    game.memory.write_byte(target + 0x22, flags);
    let entry = get_or_create_auxiliary_type(&mut game.memory, target, kind)
        .expect("contact auxiliary entry");
    write_auxiliary_byte(&mut game.memory, entry + 1, value);
}

#[test]
fn contact_class_branches_match_retail_without_the_collision_leaf() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for (seed, expected) in [
        (DirectContactSeed::NoObject, 0xEA85),
        (DirectContactSeed::OrdinaryObject, 0xEAA5),
        (DirectContactSeed::OtherObject, 0xEABC),
    ] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        seed_direct_contact(&mut exact, current, seed);
        seed_direct_contact(&mut typed, current, seed);

        select_retail_path(&mut exact, 0xE9A2, 1);
        exact
            .memory
            .write_word(current + FIELD_PATH, 0xE9A2u16.wrapping_add(1));
        run_exact(&mut exact, current, 0x7F_BDF8, 0x7F_7E75);
        let typed_path = run_typed_path(&mut typed, 0xE9A2);

        assert_eq!(typed_path.cursor().offset, expected);
        assert_eq!(
            typed_path.cursor().offset,
            exact.memory.read_word(current + FIELD_PATH)
        );
        for address in [0x0002, 0x0008] {
            assert_eq!(
                typed.memory.read_word(address),
                exact.memory.read_word(address),
                "contact word ${address:04X}"
            );
        }
        for address in [0xD746] {
            assert_eq!(
                typed.memory.read_byte(address),
                exact.memory.read_byte(address),
                "contact {seed:?} byte ${address:04X}"
            );
        }
    }
}

#[test]
fn simple_inline_lifts_match_isolated_retail_blocks() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            let spawned = game.memory.read_word(SELECTED_OBJECT);
            game.memory.write_word(0xD771, spawned);
        }
        run_exact_inline(&mut exact, current, 0x2059);
        let typed_path = run_typed_path(&mut typed, 0x2059);
        let spawned = typed.memory.read_word(SELECTED_OBJECT);
        assert_eq!(typed.memory.read_word(spawned + 0x1C), current);
        assert_eq!(
            typed.memory.read_word(spawned + 0x1C),
            exact.memory.read_word(spawned + 0x1C)
        );
        assert_eq!(
            typed_path.cursor().offset,
            exact.memory.read_word(current + FIELD_PATH)
        );
    }

    {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            for (field, value) in [(FIELD_X, 1), (FIELD_Y, 2), (FIELD_Z, 3)] {
                game.memory.write_word(current + field, value);
            }
        }
        run_exact_inline(&mut exact, current, 0xB8E4);
        let typed_path = run_typed_path(&mut typed, 0xB8E4);
        for field in [
            FIELD_X,
            FIELD_Y,
            FIELD_Z,
            FIELD_ROT_X,
            FIELD_ROT_Y,
            FIELD_ROT_Z,
        ] {
            assert_eq!(
                typed.memory.read_byte(current + field),
                exact.memory.read_byte(current + field),
                "opposite-world field ${field:02X}"
            );
        }
        assert_eq!(
            typed_path.cursor().offset,
            exact.memory.read_word(current + FIELD_PATH)
        );
    }

    for (address, expected_flags) in [
        (0x8D62, 0x84),
        (0x9808, 0x07),
        (0xCFF8, 0x04),
        (0xD098, 0x08),
        (0xE845, 0x01),
        (0xF313, 0x82),
    ] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            game.memory.write_byte(current + 0x24, 0x80);
            game.memory.write_byte(current + 0x25, 0x80);
            game.memory.write_byte(current.wrapping_add(0x1CCA), 6);
            game.memory.write_byte(0x1D74, 0);
        }
        run_exact_inline(&mut exact, current, address);
        let typed_path = run_typed_path(&mut typed, address);
        let (typed_value, exact_value) = match address {
            0x8D62 => (
                typed.memory.read_byte(current + 0x24),
                exact.memory.read_byte(current + 0x24),
            ),
            0x9808 => (
                typed.memory.read_byte(current.wrapping_add(0x1CCA)),
                exact.memory.read_byte(current.wrapping_add(0x1CCA)),
            ),
            0xF313 => (
                typed.memory.read_byte(current + 0x25),
                exact.memory.read_byte(current + 0x25),
            ),
            _ => (
                typed.memory.read_byte(0x1D74),
                exact.memory.read_byte(0x1D74),
            ),
        };
        assert_eq!(typed_value, expected_flags, "inline ${address:04X}");
        assert_eq!(typed_value, exact_value, "inline ${address:04X}");
        assert_eq!(
            typed_path.cursor().offset,
            exact.memory.read_word(current + FIELD_PATH),
            "inline cursor ${address:04X}"
        );
    }
}

#[test]
fn named_inline_service_boundaries_match_isolated_retail_blocks() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    const SERVICE_SITES: [u16; 18] = [
        0xB91A, 0xD024, 0xD0DE, 0xDCBD, 0xE78A, 0xE839, 0xE939, 0xE967, 0xF078, 0xF2E4, 0xF391,
        0xF39E, 0xF3F0, 0xF45B, 0xF46E, 0xF659, 0xF693, 0xF7C9,
    ];

    for address in SERVICE_SITES {
        assert_inline_state_matches(&rom, address, |_, _| {});
    }
}

#[test]
fn native_inline_math_services_match_retail_edge_cases() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for (snapshot, hit_points) in [(0, 0), (2, 1), (10, 7), (200, 60), (5, 250)] {
        assert_inline_state_matches(&rom, 0xF659, |game, current| {
            game.memory.write_byte(current + 0x27, snapshot);
            game.memory.write_byte(current + 0x2D, hit_points);
        });
    }

    for (yaw, relative_yaw) in [(0, 0), (10, 200), (200, 10), (250, 5)] {
        assert_inline_state_matches(&rom, 0xF693, |game, current| {
            game.memory.write_byte(current + FIELD_ROT_Y, yaw);
            game.memory
                .write_byte(current.wrapping_add(0x1CD6), relative_yaw);
        });
    }

    for yaw in [0, 1, 0x7FFF, 0x8000, 0xC001, 0xFFFF] {
        assert_inline_state_matches(&rom, 0xB91A, |game, _| {
            game.memory.write_word(0x033F + FIELD_ROT_Y, yaw);
            game.memory.write_word(0x1E52, 0xA55A);
        });
    }

    for (yaw, fixed_player_yaw) in [(0, 0), (20, 200), (200, 20), (255, 128)] {
        assert_inline_state_matches(&rom, 0xDCBD, |game, current| {
            game.memory.write_byte(current + FIELD_ROT_Y, yaw);
            game.memory
                .write_byte(0x033F + FIELD_ROT_Y + 1, fixed_player_yaw);
        });
    }

    for variant in 0..16 {
        assert_inline_state_matches(&rom, 0xD024, |game, current| {
            game.memory.write_byte(0x1D8F, variant);
            game.memory.write_word(current + FIELD_X, 32_700);
            game.memory
                .write_word(current + FIELD_Y, (-32_000i16) as u16);
            game.memory.write_word(current + FIELD_Z, 1_234);
        });
    }

    for mode in [0x10, 0x20] {
        assert_inline_state_matches(&rom, 0xE78A, |game, current| {
            let player = game.memory.read_word(SELECTED_OBJECT);
            let slot = game.memory.read_word(player + FIELD_PATH);
            game.memory.write_word(PLAYER_ONE, player);
            game.memory.write_byte(0x6AA0 + slot, mode);
            game.memory.write_word(current + 0x32, 0x7FF0);
            game.memory.write_word(current + 0x36, 0x8010);
            game.memory.write_word(player + 0x32, 0x0020);
            game.memory.write_word(player + 0x36, 0xFFE0);
            game.memory.write_word(player.wrapping_add(0x1CC1), 0x0100);
            game.memory.write_word(player.wrapping_add(0x1CC5), 0xFF00);
        });
    }

    for global_flags in [0, 2, 4, 0xFF] {
        assert_inline_state_matches(&rom, 0xE839, |game, _| {
            let player = game.memory.read_word(SELECTED_OBJECT);
            game.memory.write_word(PLAYER_ONE, player);
            game.memory.write_byte(0x1D74, global_flags);
        });
    }

    for selected_flags in [0, 0x80] {
        assert_inline_state_matches(&rom, 0xF078, |game, current| {
            let selected = game.memory.read_word(SELECTED_OBJECT);
            let slot = game.memory.read_word(selected + FIELD_PATH);
            game.memory.write_byte(0x6B63 + slot, selected_flags);
            game.memory.write_word(current.wrapping_add(0x1CE4), 0xA55A);
            game.memory.write_byte(0x6C08 + slot, 0x1B);
        });
    }

    for (class, frame, gate, override_flags, disable, mode) in [
        (9, 1, 1, 0, 1, 0x20),
        (0, 8, 1, 0, 1, 0x20),
        (0, 1, 0, 1, 1, 0x22),
        (0, 1, 0, 0, 0, 0x22),
        (0, 1, 0, 0, 1, 0x20),
        (0, 1, 0, 0, 1, 1),
    ] {
        assert_inline_state_matches(&rom, 0xF2E4, |game, current| {
            let linked = game.memory.read_word(current + 0x06);
            let slot = game.memory.read_word(linked + FIELD_PATH);
            game.memory.write_byte(0x1DE2, class);
            game.memory.write_byte(0x1B4D, frame);
            game.memory.write_byte(0x1D72, gate);
            game.memory.write_byte(0x1E0D, override_flags);
            game.memory.write_byte(0xD7F4, disable);
            game.memory.write_byte(0x6C02 + slot, mode);
            game.memory.write_byte(current.wrapping_add(0x1CD5), 250);
            game.memory.write_byte(current.wrapping_add(0x1CD7), 252);
        });
    }

    for selected_flags in [0, 0x80] {
        assert_inline_state_matches(&rom, 0xF391, |game, _| {
            let player = game.memory.read_word(SELECTED_OBJECT);
            let slot = game.memory.read_word(player + FIELD_PATH);
            game.memory.write_word(PLAYER_ONE, player);
            game.memory.write_byte(0x6B63 + slot, selected_flags);
            game.memory.write_byte(0x6A8C + slot, 0x03);
        });
    }

    for (depth, vertical) in [(200i16, 0i16), (0, 7), (207, -7), (-32_000, 32_000)] {
        assert_inline_state_matches(&rom, 0xF3F0, |game, current| {
            game.memory
                .write_word(current.wrapping_add(0x1CD3), depth as u16);
            game.memory
                .write_word(current.wrapping_add(0x1CD1), vertical as u16);
        });
    }

    assert_inline_state_matches(&rom, 0xF45B, |game, current| {
        game.memory.write_byte(current.wrapping_add(0x1CD5), 3);
        game.memory.write_byte(current.wrapping_add(0x1CD6), 250);
        game.memory.write_byte(current.wrapping_add(0x1CD7), 252);
        game.memory.write_byte(current.wrapping_add(0x1CE2), 12);
        game.memory.write_word(current.wrapping_add(0x1CD1), 0xFFFC);
        game.memory.write_word(current.wrapping_add(0x1CD3), 4);
    });

    for (horizontal, vertical, depth, target) in [
        (0i16, 0i16, 0i16, 0i16),
        (7, -7, 7, 8),
        (-7, 7, -7, -8),
        (-32_000, 32_000, 20_000, 30_000),
    ] {
        assert_inline_state_matches(&rom, 0xF46E, |game, current| {
            game.memory
                .write_word(current.wrapping_add(0x1CCF), horizontal as u16);
            game.memory
                .write_word(current.wrapping_add(0x1CD1), vertical as u16);
            game.memory
                .write_word(current.wrapping_add(0x1CD3), depth as u16);
            game.memory
                .write_word(current.wrapping_add(0x1CE4), target as u16);
        });
    }

    for (phase, control) in [(0, 0), (12, 1), (12, 4), (13, 0), (255, 0)] {
        assert_inline_state_matches(&rom, 0xE939, |game, current| {
            game.memory.write_byte(0x1DD1, phase);
            game.memory.write_byte(0x00C4, control);
            game.memory.write_word(current.wrapping_add(0x1CC8), 0xA55A);
            game.memory.write_byte(current.wrapping_add(0x1CE2), 0xFF);
        });
    }

    for (position, velocity, player_y) in [
        (-2_000i16, -10i16, 0i16),
        (-2_000, 10, 0),
        (2_000, -10, 0),
        (2_000, 10, 0),
        (0, -10, 100),
        (0, 10, -100),
        (32_700, 300, -32_000),
    ] {
        assert_inline_state_matches(&rom, 0xF7C9, |game, current| {
            let player = game.memory.read_word(SELECTED_OBJECT);
            game.memory.write_word(PLAYER_ONE, player);
            game.memory.write_word(current + FIELD_Y, position as u16);
            game.memory.write_word(current + 0x34, velocity as u16);
            game.memory.write_word(player + FIELD_Y, player_y as u16);
        });
    }
}

#[test]
fn player_relative_motion_matches_retail_pose_edges() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for (linked_transform, pitch, yaw) in [
        (false, 0, 0),
        (true, 0, 0),
        (true, 64, 0),
        (true, 0, 64),
        (true, 37, 219),
        (true, 128, 128),
    ] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            let player = game.memory.read_word(SELECTED_OBJECT);
            let slot = game.memory.read_word(player + FIELD_PATH);
            game.memory.write_word(PLAYER_ONE, player);
            game.memory
                .write_byte(0x6B63 + slot, if linked_transform { 0x80 } else { 0 });
            game.memory.write_word(player + FIELD_X, 32_700);
            game.memory
                .write_word(player + FIELD_Y, (-32_000i16) as u16);
            game.memory.write_word(player + FIELD_Z, 1_234);
            game.memory.write_byte(player + FIELD_ROT_X, pitch);
            game.memory.write_byte(player + FIELD_ROT_Y, yaw);
        }

        run_exact_inline(&mut exact, current, 0xF39E);
        let typed_path = run_typed_path(&mut typed, 0xF39E);
        for field in [FIELD_X, FIELD_Y, FIELD_Z] {
            assert_eq!(
                typed.memory.read_word(current + field),
                exact.memory.read_word(current + field),
                "player-relative field ${field:02X}, pitch={pitch}, yaw={yaw}"
            );
        }
        assert_eq!(
            typed_path.cursor().offset,
            exact.memory.read_word(current + FIELD_PATH)
        );
    }
}

#[test]
fn spawned_object_motion_matches_retail_pose_edges() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for (current_yaw, spawned_yaw, variant) in [
        (0, 0, 0),
        (64, 0, 1),
        (0, 64, 2),
        (37, 219, 4),
        (128, 128, 7),
        (255, 1, 15),
    ] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            let spawned = game.memory.read_word(SELECTED_OBJECT);
            game.memory.write_word(0xD771, spawned);
            game.memory.write_word(current + FIELD_X, 32_700);
            game.memory
                .write_word(current + FIELD_Y, (-32_000i16) as u16);
            game.memory.write_word(current + FIELD_Z, 1_234);
            game.memory.write_byte(current + FIELD_ROT_Y, current_yaw);
            game.memory.write_byte(0x1BA9, spawned_yaw);
            game.memory.write_byte(0x1D8F, variant);
        }

        run_exact_inline(&mut exact, current, 0xD0DE);
        let typed_path = run_typed_path(&mut typed, 0xD0DE);
        let spawned = typed.memory.read_word(SELECTED_OBJECT);
        for field in [
            FIELD_X,
            FIELD_Y,
            FIELD_Z,
            FIELD_ROT_X,
            FIELD_ROT_Y,
            FIELD_ROT_Z,
        ] {
            assert_eq!(
                typed.memory.read_word(spawned + field),
                exact.memory.read_word(spawned + field),
                "spawned field ${field:02X}, current_yaw={current_yaw}, spawned_yaw={spawned_yaw}, variant={variant}"
            );
        }
        assert_eq!(
            typed_path.cursor().offset,
            exact.memory.read_word(current + FIELD_PATH)
        );
    }
}

#[test]
fn launched_external_object_matches_retail_motion_edges() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for (x, y, z, pitch, yaw, roll) in [
        (100i16, 100i16, 300i16, 0, 0, 1),
        (100, -1_000, 300, 0, 0, 250),
        (-32_000, -100, 32_000, 64, 64, 128),
        (100, -17, -6, 0, 0, 255),
        (32_700, -32_000, 1_234, 37, 219, 17),
    ] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            let external = game.memory.read_word(SELECTED_OBJECT);
            game.memory.write_word(0x14D6, external);
            for (field, value) in [(FIELD_X, x), (FIELD_Y, y), (FIELD_Z, z)] {
                game.memory.write_word(current + field, value as u16);
            }
            for (field, value) in [
                (FIELD_ROT_X, pitch),
                (FIELD_ROT_Y, yaw),
                (FIELD_ROT_Z, roll),
            ] {
                game.memory.write_byte(current + field, value);
                game.memory.write_byte(external + field + 1, 0xA5);
            }
        }

        run_exact_inline(&mut exact, current, 0xAC64);
        let typed_path = run_typed_path(&mut typed, 0xAC64);
        let external = typed.memory.read_word(SELECTED_OBJECT);
        for field in [
            FIELD_X,
            FIELD_Y,
            FIELD_Z,
            FIELD_ROT_X,
            FIELD_ROT_Y,
            FIELD_ROT_Z,
            0x18,
            0x32,
            0x34,
            0x36,
        ] {
            assert_eq!(
                typed.memory.read_word(external + field),
                exact.memory.read_word(external + field),
                "launched field ${field:02X}, position=({x},{y},{z}), rotation=({pitch},{yaw},{roll})"
            );
        }
        for address in [0x16B1, 0xD767, 0xD769] {
            assert_eq!(
                typed.memory.read_word(address),
                exact.memory.read_word(address),
                "launch state ${address:04X}, position=({x},{y},{z}), rotation=({pitch},{yaw},{roll})"
            );
        }
        assert_eq!(
            typed_path.cursor().offset,
            exact.memory.read_word(current + FIELD_PATH)
        );
    }
}

#[test]
fn player_linked_object_spawn_matches_retail_record_edges() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for (mode, pool_available) in [(0u8, true), (1, true), (31, true), (32, true)] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        let spawned = exact.memory.read_word(FREE_LIST);
        for game in [&mut exact, &mut typed] {
            let player = game.memory.read_word(SELECTED_OBJECT);
            let slot = game.memory.read_word(player + FIELD_PATH);
            game.memory.write_word(PLAYER_ONE, player);
            game.memory.write_byte(0x6C02 + slot, mode);
            game.memory.write_word(current + FIELD_X, 32_700);
            game.memory
                .write_word(current + FIELD_Y, (-32_000i16) as u16);
            game.memory.write_word(current + FIELD_Z, 1_234);
            game.memory.write_byte(current + FIELD_ROT_X, 37);
            game.memory.write_byte(current + FIELD_ROT_Y, 219);
            game.memory.write_byte(current + FIELD_ROT_Z, 17);
            if !pool_available {
                game.memory.write_word(FREE_LIST, 0);
            }
        }

        run_exact_inline(&mut exact, current, 0xE967);
        let typed_path = run_typed_path(&mut typed, 0xE967);
        let should_spawn = mode & 31 != 0 && pool_available;

        let mut mismatches = Vec::new();
        for address in [
            ACTIVE_LIST,
            FREE_LIST,
            CURRENT_OBJECT,
            current,
            current + 1,
            0x1DDF,
            0xD771,
        ] {
            let typed_value = typed.memory.read_byte(address);
            let exact_value = exact.memory.read_byte(address);
            if typed_value != exact_value {
                mismatches.push((address, typed_value, exact_value));
            }
        }
        if should_spawn {
            for offset in 0..OBJECT_STRIDE {
                for address in [spawned + offset, spawned.wrapping_add(0x1CC1 + offset)] {
                    let typed_value = typed.memory.read_byte(address);
                    let exact_value = exact.memory.read_byte(address);
                    if typed_value != exact_value {
                        mismatches.push((address, typed_value, exact_value));
                    }
                }
            }
        }
        assert!(
            mismatches.is_empty(),
            "spawn record differs for mode={mode}, pool_available={pool_available}: {mismatches:02X?}"
        );
        assert_eq!(
            typed_path.cursor().offset,
            exact.memory.read_word(current + FIELD_PATH)
        );
    }
}

#[test]
fn player_auxiliary_target_reset_matches_retail_state_edges() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for initial_flags in [0u8, 0x40, 0x80, 0xFF] {
        let (mut exact, current) = seeded_game(rom.clone());
        let (mut typed, _) = seeded_game(rom.clone());
        for game in [&mut exact, &mut typed] {
            let player = game.memory.read_word(SELECTED_OBJECT);
            let slot = game.memory.read_word(player + FIELD_PATH);
            game.memory.write_word(PLAYER_ONE, player);
            game.memory.write_byte(0x6A8C + slot, initial_flags);
            game.memory.write_word(current + FIELD_X, 32_700);
            game.memory
                .write_word(current + FIELD_Y, (-32_000i16) as u16);
            game.memory.write_word(current + FIELD_Z, 1_234);
            for base in [
                0x6A8D, 0x6A8E, 0x6A8F, 0x6A90, 0x6A92, 0x6A94, 0x6A96, 0x6A98, 0x6BEA, 0x6C1C,
                0x6C24, 0x6C26, 0x6C28, 0x6C29, 0x6C2A, 0x6C2B, 0x6C2C,
            ] {
                game.memory.write_word(base + slot, 0xA55A);
            }
        }

        run_exact_inline(&mut exact, current, 0xF500);
        let typed_path = run_typed_path(&mut typed, 0xF500);
        let player = typed.memory.read_word(PLAYER_ONE);
        let slot = typed.memory.read_word(player + FIELD_PATH);
        for address in [
            0x6A8C + slot,
            0x6A8D + slot,
            0x6A8E + slot,
            0x6A8F + slot,
            0x6A90 + slot,
            0x6A91 + slot,
            0x6A92 + slot,
            0x6A93 + slot,
            0x6A94 + slot,
            0x6A95 + slot,
            0x6A96 + slot,
            0x6A97 + slot,
            0x6A98 + slot,
            0x6A99 + slot,
            0x6BEA + slot,
            0x6C1C + slot,
            0x6C1D + slot,
            0x6C24 + slot,
            0x6C25 + slot,
            0x6C26 + slot,
            0x6C27 + slot,
            0x6C28 + slot,
            0x6C29 + slot,
            0x6C2A + slot,
            0x6C2B + slot,
            0x6C2C + slot,
        ] {
            assert_eq!(
                typed.memory.read_byte(address),
                exact.memory.read_byte(address),
                "auxiliary reset state ${address:04X}, initial_flags=${initial_flags:02X}"
            );
        }
        assert_eq!(
            typed_path.cursor().offset,
            exact.memory.read_word(current + FIELD_PATH)
        );
    }
}
