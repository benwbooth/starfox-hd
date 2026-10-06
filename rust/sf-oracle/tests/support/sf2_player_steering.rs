//! Full unchanged horizontal controller, including lean, heading recovery,
//! pilot lookup, original signed multiplication and original chase helpers.
//! Special-mode response is supplied at this call boundary; this does not
//! certify its upstream producer or the complete flight-mode dispatch.

use super::{actor, rom, Source, OWNER, WRAM};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_pose::PlayerPose;
use sf2_game::player_roll::{self, ShoulderControl};
use sf2_game::player_steering::{self, PlayerSteering, SteeringContext, SteeringError};
use sf2_game::player_storage::{self, PlayerStorage, PlayerStorageInputs};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld, WorldInputError};
use sf2_game::{Angle, Buttons, InputState, ObjectId, ObjectStore, RandomState};

const SLOT: u32 = 64;

struct Native {
    objects: ObjectStore,
    world: ScenePathWorld,
    runtime: PathRuntime,
    owner: ObjectId,
    context: SteeringContext,
}

impl Native {
    fn new(source: &mut Source) -> Self {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut runtime = PathRuntime::default();
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 94,
                score: Default::default(),
            },
        )
        .unwrap();
        source
            .bus
            .write16(WRAM + u32::from(OWNER) + 0x2B, SLOT as u16);
        Self {
            objects,
            world,
            runtime,
            owner,
            context: Default::default(),
        }
    }

    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }

    fn seed(
        &mut self,
        source: &mut Source,
        seed: u16,
        pilot: u8,
        mode: u8,
        flags: u8,
        ignores: bool,
        input: InputState,
    ) {
        let low = seed as u8;
        let high = (seed >> 8) as u8;
        let steering = PlayerSteering {
            direction_age: low,
            turn_response: seed,
            camera_bank_target: high as i8,
            locked_heading: Angle::from_units(low.wrapping_add(37)),
            lateral_offset: seed.rotate_left(3) as i16,
        };
        let pose = PlayerPose {
            pitch_lean: high as i8,
            turning_lean: seed.rotate_left(7),
            yaw_trim: low as i8,
            ambient_bank: high as i8,
            steering_bank: low.wrapping_add(17) as i8,
            shoulder_bank: seed as i16,
            heading_return_bank: low.wrapping_add(99) as i8,
            yaw_offset: high as i8,
        };
        let storage = PlayerStorage {
            fine_pitch: seed.rotate_left(9),
            fine_yaw: seed.rotate_left(11),
            bank: Angle::from_units(high),
            retained_shield: 94,
        };
        *player_storage::get_mut(&self.objects, &mut self.runtime.resources, self.owner).unwrap() =
            storage;
        self.world.processed_player_input = Some(input);
        self.world.player_yaw_increment = Some(seed.rotate_left(5));
        self.context.inherited_response_target = Some(seed.rotate_left(13));
        let records = self.records();
        records.steering = Some(steering);
        records.pose = Some(pose);
        records.auxiliary.as_mut().unwrap().mode = mode;
        records.auxiliary.as_mut().unwrap().action_flags = flags;
        records.visit.as_mut().unwrap().pilot_code = pilot;
        records.contact.as_mut().unwrap().ignores_contacts = ignores;
        records.roll.as_mut().unwrap().shoulders = ShoulderControl::from_bits(low.rotate_left(3));
        for (field, value) in [
            (0x6AB9, storage.fine_pitch),
            (0x6ABB, storage.fine_yaw),
            (0x6AD0, pose.turning_lean),
            (0x6AD2, steering.turn_response),
            (0x6AD8, pose.shoulder_bank as u16),
            (0x6B15, steering.lateral_offset as u16),
        ] {
            source.bus.write16(WRAM + SLOT + field, value);
        }
        for (field, value) in [
            (0x6AA0, mode),
            (0x6B77, flags),
            (0x6BFF, pilot),
            (0x6A72, (low & !0x10) | if ignores { 0x10 } else { 0 }),
            (0x6AC0, steering.camera_bank_target as u8),
            (0x6AAE, steering.locked_heading.units()),
            (0x6B17, steering.direction_age),
            (0x6AD7, pose.steering_bank as u8),
            (0x6ADA, pose.heading_return_bank as u8),
            (0x6B7E, low.rotate_left(3)),
            (0x6ABD, storage.bank.units()),
        ] {
            source.bus.write8(WRAM + SLOT + field, value);
        }
        source.bus.write16(WRAM + 0x1936, input.pressed.bits());
        source.bus.write16(WRAM + 0x1938, input.held.bits());
        source
            .bus
            .write16(WRAM + 0x1E38, self.world.player_yaw_increment.unwrap());
        source
            .bus
            .write16(WRAM + 0x0A, self.context.inherited_response_target.unwrap());
    }

    fn advance(&mut self) -> Result<(), SteeringError> {
        player_steering::advance(
            &self.objects,
            &mut self.world,
            &mut self.runtime.resources,
            self.owner,
            self.context,
        )
    }

    fn source_records(source: &Source, mut expected: PlayerPathRecords) -> PlayerPathRecords {
        if let Some(steering) = &mut expected.steering {
            steering.direction_age = source.bus.read8(WRAM + SLOT + 0x6B17);
            steering.turn_response = source.bus.read16(WRAM + SLOT + 0x6AD2);
            steering.camera_bank_target = source.bus.read8(WRAM + SLOT + 0x6AC0) as i8;
            steering.lateral_offset = source.bus.read16(WRAM + SLOT + 0x6B15) as i16;
        }
        if let Some(pose) = &mut expected.pose {
            pose.turning_lean = source.bus.read16(WRAM + SLOT + 0x6AD0);
            pose.steering_bank = source.bus.read8(WRAM + SLOT + 0x6AD7) as i8;
            pose.shoulder_bank = source.bus.read16(WRAM + SLOT + 0x6AD8) as i16;
            pose.heading_return_bank = source.bus.read8(WRAM + SLOT + 0x6ADA) as i8;
        }
        expected.auxiliary.as_mut().unwrap().action_flags = source.bus.read8(WRAM + SLOT + 0x6B77);
        expected
    }

    fn compare(&mut self, source: &Source, before: PlayerPathRecords) {
        assert_eq!(*self.records(), Self::source_records(source, before));
        assert_eq!(
            self.world.player_yaw_increment,
            Some(source.bus.read16(WRAM + 0x1E38))
        );
        let storage =
            player_storage::get(&self.objects, &self.runtime.resources, self.owner).unwrap();
        assert_eq!(storage.fine_yaw, source.bus.read16(WRAM + SLOT + 0x6ABB));
        assert_eq!(storage.fine_pitch, source.bus.read16(WRAM + SLOT + 0x6AB9));
        assert_eq!(storage.bank.units(), source.bus.read8(WRAM + SLOT + 0x6ABD));
        assert_eq!(storage.retained_shield, 94);
    }

    fn run(&mut self, source: &mut Source) {
        let before = *self.records();
        let actor = self.objects.get(self.owner).unwrap().clone();
        source.run(0x06E4B1, Some(0x06E778), 0, OWNER, true);
        self.advance().unwrap();
        self.compare(source, before);
        assert_eq!(
            self.objects.get(self.owner).unwrap(),
            &actor,
            "steering must not integrate the actor"
        );
    }
}

fn input(held: u16, pressed: u16) -> InputState {
    InputState {
        held: Buttons::from_bits(held),
        pressed: Buttons::from_bits(pressed),
    }
}

#[test]
fn steering_matches_original_every_pilot_direction_shoulder_brake_and_contact_combination() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    for pilot in 0..=u8::MAX {
        for direction in [0, 0x100, 0x200, 0x300] {
            for shoulders in [0, 0x10, 0x20, 0x30] {
                for flags in [0, 0x20] {
                    for ignores in [false, true] {
                        native.seed(
                            &mut source,
                            u16::from(pilot) * 257,
                            pilot,
                            0x11,
                            flags,
                            ignores,
                            input(direction | shoulders, direction),
                        );
                        native.run(&mut source);
                    }
                }
            }
        }
    }
}

#[test]
fn steering_matches_original_every_word_value_in_normal_locked_and_special_turn_branches() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    for (mode, flags) in [(0x11, 0), (0x11, 4), (0x30, 0)] {
        for seed in 0..=u16::MAX {
            let direction = if seed & 1 == 0 { 0x100 } else { 0x200 };
            native.seed(
                &mut source,
                seed,
                (seed >> 8) as u8,
                mode,
                flags,
                seed & 2 != 0,
                input(direction, 0),
            );
            native.run(&mut source);
        }
    }
}

#[test]
fn steering_matches_original_mode_families_neutral_hold_and_wrapped_edge_age() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    for mode in 0..=u8::MAX {
        for flags in [0, 4, 0xFB, 0xFF] {
            for direction in [0, 0x100, 0x200, 0x300] {
                for pressed in [0, 0x100, 0x200, 0x300] {
                    native.seed(
                        &mut source,
                        u16::from(mode) * 257,
                        3,
                        mode,
                        flags,
                        false,
                        input(direction, pressed),
                    );
                    native.run(&mut source);
                }
            }
        }
    }
    for field in [
        0x6AAD, 0x6AAF, 0x6AB8, 0x6ABE, 0x6ABF, 0x6AC1, 0x6ACE, 0x6AD4, 0x6AD5, 0x6AD6, 0x6ADB,
        0x6B14, 0x6B18, 0x6B76, 0x6B78, 0x6B7D, 0x6B7F,
    ] {
        assert_eq!(
            source.bus.read8(WRAM + SLOT + field),
            0xA5,
            "neighbor={field:04X}"
        );
    }
}

#[test]
fn steering_missing_dependency_prefixes_match_original_write_order() {
    let mut source = Source::new(&rom(), 0xA5);
    for missing in 0..4 {
        let mut native = Native::new(&mut source);
        for byte in 0..=u8::MAX {
            native.seed(
                &mut source,
                u16::from(byte) * 257,
                3,
                0x11,
                0,
                false,
                input(0x200, 0),
            );
            let (stop, error) = match missing {
                0 => {
                    native.records().pose = None;
                    (0x06E9BB, SteeringError::MissingPose(native.owner))
                }
                1 => {
                    native.records().contact = None;
                    (
                        0x06E61E,
                        SteeringError::World(WorldInputError::MissingPlayerContact(native.owner)),
                    )
                }
                2 => {
                    native.records().roll = None;
                    (0x06E673, SteeringError::MissingRoll(native.owner))
                }
                _ => {
                    native.records().auxiliary.as_mut().unwrap().action_flags = 4;
                    source.bus.write8(WRAM + SLOT + 0x6B77, 4);
                    native.context.inherited_response_target = None;
                    (0x06E99E, SteeringError::MissingInheritedResponseTarget)
                }
            };
            let before = *native.records();
            source.run(0x06E4B1, Some(stop), 0, OWNER, true);
            assert_eq!(native.advance(), Err(error));
            native.compare(&source, before);
            // Restore records whose absence persists beyond seed().
            if missing == 1 {
                native.records().contact = Some(Default::default());
            }
            if missing == 2 {
                native.records().roll = Some(Default::default());
            }
        }
    }
}

#[test]
fn steering_sine_and_pilot_tables_are_byte_equal_to_original() {
    let rom = rom();
    for angle in 0..256 {
        assert_eq!(sf_core::snes_trig::SINTAB[angle] as u8, rom[0xE26 + angle]);
    }
    assert_eq!(&rom[0x369F0..0x369F6], &[24; 6]);
    assert_eq!(&rom[0x367F9..0x367FF], &[40; 6]);
}

#[test]
fn continuous_shoulders_steering_roll_and_pose_match_original_call_order() {
    let mut source = Source::new(&rom(), 0xA5);
    for pilot in 0..6 {
        let mut native = Native::new(&mut source);
        native.seed(
            &mut source,
            u16::from(pilot) * 2713,
            pilot,
            0x11,
            0,
            false,
            input(0, 0),
        );
        // Pose needs the entering shared pitch target and disjoint additive
        // fields. Supply them once; do not re-seed retained state each visit.
        native.world.player_pitch_target = Some(0);
        source.bus.write16(WRAM + 0x1E36, 0);
        native.records().roll.as_mut().unwrap().tap_window = 0;
        native.records().roll.as_mut().unwrap().impulse = 0;
        source.bus.write8(WRAM + SLOT + 0x6ADC, 0);
        source.bus.write8(WRAM + SLOT + 0x6ADD, 0);
        for (field, value) in [
            (0x6ACF, native.records().pose.unwrap().pitch_lean as u8),
            (0x6AD4, native.records().pose.unwrap().yaw_trim as u8),
            (0x6AD5, native.records().pose.unwrap().ambient_bank as u8),
            (0x6ADE, native.records().pose.unwrap().yaw_offset as u8),
        ] {
            source.bus.write8(WRAM + SLOT + field, value);
        }
        for visit in 0..320 {
            let direction = [0x200, 0x100, 0x300, 0][(visit / 20) % 4];
            let shoulder = if (visit / 40) % 2 == 0 { 0x10 } else { 0x20 };
            let held = direction | if visit % 40 < 12 { shoulder } else { 0 };
            let pressed = if matches!(visit % 40, 0 | 4) {
                shoulder
            } else {
                0
            };
            native.world.processed_player_input = Some(input(held, pressed));
            native.world.player_yaw_increment = Some(0);
            source.bus.write16(WRAM + 0x1936, pressed);
            source.bus.write16(WRAM + 0x1938, held);
            source.bus.write16(WRAM + 0x1E38, 0);
            source.run(0x069075, None, 0, OWNER, true);
            player_roll::prepare_shoulders(&native.objects, &mut native.world, native.owner)
                .unwrap();
            source.bus.write16(
                WRAM + 0x0A,
                native.context.inherited_response_target.unwrap(),
            );
            native.run(&mut source);
            let spinning = source.bus.read8(WRAM + SLOT + 0x6ADD) != 0;
            source.run(
                0x06E7FF,
                Some(if spinning { 0x06E8F4 } else { 0x06E8C8 }),
                0,
                OWNER,
                true,
            );
            player_roll::advance(&native.objects, &mut native.world, native.owner).unwrap();
            source.run(0x06ECB0, Some(0x06EE09), 0, OWNER, true);
            sf2_game::player_pose::compose(
                &mut native.objects,
                &mut native.world,
                &mut native.runtime.resources,
                native.owner,
            )
            .unwrap();
            let records = *native.records();
            assert_eq!(
                records.protection.unwrap().control() & 0x40,
                source.bus.read8(WRAM + SLOT + 0x6C02) & 0x40
            );
            for (field, actual) in [
                (0x6B7E, records.roll.unwrap().shoulders.bits()),
                (0x6ADC, records.roll.unwrap().tap_window),
                (0x6ADD, records.roll.unwrap().impulse as u8),
                (0x6AD4, records.pose.unwrap().yaw_trim as u8),
            ] {
                assert_eq!(
                    actual,
                    source.bus.read8(WRAM + SLOT + field),
                    "pilot={pilot} visit={visit} field={field:04X}"
                );
            }
            let base = &native.objects.get(native.owner).unwrap().base;
            for (field, value) in [
                (0x12, base.pitch.units()),
                (0x14, base.yaw.units()),
                (0x16, base.roll.units()),
            ] {
                assert_eq!(
                    value,
                    source.bus.read8(WRAM + u32::from(OWNER) + field),
                    "pilot={pilot} visit={visit} angle={field:02X}"
                );
            }
        }
    }
}
