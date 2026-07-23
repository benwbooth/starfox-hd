#![cfg(feature = "oracle-bridge")]

use sf2_data::path::PathAddress;
use sf2_game::object::*;
use sf2_game::oracle_compat::Game;
use sf2_map::Sf2MapHost;
use sf2_path::{ChildSpawn, PlayerTargetUpdate, Sf2PathCondition, Sf2PathHost, Sf2PathOperation};

#[test]
fn retail_object_pool_formats_all_sixty_stride_3f_records() {
    let game = Game::new(Vec::new()).unwrap();
    assert_eq!(game.memory.read_word(ACTIVE_LIST), 0);
    assert_eq!(game.memory.read_word(FREE_LIST), OBJECT_POOL_BASE);
    for index in 0..OBJECT_COUNT {
        let object = object_address(index);
        assert_eq!(object_index(object), Some(index));
        let expected_next = if index + 1 == OBJECT_COUNT {
            0
        } else {
            object_address(index + 1)
        };
        assert_eq!(game.memory.read_word(object), expected_next);
    }
}

#[test]
fn retail_map_auxiliary_pool_is_separate_and_formats_512_stride_19_records() {
    let game = Game::new(Vec::new()).unwrap();
    assert_eq!(game.memory.read_word(MAP_AUX_ACTIVE_LIST), 0);
    assert_eq!(game.memory.read_word(MAP_AUX_FREE_LIST), MAP_AUX_POOL_BASE);
    for index in 0..MAP_AUX_COUNT {
        let record = MAP_AUX_POOL_BASE + MAP_AUX_STRIDE * index as u16;
        let expected_next = if index + 1 == MAP_AUX_COUNT {
            0
        } else {
            record + MAP_AUX_STRIDE
        };
        assert_eq!(game.memory.read_word(record), expected_next);
    }
}

#[test]
fn map_opcode_90_allocates_only_the_retail_auxiliary_record_pool() {
    let mut game = Game::new(Vec::new()).unwrap();
    let object_free_before = game.memory.read_word(FREE_LIST);
    game.memory.write_byte(0x190E, 0xA5);

    Sf2MapHost::spawn_aux_object(&mut game, 0x1111, 0x2222, 0x3333, 0x44, 0x5555, 0x6666).unwrap();
    let first = MAP_AUX_POOL_BASE;
    assert_eq!(game.memory.read_word(FREE_LIST), object_free_before);
    assert!(game.active_objects().is_empty());
    assert_eq!(active_map_auxiliaries(&game.memory), vec![first]);
    assert_eq!(game.memory.read_word(first + 0x04), 0x1111);
    assert_eq!(game.memory.read_word(first + 0x06), 0x2222);
    assert_eq!(game.memory.read_word(first + 0x08), 0x3333);
    assert_eq!(game.memory.read_byte(first + 0x0A), 0);
    assert_eq!(game.memory.read_byte(first + 0x0B), 0x44);
    assert_eq!(game.memory.read_byte(first + 0x0C), 0);
    assert_eq!(game.memory.read_word(first + 0x0D), 0x5555);
    assert_eq!(game.memory.read_word(first + 0x0F), 0x6666);
    assert_eq!(game.memory.read_byte(first + 0x12), 2);
    assert_eq!(game.memory.read_byte(first + 0x18), 0xA5);

    Sf2MapHost::spawn_aux_object(&mut game, 1, 2, 3, 4, 5, 6).unwrap();
    let second = first + MAP_AUX_STRIDE;
    assert_eq!(active_map_auxiliaries(&game.memory), vec![first, second]);
    assert_eq!(game.memory.read_word(first), second);
    assert_eq!(game.memory.read_word(second + 2), first);
}

#[test]
fn retail_l_add_inserts_after_current_and_preserves_prev_links() {
    let mut game = Game::new(Vec::new()).unwrap();
    let first = allocate(&mut game.memory, 0).unwrap();
    let second = allocate(&mut game.memory, first).unwrap();
    let third = allocate(&mut game.memory, first).unwrap();
    assert_eq!((first, second, third), (0x03BD, 0x03FC, 0x043B));
    assert_eq!(game.memory.read_word(ACTIVE_LIST), first);
    assert_eq!(game.memory.read_word(first + FIELD_NEXT), third);
    assert_eq!(game.memory.read_word(third + FIELD_PREV), first);
    assert_eq!(game.memory.read_word(third + FIELD_NEXT), second);
    assert_eq!(game.memory.read_word(second + FIELD_PREV), third);
    assert_eq!(game.memory.read_word(FREE_LIST), 0x047A);
}

#[test]
fn path_variable_ids_share_exact_object_and_parallel_wram_storage() {
    let mut game = Game::new(Vec::new()).unwrap();
    let object = allocate(&mut game.memory, 0).unwrap();
    game.memory.write_word(CURRENT_OBJECT, object);

    Sf2PathHost::write_variable_word(&mut game, 0x0C, 0x1234).unwrap();
    assert_eq!(game.memory.read_word(object + 0x0C), 0x1234);

    Sf2PathHost::write_variable_word(&mut game, 0xA3, 0xBEEF).unwrap();
    assert_eq!(
        game.memory
            .read_word(object.wrapping_add(0x1C41).wrapping_add(0xA3)),
        0xBEEF
    );
}

#[test]
fn first_retail_map_tick_spawns_real_shape_records_into_the_pool() {
    let mut game = Game::new(Vec::new()).unwrap();
    game.tick(0).unwrap();
    let objects = game.active_objects();
    assert!(!objects.is_empty());
    assert_eq!(objects[0], OBJECT_POOL_BASE);
    assert_eq!(game.memory.read_word(objects[0] + FIELD_SHAPE), 0xBC9C);
    // Both retail selection-ship initializers share `$06:8260`, whose final
    // instance becomes `$12C3` as the active player record.
    assert_eq!(game.memory.read_word(PLAYER_ONE), objects[1]);
    assert_eq!(game.memory.read_word(PLAYER_TWO), 0);
}

#[test]
fn playable_mission_entry_preserves_bootstrapped_players_and_retail_map_pointer() {
    let game = Game::from_playable_root(vec![0; 0x10_0000], 4).unwrap();
    let objects = game.active_objects();
    assert_eq!(objects.len(), 2);
    assert_ne!(game.memory.read_word(PLAYER_ONE), 0);
    for object in objects {
        assert_eq!(game.memory.read_word(object + FIELD_STRATEGY), 0x9C27);
        assert_eq!(game.memory.read_byte(object + FIELD_STRATEGY + 2), 0x06);
    }
    assert_eq!(game.map_cursor(), sf2_data::map::SCRIPT_ROOTS[4].address);
    assert_eq!(game.memory.read_byte(0x192E), 0x05);
    // The map VM fetches from `$8000,X`; retail stores X, not the canonical
    // CPU address used by the extracted catalog.
    assert_eq!(game.memory.read_word(0x1657), 0x00E8);
    assert_eq!(game.memory.read_byte(0x00C4), 2);
    for (anchor, camera) in [(0x034Bu16, 0x00C7u16), (0x034D, 0x00C9), (0x034F, 0x00CB)] {
        assert_eq!(game.memory.read_word(camera), game.memory.read_word(anchor));
    }
    for (anchor, camera) in [(0x0351u16, 0x00CDu16), (0x0353, 0x00CE), (0x0355, 0x00CF)] {
        assert_eq!(game.memory.read_byte(camera), game.memory.read_byte(anchor));
    }
}

#[test]
fn player_initializer_allocates_retail_storage_and_installs_entry_strategy() {
    let mut game = Game::new(vec![0; 0x10_0000]).unwrap();
    let player = allocate(&mut game.memory, 0).unwrap();
    game.memory.write_word(player + FIELD_STRATEGY, 0x82F9);
    game.memory.write_byte(player + FIELD_STRATEGY + 2, 0x06);
    game.memory.write_byte(0x1DD1, 0x20);
    game.memory.write_byte(0x1DD5, 0x10);

    game.initialize_player_strategy(player).unwrap();
    let slot = game.memory.read_word(player + FIELD_PATH);
    assert_ne!(slot, 0);
    assert_eq!(game.memory.read_word(PLAYER_ONE), player);
    assert_eq!(game.memory.read_word(0x1E24), slot);
    assert_eq!(game.memory.read_word(player + FIELD_STRATEGY), 0x845C);
    assert_eq!(game.memory.read_byte(player + FIELD_STRATEGY + 2), 6);
    assert_eq!(game.memory.read_byte(player + 0x2D), 1);
    assert_eq!(game.memory.read_byte(player + 0x2E), 0);
    assert_eq!(game.memory.read_byte(player + 0x1CCB), 0x80);
    assert_eq!(game.memory.read_byte(0x6BBA + slot), 0xFF);
    assert_eq!(game.memory.read_byte(0x6BBB + slot), 0xFF);
    assert_eq!(game.memory.read_byte(0x6BC2 + slot), 0xC0);
    assert_eq!(game.memory.read_byte(0x1D73), 0xFE);
    assert_eq!(game.memory.read_word(0x1DF9), 0x0320);
    assert_eq!(game.memory.read_word(0x1DFB), 0xFFF6);
}

#[test]
fn draw_records_are_built_from_the_same_retail_object_bytes() {
    let mut game = Game::new(Vec::new()).unwrap();
    let object = allocate(&mut game.memory, 0).unwrap();
    game.memory.write_word(object + FIELD_SHAPE, 0xEA00);
    game.memory.write_byte(object + 0x20, 0x08);
    game.memory.write_word(object + FIELD_X, (-12i16) as u16);
    game.memory.write_word(object + FIELD_Y, 34);
    game.memory.write_word(object + FIELD_Z, (-56i16) as u16);
    game.memory.write_byte(object + FIELD_ROT_Y, 0x40);

    let records = game.draw_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].0, object);
    assert_eq!(records[0].1.shape, 0xEA00);
    assert_eq!(
        (records[0].1.x, records[0].1.y, records[0].1.z),
        (-12, 34, -56)
    );
    assert_eq!(records[0].1.rotation_y, 0x40);
}

#[test]
fn map_slot_configuration_uses_the_retail_sparse_overlapping_layout() {
    let mut game = Game::new(Vec::new()).unwrap();
    Sf2MapHost::configure_slot(
        &mut game,
        2,
        false,
        [0xFC, 0x14, 0x08, 0x0C, 0xE2, 0x0A, 0x05],
    )
    .unwrap();

    let base = 0x686A + 2 * 0x10;
    assert_eq!(game.memory.read_word(base), 0xFC00);
    assert_eq!(game.memory.read_word(base + 4), 0x1400);
    assert_eq!(game.memory.read_word(base + 6), 0x0800);
    assert_eq!(game.memory.read_word(base + 0x0A), 0x0C00);
    assert_eq!(game.memory.read_word(base + 0x0C), 0x0AE2);
    assert_eq!(game.memory.read_word(base + 0x0D), 0x050A);
    assert_eq!(game.memory.read_byte(base + 0x0F), 2);
    assert_eq!(game.memory.read_byte(0x1910), 1);

    Sf2MapHost::configure_slot(&mut game, 3, true, [0; 7]).unwrap();
    assert_eq!(game.memory.read_byte(0x686A + 3 * 0x10 + 0x0F), 0);
    assert_eq!(game.memory.read_byte(0x1910), 2);
}

#[test]
fn strategic_map_auxiliary_calls_write_the_exact_pilot_slot_arrays() {
    let mut game = Game::new(Vec::new()).unwrap();
    let player = allocate(&mut game.memory, 0).unwrap();
    game.memory.write_word(PLAYER_ONE, player);
    game.memory.write_word(player + FIELD_PATH, 3);
    game.memory.write_word(0x08, 0x1234);
    game.memory.write_word(0x0A, 0x5678);
    game.memory.write_byte(0x02, 0x9A);
    game.memory.write_byte(0x04, 0xBC);
    game.memory.write_byte(0xA7, 0xDE);

    Sf2MapHost::call_65816(&mut game, 0x069A2F, None).unwrap();
    assert_eq!(game.memory.read_word(0x6BF5 + 3), 0x1234);
    assert_eq!(game.memory.read_word(0x6BF7 + 3), 0x5678);
    assert_eq!(game.memory.read_byte(0x6BF9 + 3), 0x9A);
    assert_eq!(game.memory.read_byte(0x6BFA + 3), 0xBC);

    Sf2MapHost::call_65816(&mut game, 0x069A5F, None).unwrap();
    assert_eq!(game.memory.read_word(0x6BFB + 3), 0x1234);
    assert_eq!(game.memory.read_byte(0x6B49 + 3), 0x78);
    assert_eq!(game.memory.read_byte(0x6BFD + 3), 0x9A);
    assert_eq!(game.memory.read_byte(0x6BFE + 3), 0xBC);
    assert_eq!(game.memory.read_byte(0x6B59 + 3), 0xDE);
}

#[test]
fn strategic_map_player_placement_matches_retail_cardinal_math() {
    let mut game = Game::new(Vec::new()).unwrap();
    let player = allocate(&mut game.memory, 0).unwrap();
    let target = allocate(&mut game.memory, player).unwrap();
    game.memory.write_word(PLAYER_ONE, player);
    game.memory.write_word(0x14D6, target);
    game.memory.write_word(player + FIELD_PATH, 5);
    game.memory.write_word(0x1DE4, 0);
    game.memory.write_word(0x1DE8, 0);
    game.memory.write_word(0x1DF3, 0);
    game.memory.write_word(0x1DF5, 1000);
    game.memory.write_byte(0x1DEA, 0x30);

    Sf2MapHost::call_65816(&mut game, 0x069B3C, None).unwrap();
    assert_eq!(game.memory.read_word(0x1DC0), 0x4650);
    assert_eq!(game.memory.read_word(player + FIELD_X) as i16, 0);
    assert_eq!(game.memory.read_word(player + FIELD_Y) as i16, 0);
    assert_eq!(game.memory.read_word(player + FIELD_Z) as i16, -17859);
    assert_eq!(game.memory.read_word(target + FIELD_X), 0);
    assert_eq!(game.memory.read_word(target + FIELD_Y), 0);
    assert_eq!(game.memory.read_word(target + FIELD_Z), 1000);
    assert_eq!(game.memory.read_byte(player + FIELD_ROT_Y), 0x30);
    assert_eq!(game.memory.read_byte(0x6AE6 + 5), 0x16);
    assert_eq!(game.memory.read_byte(0x6ABC + 5), 0x30);
    assert_eq!(game.memory.read_byte(0x6B34 + 5), 0xD0);
    assert_eq!(game.memory.read_byte(0x0354), 0xD0);
    assert_eq!(game.memory.read_byte(0x1D98), 1);
}

#[test]
fn strategic_map_marker_call_rasterizes_and_erases_the_retail_bitplane() {
    let mut game = Game::new(Vec::new()).unwrap();
    Sf2MapHost::configure_slot(
        &mut game,
        0,
        false,
        [0xFC, 0x14, 0x08, 0x0C, 0xA8, 0x09, 0x05],
    )
    .unwrap();

    Sf2MapHost::call_65816(&mut game, 0x0DDA7A, Some(0x80)).unwrap();
    for row in 0..6u16 {
        assert_eq!(game.memory.read_byte(0xCF36 + 0x00AF + row * 0x10), 0xC0);
        assert_eq!(game.memory.read_byte(0xCF36 + 0x00A0 + row * 0x10), 0x03);
    }
    assert_eq!(game.map_markers.last().unwrap().kind, 0x80);
    assert_eq!(game.map_markers.last().unwrap().table_index, 0);

    Sf2MapHost::call_65816(&mut game, 0x0DDA7A, Some(0x00)).unwrap();
    for row in 0..6u16 {
        assert_eq!(game.memory.read_byte(0xCF36 + 0x00AF + row * 0x10), 0);
        assert_eq!(game.memory.read_byte(0xCF36 + 0x00A0 + row * 0x10), 0);
    }
}

#[test]
fn allocated_objects_receive_the_retail_formatter_defaults() {
    let mut game = Game::new(Vec::new()).unwrap();
    game.memory.write_word(0x1B84, 0x0002);
    game.memory.write_byte(0x190E, 0x5A);
    let object = allocate(&mut game.memory, 0).unwrap();
    assert_eq!(game.memory.read_byte(object + 0x08), 0x10);
    assert_eq!(game.memory.read_byte(object + 0x09), 0x08);
    assert_eq!(game.memory.read_byte(object + 0x22), 0x04);
    assert_eq!(game.memory.read_byte(object + 0x26), 0x08);
    assert_eq!(game.memory.read_byte(object + 0x31), 0x04);
    assert_eq!(game.memory.read_byte(object.wrapping_add(0x1CF0)), 0x5A);
}

#[test]
fn retail_child_chain_uses_fields_29_13_and_hierarchy_flags() {
    let mut game = Game::new(Vec::new()).unwrap();
    let mother = allocate(&mut game.memory, 0).unwrap();
    game.memory.write_word(CURRENT_OBJECT, mother);
    Sf2PathHost::spawn_child(
        &mut game,
        ChildSpawn {
            shape: 0xBC9C,
            path: PathAddress { offset: 0x8D81 },
            rotation: [1, 2, 3],
            hit_points: 4,
            attack_points: 5,
            offset: [-6, 7, -8],
            child_number: 9,
        },
    )
    .unwrap();
    let child = game.memory.read_word(mother + 0x29);
    assert!(object_index(child).is_some());
    assert_eq!(game.memory.read_word(CURRENT_OBJECT), mother);
    assert_eq!(game.memory.read_word(0xD771), child);
    assert_eq!(game.memory.read_word(child + 0x06), mother);
    assert_eq!(game.memory.read_byte(child + 0x13), 9);
    assert_eq!(game.memory.read_byte(child + 0x2D), 4);
    assert_eq!(game.memory.read_byte(child + 0x2E), 5);
    assert_eq!(game.memory.read_word(child + FIELD_STRATEGY), 0x7E1E);
    assert_eq!(game.memory.read_byte(child + FIELD_STRATEGY + 2), 0x7F);
    assert_ne!(game.memory.read_byte(child + 0x31) & 0x10, 0);
    assert_ne!(game.memory.read_byte(mother + 0x23) & 0x10, 0);
    assert_ne!(game.memory.read_byte(child + 0x23) & 0x04, 0);
    assert_ne!(game.memory.read_byte(child + 0x25) & 0x01, 0);
    assert_eq!(game.memory.read_word(child.wrapping_add(0x1CD8)), mother);
    assert_eq!(game.memory.read_word(child.wrapping_add(0x1CCF)) as i16, -6);

    game.memory.write_word(CURRENT_OBJECT, mother);
    assert!(!Sf2PathHost::child_is_dead(&mut game, 9).unwrap());
    Sf2PathHost::flag_child(&mut game, 9).unwrap();
    assert_ne!(game.memory.read_byte(child + 0x23) & 0x08, 0);
    Sf2PathHost::remove_child(&mut game, 9).unwrap();
    assert_ne!(game.memory.read_byte(child + 0x25) & 0x08, 0);
}

#[test]
fn path_end_returns_the_object_to_the_retail_pool_after_the_stable_frame_walk() {
    let mut game = Game::new(Vec::new()).unwrap();
    let object = allocate(&mut game.memory, 0).unwrap();
    game.memory.write_word(object + FIELD_PATH, 0xB09F);
    game.memory.write_word(object + FIELD_STRATEGY, 0x7E53);
    game.memory.write_byte(object + FIELD_STRATEGY + 2, 0x7F);

    game.tick(0).unwrap();

    assert!(!game.active_objects().contains(&object));
    assert_eq!(game.memory.read_word(FREE_LIST), object);
}

#[test]
fn yielding_goto_persists_its_target_in_the_object_path_field() {
    let mut game = Game::new(Vec::new()).unwrap();
    let object = allocate(&mut game.memory, 0).unwrap();
    game.memory.write_word(object + FIELD_PATH, 0xF521);
    game.memory.write_word(object + FIELD_STRATEGY, 0x7E53);
    game.memory.write_byte(object + FIELD_STRATEGY + 2, 0x7F);

    game.tick(0).unwrap();

    assert_eq!(game.memory.read_word(object + FIELD_PATH), 0xF527);
}

#[test]
fn native_player_exhaust_root_is_present_in_the_certified_path_catalog() {
    let command = sf2_path::command_at(PathAddress { offset: 0xF536 })
        .expect("native-installed exhaust path must be extracted");
    assert_eq!(command.opcode, 0x04D);
    assert_eq!(
        &command.raw[..command.raw_len as usize],
        &[0x4D, 0x00, 0x00]
    );
}

#[test]
#[cfg(feature = "oracle-bridge")]
fn playable_autopilot_does_not_exhaust_the_retail_object_pool() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace has a repository parent");
    let rom_path = repo.join("Star Fox 2 (USA, Europe).sfc");
    let Ok(rom) = std::fs::read(&rom_path) else {
        eprintln!("skip: no retail SF2 ROM at {}", rom_path.display());
        return;
    };
    let mut game = Game::from_playable_root(rom, 4).unwrap();
    let mut maximum_objects = 0;
    for frame in 0..300 {
        let pad = if frame <= 400 && frame % 60 == 0 {
            sf_core::pad::START
        } else {
            0
        };
        game.tick(pad)
            .unwrap_or_else(|error| panic!("frame {frame}: {error:?}"));
        maximum_objects = maximum_objects.max(game.active_objects().len());
        assert_ne!(
            game.memory.read_word(FREE_LIST),
            0,
            "frame {frame} exhausted the 60-record pool"
        );
    }
    assert!(maximum_objects < OBJECT_COUNT);
}

#[test]
fn find_shape_and_shape_dead_share_the_current_object_link_field() {
    let mut game = Game::new(Vec::new()).unwrap();
    let current = allocate(&mut game.memory, 0).unwrap();
    let target = allocate(&mut game.memory, current).unwrap();
    game.memory.write_word(CURRENT_OBJECT, current);
    game.memory.write_word(target + FIELD_SHAPE, 0xD00D);
    Sf2PathHost::find_shape(&mut game, 0xD00D).unwrap();
    assert_eq!(game.memory.read_word(current + 0x06), target);
    assert!(!Sf2PathHost::pointed_shape_is_dead(&game).unwrap());
    Sf2PathHost::find_shape(&mut game, 0xFFFF).unwrap();
    assert!(Sf2PathHost::pointed_shape_is_dead(&game).unwrap());
}

#[test]
fn selected_slot_nibble_uses_the_sparse_pilot_array_and_allows_null() {
    let mut game = Game::new(Vec::new()).unwrap();
    let current = allocate(&mut game.memory, 0).unwrap();
    let selected = allocate(&mut game.memory, current).unwrap();
    game.memory.write_word(CURRENT_OBJECT, current);
    game.memory.write_word(SELECTED_OBJECT, selected);
    game.memory.write_word(selected + FIELD_PATH, 0x0012);
    game.memory.write_byte(0x6AA1 + 0x12, 0xB9);
    Sf2PathHost::set_selected_slot_low_nibble_4(&mut game).unwrap();
    assert_eq!(game.memory.read_byte(0x6AA1 + 0x12), 0xB4);
    assert_eq!(game.memory.read_word(selected + FIELD_PATH), 0x0012);

    game.memory.write_word(SELECTED_OBJECT, 0);
    game.memory.write_word(FIELD_PATH, 0x0020);
    game.memory.write_byte(0x6AA1 + 0x20, 0x6F);
    Sf2PathHost::set_selected_slot_low_nibble_4(&mut game).unwrap();
    assert_eq!(game.memory.read_byte(0x6AA1 + 0x20), 0x64);
}

#[test]
fn sf2_random_uses_the_retail_four_byte_subtract_chain() {
    let mut game = Game::new(Vec::new()).unwrap();
    for (address, value) in [(0xE0, 0x11), (0xE1, 0x28), (0xE2, 0xE9), (0xE3, 0x9B)] {
        game.memory.write_byte(address, value);
    }
    assert_eq!(Sf2PathHost::random_byte(&mut game).unwrap(), 0x51);
    assert_eq!(
        [
            game.memory.read_byte(0xE0),
            game.memory.read_byte(0xE1),
            game.memory.read_byte(0xE2),
            game.memory.read_byte(0xE3),
        ],
        [0x51, 0xE8, 0xFE, 0x62]
    );
}

#[test]
fn auxiliary_heap_uses_retail_end_split_chain_growth_and_coalescing() {
    let mut game = Game::new(Vec::new()).unwrap();
    let object = allocate(&mut game.memory, 0).unwrap();
    game.memory.write_word(CURRENT_OBJECT, object);

    assert_eq!(game.memory.read_word(AUX_HEAP_BASE), 2);
    assert_eq!(game.memory.read_word(AUX_HEAP_BASE + 6), 0x47FE);

    Sf2PathHost::allocate_auxiliary_type_0b(&mut game, 0x55).unwrap();
    let first_base = game.memory.read_word(object + 0x1CEC);
    assert_eq!(first_base, 0x47FA);
    assert_eq!(read_auxiliary_byte(&game.memory, first_base), 1);
    assert_eq!(read_auxiliary_byte(&game.memory, first_base + 1), 0x0B);
    assert_eq!(read_auxiliary_byte(&game.memory, first_base + 2), 0x55);

    Sf2PathHost::allocate_auxiliary_type_0d(&mut game, 0xA6).unwrap();
    let grown_base = game.memory.read_word(object + 0x1CEC);
    assert_eq!(grown_base, 0x47EC);
    assert_eq!(read_auxiliary_byte(&game.memory, grown_base), 2);
    assert_eq!(read_auxiliary_byte(&game.memory, grown_base + 1), 0x0B);
    assert_eq!(read_auxiliary_byte(&game.memory, grown_base + 2), 0x55);
    assert_eq!(read_auxiliary_byte(&game.memory, grown_base + 5), 0x0D);
    assert_eq!(read_auxiliary_byte(&game.memory, grown_base + 6), 0xA6);

    free_all_auxiliary(&mut game.memory, object);
    assert_eq!(game.memory.read_word(object + 0x1CDC), 0);
    assert_eq!(game.memory.read_word(object + 0x1CEC), 0);
    assert_eq!(game.memory.read_word(AUX_HEAP_BASE), 2);
    assert_eq!(game.memory.read_word(AUX_HEAP_BASE + 6), 0x47FE);
}

#[test]
fn player_aux_mode_round_trips_anchor_record_and_queues_retail_events() {
    let mut game = Game::new(Vec::new()).unwrap();
    let player = allocate(&mut game.memory, 0).unwrap();
    let invalidated = allocate(&mut game.memory, player).unwrap();
    game.memory.write_word(PLAYER_ONE, player);
    game.memory.write_word(CURRENT_OBJECT, player);
    game.memory.write_byte(invalidated + 0x31, 0x08);
    for offset in 0..0x3Fu16 {
        game.memory
            .write_byte(0x033F + offset, (offset as u8).wrapping_mul(3));
    }

    Sf2PathHost::perform_path_operation(&mut game, Sf2PathOperation::SetPlayerAuxMode(true))
        .unwrap();
    assert_ne!(game.memory.read_word(0x1B84) & 2, 0);
    assert_ne!(game.memory.read_byte(player + 0x26) & 8, 0);
    assert_ne!(game.memory.read_byte(invalidated + 0x21) & 1, 0);
    let entry = find_auxiliary_type(&game.memory, player, 8).unwrap();
    let block = read_auxiliary_word(&game.memory, entry + 1);
    assert_eq!(read_auxiliary_byte(&game.memory, block + 0x21), 0x63);
    assert_eq!(game.memory.read_word(0x1CF6), 0x00F8);
    assert_eq!(game.memory.read_byte(0x1D16), 2);

    write_auxiliary_byte(&mut game.memory, block + 0x21, 0xD4);
    Sf2PathHost::perform_path_operation(&mut game, Sf2PathOperation::SetPlayerAuxMode(false))
        .unwrap();
    assert_eq!(game.memory.read_word(0x1B84) & 2, 0);
    assert_eq!(game.memory.read_byte(player + 0x26) & 8, 0);
    assert_eq!(game.memory.read_byte(0x033F + 0x21), 0xD4);
    assert_eq!(game.memory.read_word(0x1CF8), 0x00F7);
    assert_eq!(game.memory.read_byte(0x1D16), 4);
}

#[test]
fn path_hold_installs_the_retail_hold_strategy() {
    let mut game = Game::new(Vec::new()).unwrap();
    let object = allocate(&mut game.memory, 0).unwrap();
    game.memory.write_word(CURRENT_OBJECT, object);
    Sf2PathHost::enter_path_hold(&mut game).unwrap();
    assert_ne!(game.memory.read_byte(object + 0x09) & 0x08, 0);
    assert_eq!(game.memory.read_word(object + FIELD_STRATEGY), 0x9DDE);
    assert_eq!(game.memory.read_byte(object + FIELD_STRATEGY + 2), 0x7F);
}

#[test]
fn fire_weapon_uses_the_reachable_retail_dispatch_and_formatter() {
    let mut game = Game::new(Vec::new()).unwrap();
    let source = allocate(&mut game.memory, 0).unwrap();
    let player = allocate(&mut game.memory, source).unwrap();
    game.memory.write_word(CURRENT_OBJECT, source);
    game.memory.write_word(PLAYER_ONE, player);
    game.memory.write_word(player + FIELD_PATH, 6);
    game.memory.write_byte(0x6AA0 + 6, 0);
    for (field, value) in [(FIELD_X, 100u16), (FIELD_Y, 200), (FIELD_Z, 300)] {
        game.memory.write_word(source + field, value);
    }
    for (field, value) in [
        (FIELD_ROT_X, 0x10),
        (FIELD_ROT_Y, 0x20),
        (FIELD_ROT_Z, 0x30),
    ] {
        game.memory.write_byte(source + field, value);
    }
    game.memory.write_byte(source + 0x18, 0x25);
    game.memory.write_byte(source + 0x2F, 0x14);

    Sf2PathHost::fire_weapon(&mut game).unwrap();
    let weapon = game.memory.read_word(0xD771);
    assert!(object_index(weapon).is_some());
    assert_eq!(game.memory.read_word(CURRENT_OBJECT), source);
    assert_eq!(game.memory.read_word(weapon + FIELD_SHAPE), 0xBC9C);
    assert_eq!(game.memory.read_word(weapon + FIELD_PATH), 0xEE4C);
    assert_eq!(game.memory.read_word(weapon + FIELD_STRATEGY), 0x7E1E);
    assert_eq!(game.memory.read_byte(weapon + FIELD_STRATEGY + 2), 0x7F);
    assert_eq!(game.memory.read_byte(weapon + 0x18), 0x46);
    assert_eq!(game.memory.read_word(weapon + 0x1C), source);
    assert_eq!(game.memory.read_word(source + 0x1C), weapon);
    assert_eq!(game.memory.read_byte(weapon + 0x31) & 0x52, 0x52);
    assert_eq!(game.memory.read_byte(weapon.wrapping_add(0x1CCB)), 0x80);
}

#[test]
fn recovered_pilot_services_write_the_retail_slot_layout() {
    let mut game = Game::new(Vec::new()).unwrap();
    let current = allocate(&mut game.memory, 0).unwrap();
    let player = allocate(&mut game.memory, current).unwrap();
    game.memory.write_word(CURRENT_OBJECT, current);
    game.memory.write_word(PLAYER_ONE, player);
    game.memory.write_word(player + FIELD_PATH, 4);
    for (field, value) in [(FIELD_X, 0x1111), (FIELD_Y, 0x2222), (FIELD_Z, 0x3333)] {
        game.memory.write_word(current + field, value);
    }

    Sf2PathHost::perform_path_operation(
        &mut game,
        Sf2PathOperation::ConfigurePilotAuxModeA(0xFF81),
    )
    .unwrap();
    assert_eq!(game.memory.read_word(0x6A90 + 4), 0xFF02);
    assert_eq!(game.memory.read_word(0x6C26 + 4), 1);
    assert_eq!(game.memory.read_word(0x6A98 + 4), current);
    assert_eq!(game.memory.read_byte(0x6A8D + 4), 1);
    assert_eq!(game.memory.read_byte(0x6A8E + 4), 2);
    assert_eq!(game.memory.read_byte(0x6A8F + 4), 2);
    assert_eq!(game.memory.read_word(0x6A92 + 4), 0x1111);
    assert_eq!(game.memory.read_word(0x6A94 + 4), 0x2222);
    assert_eq!(game.memory.read_word(0x6A96 + 4), 0x3333);

    game.memory.write_word(0x6C1C + 4, 8);
    game.memory.write_byte(0x6C00 + 4, 1);
    Sf2PathHost::perform_path_operation(&mut game, Sf2PathOperation::UpdatePilotAuxState).unwrap();
    assert_eq!(game.memory.read_byte(0x6C11 + 4), 4);
    assert_eq!(game.memory.read_byte(0x6C12 + 4), 0x24);

    game.memory.write_word(0x6BBC + 4, 0xFFFF);
    Sf2PathHost::update_player_target(&mut game, PlayerTargetUpdate::Flag08).unwrap();
    assert_eq!(game.memory.read_word(SELECTED_OBJECT), 0);
    assert_eq!(game.memory.read_word(0x6BB8 + 4), current);
    assert_ne!(game.memory.read_byte(0x6BC2 + 4) & 8, 0);
}

#[test]
fn force_trigger_expansion_services_match_retail_state_transitions() {
    let mut game = Game::new(Vec::new()).unwrap();
    let current = allocate(&mut game.memory, 0).unwrap();
    let selected = allocate(&mut game.memory, current).unwrap();
    let linked = allocate(&mut game.memory, current).unwrap();
    game.memory.write_word(CURRENT_OBJECT, current);
    game.memory.write_word(SELECTED_OBJECT, selected);
    game.memory.write_word(selected + FIELD_PATH, 4);

    game.memory.write_word(current + 0x1CD8, 0xCAFE);
    Sf2PathHost::perform_path_operation(&mut game, Sf2PathOperation::ClearObjectRelativeReference)
        .unwrap();
    assert_eq!(game.memory.read_word(current + 0x1CD8), 0);

    game.memory.write_word(current + FIELD_PATH, 0x1683);
    game.memory.write_word(current + 0x1CE6, linked);
    Sf2PathHost::perform_path_operation(
        &mut game,
        Sf2PathOperation::PreserveCurrentPathContinuation,
    )
    .unwrap();
    assert_eq!(game.memory.read_word(linked + 0x0F), 0x1683);

    game.memory.write_word(current + FIELD_PATH, 0x4321);
    game.memory.write_word(current + 0x1CE6, 0);
    Sf2PathHost::perform_path_operation(
        &mut game,
        Sf2PathOperation::PreserveCurrentPathContinuation,
    )
    .unwrap();
    let continuation = find_auxiliary_type(&game.memory, current, 3).unwrap();
    assert_eq!(read_auxiliary_word(&game.memory, continuation + 1), 0x4321);

    game.memory.write_byte(0x6C06 + 4, 2);
    Sf2PathHost::perform_path_operation(
        &mut game,
        Sf2PathOperation::IncrementSelectedAuxiliaryStage,
    )
    .unwrap();
    assert_eq!(game.memory.read_byte(0x6C06 + 4), 3);
    Sf2PathHost::perform_path_operation(
        &mut game,
        Sf2PathOperation::IncrementSelectedAuxiliaryStage,
    )
    .unwrap();
    assert_eq!(game.memory.read_byte(0x6C06 + 4), 3);

    game.memory.write_byte(0x1DD5, 7);
    game.memory.write_byte(0x6C00 + 4, 7);
    assert!(Sf2PathHost::evaluate_path_condition(
        &mut game,
        Sf2PathCondition::SelectedAuxiliaryStateMatchesGlobal,
    )
    .unwrap());

    game.memory.write_byte(current + 0x1CE3, 5);
    game.memory.write_byte(0x6C05 + 4, 5);
    game.memory.write_byte(0x6C04 + 4, 0xA8);
    game.memory.write_byte(current + FIELD_Z, 0x6B);
    assert!(!Sf2PathHost::advance_selected_auxiliary_progress(&mut game, 1).unwrap());
    assert_eq!(game.memory.read_byte(0x6C04 + 4), 0xA9);
    assert_eq!(game.memory.read_byte(0x1E03), 1);
    assert_eq!(game.memory.read_byte(0x1E05), 0x6B);
    assert!(Sf2PathHost::advance_selected_auxiliary_progress(&mut game, 1).unwrap());
    game.memory.write_byte(current + 0x1CE3, 6);
    assert!(!Sf2PathHost::advance_selected_auxiliary_progress(&mut game, 1).unwrap());
    assert_eq!(game.memory.read_byte(0x6C05 + 4), 6);

    Sf2PathHost::perform_path_operation(
        &mut game,
        Sf2PathOperation::PreserveCurrentObjectForParent,
    )
    .unwrap();
    assert_eq!(game.memory.read_word(0xD767), current);

    game.memory.write_word(current + 0x32, 3);
    game.memory.write_word(current + 0x36, 0x4001);
    Sf2PathHost::perform_path_operation(&mut game, Sf2PathOperation::ScaleHorizontalMotion)
        .unwrap();
    assert_eq!(game.memory.read_word(current + 0x32), 24);
    assert_eq!(game.memory.read_word(current + 0x36), 8);
}

#[test]
fn external_strategy_uses_the_retail_rom_pair_table() {
    let mut rom = vec![0; 0x10_0000];
    let offset = 6 * 0x8000 + 0x0135 + 3 * 4;
    rom[offset..offset + 4].copy_from_slice(&[0x34, 0x12, 0x78, 0x56]);
    let mut game = Game::new(rom).unwrap();
    let current = allocate(&mut game.memory, 0).unwrap();
    game.memory.write_word(CURRENT_OBJECT, current);
    Sf2PathHost::perform_path_operation(&mut game, Sf2PathOperation::CallExternalStrategy(3))
        .unwrap();
    assert_eq!(game.memory.read_word(current + FIELD_SHAPE), 0x1234);
    assert_eq!(game.memory.read_word(current.wrapping_add(0x1CCD)), 0x5678);
}
