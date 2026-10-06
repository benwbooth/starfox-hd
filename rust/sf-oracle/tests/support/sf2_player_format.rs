//! Common formatter and complete shared storage entry, with original code
//! unchanged. The source is stopped immediately before its final RTS.

use super::*;
use sf2_game::path_appearance::AnimationControl;

const FIRST_STATE: u32 = 0x033F;
const LAST_STATE: u32 = 0x10000;
const FORMATTER: u32 = 0x0682B7;
const FORMATTER_RETURN: u32 = 0x0682E9;

fn read_state(source: &Source) -> Vec<u8> {
    (FIRST_STATE..LAST_STATE)
        .map(|address| source.bus.read8(WRAM + address))
        .collect()
}

fn assert_formatted(
    source: &Source,
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) {
    let native = objects.get(owner).unwrap();
    let address = WRAM + u32::from(OWNER);
    assert_eq!(world.player_display_subject, Some(owner));
    for publication in [0x150D, 0x12C1, 0x1509] {
        assert_eq!(source.bus.read16(WRAM + publication), OWNER);
    }
    assert_eq!(
        native.extension.path_state.animation.shape.packed(),
        source.bus.read8(address + 0x1CCB)
    );
    assert_eq!(native.base.attack_power, source.bus.read8(address + 0x2E));
    assert_eq!(
        native.extension.path_state.hold_latched,
        source.bus.read8(address + 0x09) & 8 != 0
    );
    assert_eq!(
        native.base.contacts.allow_same_shape,
        source.bus.read8(address + 0x22) & 32 != 0
    );
    assert_eq!(
        native.base.flags.casts_shadow,
        source.bus.read8(address + 0x20) & 8 != 0
    );
    assert_eq!(
        native.base.flags.exclude_from_shape_footprint_search,
        source.bus.read8(address + 0x24) & 4 != 0
    );
    assert_eq!(
        native.base.flags.maximum_draw_distance,
        source.bus.read8(address + 0x26) & 16 != 0
    );
}

#[test]
fn formatter_matches_original_for_all_flag_bytes_and_preserves_other_state() {
    let rom = rom();
    let mut source = Source::new(&rom, 0xA7);
    for seed in 0..=u8::MAX {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let other = actor(&mut objects);
        let mut world = ScenePathWorld::new(RandomState::new([1, 2, 3, seed]));
        world.primary_player = Some(other);
        world.fixed_players = [Some(other), Some(owner)];
        world.player_display_subject = Some(other);
        let native = objects.get_mut(owner).unwrap();
        // Each full byte traverses all 256 values; the independent rotations
        // vary the surrounding unrelated flags, not only the targeted bit.
        let flags = [
            seed,
            seed.rotate_left(1),
            seed.rotate_left(2),
            !seed,
            seed.rotate_left(3),
        ];
        for (offset, value) in [0x09, 0x22, 0x20, 0x24, 0x26].into_iter().zip(flags) {
            source.bus.write8(WRAM + u32::from(OWNER) + offset, value);
        }
        native.extension.path_state.hold_latched = flags[0] & 8 != 0;
        native.base.contacts.allow_same_shape = flags[1] & 32 != 0;
        native.base.flags.casts_shadow = flags[2] & 8 != 0;
        native.base.flags.exclude_from_shape_footprint_search = flags[3] & 4 != 0;
        native.base.flags.maximum_draw_distance = flags[4] & 16 != 0;
        native.extension.path_state.animation.shape = AnimationControl::from_packed(seed);
        native.base.attack_power = seed;
        source.bus.write8(WRAM + u32::from(OWNER) + 0x1CCB, seed);
        source.bus.write8(WRAM + u32::from(OWNER) + 0x2E, seed);
        source.bus.write16(WRAM + 0x12C3, OTHER);
        for publication in [0x150D, 0x12C1, 0x1509] {
            source.bus.write16(WRAM + publication, OTHER);
        }
        let mut expected = read_state(&source);
        source.run(FORMATTER, Some(FORMATTER_RETURN), 0, OWNER, true);
        player_storage::format_for_scene(&mut objects, &mut world, owner).unwrap();
        assert_formatted(&source, &objects, &world, owner);
        assert_eq!(world.primary_player, Some(other));
        assert_eq!(world.fixed_players, [Some(other), Some(owner)]);
        assert_eq!(source.bus.read16(WRAM + 0x12C3), OTHER);
        assert_eq!(world.random.bytes(), [1, 2, 3, seed]);
        assert!(objects.get(owner).unwrap().base.player_storage.is_none());

        let native = objects.get(owner).unwrap();
        let write =
            |bytes: &mut [u8], address: u32, value| bytes[(address - FIRST_STATE) as usize] = value;
        for publication in [0x150D, 0x12C1, 0x1509] {
            for (index, byte) in OWNER.to_le_bytes().into_iter().enumerate() {
                write(&mut expected, publication + index as u32, byte);
            }
        }
        let object_address = u32::from(OWNER);
        write(
            &mut expected,
            object_address + 0x1CCB,
            native.extension.path_state.animation.shape.packed(),
        );
        write(
            &mut expected,
            object_address + 0x2E,
            native.base.attack_power,
        );
        for (offset, original, bit, enabled) in [
            (0x09, flags[0], 8, native.extension.path_state.hold_latched),
            (0x22, flags[1], 32, native.base.contacts.allow_same_shape),
            (0x20, flags[2], 8, native.base.flags.casts_shadow),
            (
                0x24,
                flags[3],
                4,
                native.base.flags.exclude_from_shape_footprint_search,
            ),
            (0x26, flags[4], 16, native.base.flags.maximum_draw_distance),
        ] {
            write(
                &mut expected,
                object_address + offset,
                (original & !bit) | if enabled { bit } else { 0 },
            );
        }
        assert_eq!(
            read_state(&source),
            expected,
            "formatter write set seed={seed}"
        );
    }
}

#[test]
fn shared_entry_composes_actual_allocation_with_formatting_before_its_caller_reset() {
    let rom = rom();
    for seed in [0, 1, 4, 5, 127, 128, 254, 255] {
        let mut source = Source::new(&rom, !seed);
        source.run(0x7F1737, None, 0, OWNER, true);
        source.bus.write8(WRAM + 0x1E14, seed);
        source.bus.write8(WRAM + 0x1DD1, seed);
        source.bus.write8(WRAM + u32::from(OWNER) + 0x09, 8);
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut runtime = PathRuntime::default();
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .hold_latched = true;
        source.run(0x068260, Some(FORMATTER_RETURN), 0, OWNER, true);
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: seed,
                reserve_shield: seed,
                score: PlayerScore::default(),
            },
        )
        .unwrap();
        assert_formatted(&source, &objects, &world, owner);
        assert_eq!(runtime.resources.available_capacity(), source.available());
        assert_eq!(runtime.resources.owner_count(owner), 1);
        assert_eq!(world.primary_player, Some(owner));
        assert_eq!(
            world
                .player(&objects, owner)
                .unwrap()
                .contact
                .unwrap()
                .hit
                .reserve_shield,
            seed
        );
        assert_eq!(
            world
                .player(&objects, owner)
                .unwrap()
                .visit
                .unwrap()
                .pilot_code,
            seed
        );
        assert_eq!(
            world.player(&objects, owner).unwrap().target_selection,
            Some(TargetSelection::default())
        );
        assert_eq!(
            player_storage::get(&objects, &runtime.resources, owner)
                .unwrap()
                .retained_shield,
            seed
        );
        assert!(world.action_gate.is_none() && world.processed_player_input.is_none());
        assert!(world.view_transition_mode.is_none());
    }
}
