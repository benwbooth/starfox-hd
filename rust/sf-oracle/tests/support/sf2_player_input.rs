use super::{actor, rom, Source, OWNER, WRAM};
use sf2_game::hit_response::HitSide;
use sf2_game::path_program::SelectedAuxiliaryState;
use sf2_game::player_input::{prepare, PlayerInputSettings};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::view_transition::ViewTransitionMode;
use sf2_game::{Buttons, InputState, ObjectId, ObjectStore, RandomState};

const SLOT: u32 = 64;

fn input(held: u16, pressed: u16) -> InputState {
    InputState {
        held: Buttons::from_bits(held),
        pressed: Buttons::from_bits(pressed),
    }
}

fn world() -> (ObjectStore, ScenePathWorld, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                contact: Some(Default::default()),
                auxiliary: Some(SelectedAuxiliaryState {
                    mode: 0,
                    action_flags: 0,
                    stored_world_position: Default::default(),
                    stored_rotation: Default::default(),
                }),
                injected_input: Some(InputState::default()),
                ..Default::default()
            },
        )
        .unwrap();
    (objects, world, owner)
}

#[allow(clippy::too_many_arguments)]
fn compare(
    source: &mut Source,
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    word: u16,
    mode: u8,
    action_flags: u8,
    protection: u8,
    shared_mode: u16,
    settings: PlayerInputSettings,
) {
    let controllers = [
        input(word, !word),
        input(word.rotate_left(5), word.rotate_right(3)),
    ];
    let injection = input(word.wrapping_mul(37), word ^ 0x956A);
    let before = input(word ^ 0xA5A5, word ^ 0x5A5A);
    let side = if word & 0x100 != 0 {
        HitSide::Secondary
    } else {
        HitSide::Primary
    };
    let flags = ((word as u8) & !0x40) | if side == HitSide::Secondary { 0x40 } else { 0 };
    source.bus.write8(WRAM + u32::from(OWNER) + 0x23, flags);
    source
        .bus
        .write16(WRAM + u32::from(OWNER) + 0x2B, SLOT as u16);
    source.bus.write16(WRAM + 0x1B84, shared_mode);
    source.bus.write8(WRAM + SLOT + 0x6AA0, mode);
    source.bus.write8(WRAM + SLOT + 0x6B77, action_flags);
    source.bus.write8(WRAM + SLOT + 0x6B7D, protection);
    source.bus.write8(WRAM + 0x1DCF, settings.flight_style);
    source.bus.write8(WRAM + 0x1DD0, settings.button_layout);
    for (held, pressed, value) in [
        (0x1292, 0x1296, controllers[0]),
        (0x1294, 0x1298, controllers[1]),
        (0x6A8A + SLOT, 0x6A88 + SLOT, injection),
        (0x1938, 0x1936, before),
        (0x1DA7, 0x1DA9, before),
    ] {
        source.bus.write16(WRAM + held, value.held.bits());
        source.bus.write16(WRAM + pressed, value.pressed.bits());
    }
    objects.get_mut(owner).unwrap().base.contacts.hit_side = side;
    // Deliberately not tied to controller selection.
    world.primary_player = Some(owner);
    world.controller_inputs = controllers.map(Some);
    world.view_transition_mode = Some(ViewTransitionMode { flags: shared_mode });
    world.player_input_settings = Some(settings);
    world.processed_player_input = Some(before);
    world.unmasked_player_input = Some(before);
    let records = world.player_mut(objects, owner).unwrap();
    records.auxiliary.as_mut().unwrap().mode = mode;
    records.auxiliary.as_mut().unwrap().action_flags = action_flags;
    records
        .contact
        .as_mut()
        .unwrap()
        .hit
        .hold_secondary_protection = protection & 0x80 != 0;
    records.injected_input = Some(injection);

    source.run(0x069457, None, 0, OWNER, true);
    let result = prepare(objects, world, owner).unwrap();
    for (held, pressed, native) in [
        (0x1938, 0x1936, result),
        (0x1DA7, 0x1DA9, world.unmasked_player_input.unwrap()),
        (
            0x6A8A + SLOT,
            0x6A88 + SLOT,
            world
                .player(objects, owner)
                .unwrap()
                .injected_input
                .unwrap(),
        ),
        (0x1292, 0x1296, world.controller_inputs[0].unwrap()),
        (0x1294, 0x1298, world.controller_inputs[1].unwrap()),
    ] {
        assert_eq!(
            native.held.bits(),
            source.bus.read16(WRAM + held),
            "{word:04X} mode={mode:02X} settings={settings:?} held={held:04X}"
        );
        assert_eq!(
            native.pressed.bits(),
            source.bus.read16(WRAM + pressed),
            "{word:04X} mode={mode:02X} settings={settings:?} pressed={pressed:04X}"
        );
    }
    assert_eq!(world.processed_player_input, Some(result));
    for (field, value) in [(0x6AA0, mode), (0x6B77, action_flags), (0x6B7D, protection)] {
        assert_eq!(source.bus.read8(WRAM + SLOT + field), value);
    }
    assert_eq!(source.bus.read16(WRAM + 0x1B84), shared_mode);
    assert_eq!(source.bus.read8(WRAM + u32::from(OWNER) + 0x23), flags);
}

#[test]
fn processed_input_matches_original_for_every_word_and_both_control_options() {
    let mut source = Source::new(&rom(), 0xA7);
    let (mut objects, mut world, owner) = world();
    for flight_style in [0x7F, 0x80] {
        for button_layout in [0, 255] {
            for word in 0..=u16::MAX {
                compare(
                    &mut source,
                    &mut objects,
                    &mut world,
                    owner,
                    word,
                    16,
                    word as u8,
                    0x7F,
                    0xA5A5,
                    PlayerInputSettings {
                        flight_style,
                        button_layout,
                    },
                );
            }
        }
    }
}

#[test]
fn processed_input_matches_original_for_every_movement_mode_and_gate_byte() {
    let mut source = Source::new(&rom(), 0x69);
    let (mut objects, mut world, owner) = world();
    for mode in 0..=u8::MAX {
        for flags in 0..=u8::MAX {
            let word = u16::from(mode) | u16::from(flags) << 8;
            compare(
                &mut source,
                &mut objects,
                &mut world,
                owner,
                word,
                mode,
                mode.rotate_left(3),
                flags,
                0xD5A0 | u16::from(flags & 3),
                PlayerInputSettings {
                    flight_style: flags.rotate_left(2),
                    button_layout: mode,
                },
            );
        }
    }
}
