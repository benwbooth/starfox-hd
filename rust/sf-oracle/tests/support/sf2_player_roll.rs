//! Original shoulder and barrel-roll bodies, including a continuous input
//! preparation sequence. The intervening steering/movement code is not run
//! or represented as complete by this boundary test.

use super::{actor, rom, Source, OWNER, WRAM};
use sf2_game::hit_response::HitSide;
use sf2_game::path_program::SelectedAuxiliaryState;
use sf2_game::path_protection::DeflectionProtection;
use sf2_game::player_input::{self, PlayerInputSettings};
use sf2_game::player_roll::{self, PlayerRoll, ShoulderControl};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::view_transition::ViewTransitionMode;
use sf2_game::{Button, Buttons, InputState, ObjectId, ObjectStore, RandomState};

const SLOT: u32 = 64;

fn fixture(source: &mut Source) -> (ObjectStore, ScenePathWorld, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                roll: Some(PlayerRoll::default()),
                protection: Some(DeflectionProtection::default()),
                auxiliary: Some(SelectedAuxiliaryState {
                    mode: 0x11,
                    action_flags: 1,
                    stored_world_position: Default::default(),
                    stored_rotation: Default::default(),
                }),
                contact: Some(Default::default()),
                injected_input: Some(Default::default()),
                ..Default::default()
            },
        )
        .unwrap();
    source
        .bus
        .write16(WRAM + u32::from(OWNER) + 0x2B, SLOT as u16);
    (objects, world, owner)
}

fn set_input(source: &mut Source, world: &mut ScenePathWorld, held: u16, pressed: u16) {
    source.bus.write16(WRAM + 0x1938, held);
    source.bus.write16(WRAM + 0x1936, pressed);
    world.processed_player_input = Some(InputState {
        held: Buttons::from_bits(held),
        pressed: Buttons::from_bits(pressed),
    });
}

fn seed(
    source: &mut Source,
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    roll: PlayerRoll,
    protection: u8,
    action: u8,
) {
    for (field, value) in [
        (0x6B7E, roll.shoulders.bits()),
        (0x6ADC, roll.tap_window),
        (0x6ADD, roll.impulse as u8),
        (0x6C02, protection),
        (0x6B77, action),
    ] {
        source.bus.write8(WRAM + SLOT + field, value);
    }
    let records = world.player_mut(objects, owner).unwrap();
    records.roll = Some(roll);
    records.protection = Some(DeflectionProtection::from_control(protection));
    records.auxiliary.as_mut().unwrap().action_flags = action;
}

fn compare(source: &Source, objects: &ObjectStore, world: &ScenePathWorld, owner: ObjectId) {
    let records = world.player(objects, owner).unwrap();
    let roll = records.roll.unwrap();
    for (field, value) in [
        (0x6B7E, roll.shoulders.bits()),
        (0x6ADC, roll.tap_window),
        (0x6ADD, roll.impulse as u8),
        (0x6C02, records.protection.unwrap().control()),
        (0x6B77, records.auxiliary.unwrap().action_flags),
    ] {
        assert_eq!(
            value,
            source.bus.read8(WRAM + SLOT + field),
            "field={field:04X} roll={roll:?}"
        );
    }
}

fn run_roll(source: &mut Source) {
    let spinning = source.bus.read8(WRAM + SLOT + 0x6ADD) != 0;
    // Local RTS helper: stop at its actual return, without changing any
    // original opcode or forcing a branch to match the native result.
    source.run(
        0x06E7FF,
        Some(if spinning { 0x06E8F4 } else { 0x06E8C8 }),
        0,
        OWNER,
        true,
    );
}

#[test]
fn shoulders_match_original_every_retained_control_and_input_combination() {
    let mut source = Source::new(&rom(), 0xA5);
    let (objects, mut world, owner) = fixture(&mut source);
    for control in 0..=u8::MAX {
        for pressed in 0..4u16 {
            for held in 0..4u16 {
                seed(
                    &mut source,
                    &objects,
                    &mut world,
                    owner,
                    PlayerRoll {
                        shoulders: ShoulderControl::from_bits(control),
                        tap_window: 233,
                        impulse: -28,
                    },
                    0xAD,
                    0xFE,
                );
                set_input(
                    &mut source,
                    &mut world,
                    held * 16 | 0xFFCF,
                    pressed * 16 | 0xFFCF,
                );
                source.run(0x069075, None, 0, OWNER, true);
                player_roll::prepare_shoulders(&objects, &mut world, owner).unwrap();
                compare(&source, &objects, &world, owner);
            }
        }
    }
}

#[test]
fn roll_matches_original_every_impulse_and_protection_byte_and_all_tap_branches() {
    let mut source = Source::new(&rom(), 0xA5);
    let (objects, mut world, owner) = fixture(&mut source);
    for impulse in 1..=u8::MAX {
        for protection in 0..=u8::MAX {
            seed(
                &mut source,
                &objects,
                &mut world,
                owner,
                PlayerRoll {
                    shoulders: ShoulderControl::from_bits(protection),
                    tap_window: protection.rotate_left(3),
                    impulse: impulse as i8,
                },
                protection,
                impulse,
            );
            // Both implementations must avoid reading input in this branch.
            world.processed_player_input = None;
            run_roll(&mut source);
            player_roll::advance(&objects, &mut world, owner).unwrap();
            compare(&source, &objects, &world, owner);
        }
    }
    for window in 0..=u8::MAX {
        for shoulders in [0, 0x40, 0x80, 0xFF] {
            for pressed in 0..4u16 {
                for held in 0..4u16 {
                    seed(
                        &mut source,
                        &objects,
                        &mut world,
                        owner,
                        PlayerRoll {
                            shoulders: ShoulderControl::from_bits(shoulders),
                            tap_window: window,
                            impulse: 0,
                        },
                        window ^ 0xB5,
                        window.rotate_left(3),
                    );
                    set_input(
                        &mut source,
                        &mut world,
                        held * 16 | 0xFFCF,
                        pressed * 16 | 0xFFCF,
                    );
                    run_roll(&mut source);
                    player_roll::advance(&objects, &mut world, owner).unwrap();
                    compare(&source, &objects, &world, owner);
                }
            }
        }
    }
    for field in [
        0x6ADB, 0x6ADE, 0x6B76, 0x6B78, 0x6B7D, 0x6B7F, 0x6C01, 0x6C03,
    ] {
        assert_eq!(source.bus.read8(WRAM + SLOT + field), 0xA5);
    }
}

#[test]
fn sampled_input_shoulders_and_roll_match_one_retained_sequence_for_both_players() {
    for secondary in [false, true] {
        for flight_style in [0, 0x80] {
            for button_layout in [0, 1] {
                for shoulder in [Button::LeftShoulder, Button::RightShoulder] {
                    let mut source = Source::new(&rom(), 0xA5);
                    let (mut objects, mut world, owner) = fixture(&mut source);
                    seed(
                        &mut source,
                        &objects,
                        &mut world,
                        owner,
                        PlayerRoll::default(),
                        0xA5,
                        0xEF,
                    );
                    let settings = PlayerInputSettings {
                        flight_style,
                        button_layout,
                    };
                    world.player_input_settings = Some(settings);
                    world.view_transition_mode = Some(ViewTransitionMode::default());
                    objects.get_mut(owner).unwrap().base.contacts.hit_side = if secondary {
                        HitSide::Secondary
                    } else {
                        HitSide::Primary
                    };
                    source.bus.write8(
                        WRAM + u32::from(OWNER) + 0x23,
                        if secondary { 0x40 } else { 0 },
                    );
                    source.bus.write8(WRAM + SLOT + 0x6B7D, 0);
                    source.bus.write8(WRAM + SLOT + 0x6AA0, 0x11);
                    source.bus.write16(WRAM + SLOT + 0x6A88, 0);
                    source.bus.write16(WRAM + SLOT + 0x6A8A, 0);
                    source.bus.write8(WRAM + 0x1DCF, flight_style);
                    source.bus.write8(WRAM + 0x1DD0, button_layout);
                    let mut sampled = InputState::default();
                    // Repeated real edges, held-both arbitration, complete
                    // roll decay, idle saturation and paused input gates.
                    for visit in 0..160u16 {
                        let held = match visit % 40 {
                            0 | 2..=19 => shoulder as u16,
                            20..=24 => 0x30,
                            25..=28 => Button::LeftShoulder as u16,
                            29 => Button::RightShoulder as u16,
                            _ => 0,
                        } | if visit % 3 == 0 { 0x0840 } else { 0x0480 };
                        sampled.sample(Buttons::from_bits(held));
                        let controllers = if secondary {
                            [InputState::default(), sampled]
                        } else {
                            [sampled, InputState::default()]
                        };
                        world.controller_inputs = controllers.map(Some);
                        let paused = visit % 51 == 0;
                        world.view_transition_mode = Some(ViewTransitionMode {
                            flags: if paused { 2 } else { 0 },
                        });
                        source
                            .bus
                            .write16(WRAM + 0x1B84, if paused { 2 } else { 0 });
                        for (held, pressed, input) in [
                            (0x1292, 0x1296, controllers[0]),
                            (0x1294, 0x1298, controllers[1]),
                        ] {
                            source.bus.write16(WRAM + held, input.held.bits());
                            source.bus.write16(WRAM + pressed, input.pressed.bits());
                        }
                        source.run(0x069457, None, 0, OWNER, true);
                        player_input::prepare(&objects, &mut world, owner).unwrap();
                        source.run(0x069075, None, 0, OWNER, true);
                        player_roll::prepare_shoulders(&objects, &mut world, owner).unwrap();
                        run_roll(&mut source);
                        player_roll::advance(&objects, &mut world, owner).unwrap();
                        compare(&source, &objects, &world, owner);
                    }
                }
            }
        }
    }
}

#[test]
fn original_walker_heading_confirms_shipping_both_shoulder_regression_sequence() {
    let left = Button::LeftShoulder as u16;
    let right = Button::RightShoulder as u16;
    for pilot in [0, 1, 2, 3, 4, 5, 255] {
        let mut source = Source::new(&rom(), 0);
        source
            .bus
            .write16(WRAM + u32::from(OWNER) + 0x2B, SLOT as u16);
        source.bus.write8(WRAM + SLOT + 0x6BFF, pilot);
        source.bus.write8(WRAM + SLOT + 0x6B77, 1);
        source.bus.write16(WRAM + SLOT + 0x6ABB, 13 * 256);
        let mut input = InputState::default();
        let mut heading = 13u8;
        for (held, expected_control, spring, velocity) in [
            (right, 0xA0, -2_176i16, -1i8),
            (left | right, 0x40, 544, 0),
            (left | right, 0x40, 2_584, 1),
            (right, 0x20, -238, 0),
            (0, 0, -119, 0),
            (left | right, 0xA0, -2_265, -1),
        ] {
            input.sample(Buttons::from_bits(held));
            source.bus.write16(WRAM + 0x1938, input.held.bits());
            source.bus.write16(WRAM + 0x1936, input.pressed.bits());
            source.run(0x069075, None, 0, OWNER, true);
            // End immediately after actual heading publication. Subsequent
            // terrain/animation services are outside this control check.
            source.run(0x06B482, Some(0x06B5B5), 0, OWNER, true);
            heading = heading.wrapping_add_signed(velocity);
            assert_eq!(source.bus.read8(WRAM + SLOT + 0x6B7E), expected_control);
            assert_eq!(source.bus.read16(WRAM + SLOT + 0x6AD0) as i16, spring);
            assert_eq!(source.bus.read8(WRAM + SLOT + 0x6ACD) as i8, velocity);
            assert_eq!(
                source.bus.read16(WRAM + SLOT + 0x6ABB),
                u16::from(heading) * 256
            );
            assert_eq!(
                source.bus.read8(WRAM + u32::from(OWNER) + 0x14),
                heading.wrapping_add_signed((spring >> 8) as i8)
            );
        }
    }
}

#[test]
fn original_roll_protection_publication_precedes_decay_and_new_impulse() {
    for impulse in [0, 2, -2] {
        let mut source = Source::new(&rom(), 0);
        let (objects, mut world, owner) = fixture(&mut source);
        seed(
            &mut source,
            &objects,
            &mut world,
            owner,
            PlayerRoll {
                shoulders: ShoulderControl::from_bits(0x40),
                tap_window: 0x21,
                impulse,
            },
            0xAD,
            0xFF,
        );
        set_input(&mut source, &mut world, 0x20, 0x20);
        source.writes = Some(Vec::new());
        run_roll(&mut source);
        let writes: Vec<_> = source
            .writes
            .unwrap()
            .into_iter()
            .filter(|(address, _)| {
                [0x6ADD, 0x6C02]
                    .iter()
                    .any(|field| *address == WRAM + SLOT + field)
            })
            .collect();
        assert_eq!(
            writes,
            [
                (WRAM + SLOT + 0x6C02, if impulse == 0 { 0xAD } else { 0xED }),
                (WRAM + SLOT + 0x6ADD, if impulse == 0 { 32 } else { 0 }),
            ]
        );
    }
}
