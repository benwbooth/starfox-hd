//! Execute the complete original pose body and its original wrapped chase
//! helpers. Only the CPU bootstrap is synthetic; no source byte is patched.
//! The continuous roll/pose sequence checks these two call boundaries, not
//! the intervening speed, terrain or complete player-mode implementation.

use super::{actor, rom, Source, OWNER, WRAM};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_pose::{self, PlayerPose, PoseError};
use sf2_game::player_roll;
use sf2_game::player_storage::{self, PlayerStorage, PlayerStorageInputs};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld, WorldInputError};
use sf2_game::{Angle, Button, Buttons, InputState, ObjectId, ObjectStore, RandomState};

const SLOT: u32 = 64;

struct Native {
    objects: ObjectStore,
    world: ScenePathWorld,
    runtime: PathRuntime,
    owner: ObjectId,
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
                pilot_code: 3,
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
        }
    }

    fn storage(&self) -> &PlayerStorage {
        player_storage::get(&self.objects, &self.runtime.resources, self.owner).unwrap()
    }

    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }

    fn compose(&mut self) -> Result<(), PoseError> {
        player_pose::compose(
            &mut self.objects,
            &mut self.world,
            &mut self.runtime.resources,
            self.owner,
        )
    }

    fn seed(
        &mut self,
        source: &mut Source,
        seed: u16,
        mode: u8,
        action: u8,
        linked: bool,
        ignores: bool,
    ) {
        let low = seed as u8;
        let high = (seed >> 8) as u8;
        let storage = PlayerStorage {
            fine_pitch: seed,
            fine_yaw: seed.rotate_left(5),
            bank: Angle::from_units(low),
            retained_shield: 94,
        };
        let pose = PlayerPose {
            pitch_lean: low.rotate_left(1) as i8,
            turning_lean: seed.rotate_left(7),
            yaw_trim: low.wrapping_add(11) as i8,
            ambient_bank: low.wrapping_add(33) as i8,
            steering_bank: low.wrapping_add(55) as i8,
            shoulder_bank: seed.wrapping_mul(13) as i16,
            heading_return_bank: low.wrapping_add(77) as i8,
            yaw_offset: low.wrapping_add(99) as i8,
        };
        *player_storage::get_mut(&self.objects, &mut self.runtime.resources, self.owner).unwrap() =
            storage;
        self.world.player_pitch_target = Some(seed.rotate_left(11));
        self.world.player_yaw_increment = Some(seed.rotate_left(3));
        let records = self.records();
        records.pose = Some(pose);
        records.auxiliary.as_mut().unwrap().mode = mode;
        records.auxiliary.as_mut().unwrap().action_flags = action;
        records.auxiliary.as_mut().unwrap().stored_rotation.pitch = Angle::from_units(low);
        records.charge.as_mut().unwrap().linked_mode = linked;
        records.contact.as_mut().unwrap().ignores_contacts = ignores;
        records.roll.as_mut().unwrap().impulse = high as i8;
        records.yaw_motion = Some(!seed);
        for (field, value) in [
            (0x6AB9, storage.fine_pitch),
            (0x6ABB, storage.fine_yaw),
            (0x6AD0, pose.turning_lean),
            (0x6AD8, pose.shoulder_bank as u16),
            (0x6ACD, !seed),
        ] {
            source.bus.write16(WRAM + SLOT + field, value);
        }
        for (field, value) in [
            (0x6ABD, storage.bank.units()),
            (0x6ACF, pose.pitch_lean as u8),
            (0x6AD4, pose.yaw_trim as u8),
            (0x6AD5, pose.ambient_bank as u8),
            (0x6AD7, pose.steering_bank as u8),
            (0x6ADA, pose.heading_return_bank as u8),
            (0x6ADE, pose.yaw_offset as u8),
            (0x6ADD, high),
            (0x6AA0, mode),
            (0x6B77, action),
            (0x6B32, low),
            (0x6B63, (low & !0x80) | if linked { 0x80 } else { 0 }),
            (0x6A72, (low & !0x10) | if ignores { 0x10 } else { 0 }),
        ] {
            source.bus.write8(WRAM + SLOT + field, value);
        }
        source
            .bus
            .write16(WRAM + 0x1E36, self.world.player_pitch_target.unwrap());
        source
            .bus
            .write16(WRAM + 0x1E38, self.world.player_yaw_increment.unwrap());
        let base = &mut self.objects.get_mut(self.owner).unwrap().base;
        base.pitch = Angle::from_units(high ^ 0x5A);
        base.yaw = Angle::from_units(high ^ 0x7B);
        base.roll = Angle::from_units(high ^ 0x91);
        for (field, value) in [
            (0x12, base.pitch.units()),
            (0x14, base.yaw.units()),
            (0x16, base.roll.units()),
        ] {
            source.bus.write8(WRAM + u32::from(OWNER) + field, value);
        }
    }

    fn compare(&self, source: &Source) {
        let records = self.world.player(&self.objects, self.owner).unwrap();
        let storage = self.storage();
        for (field, actual) in [
            (0x6AB9, storage.fine_pitch),
            (0x6ABB, storage.fine_yaw),
            (0x6ACD, records.yaw_motion.unwrap()),
        ] {
            assert_eq!(
                actual,
                source.bus.read16(WRAM + SLOT + field),
                "field={field:04X}"
            );
        }
        for (field, actual) in [
            (0x6ABD, storage.bank.units()),
            (0x6AD4, records.pose.unwrap().yaw_trim as u8),
        ] {
            assert_eq!(
                actual,
                source.bus.read8(WRAM + SLOT + field),
                "field={field:04X}"
            );
        }
        let base = &self.objects.get(self.owner).unwrap().base;
        for (field, actual) in [
            (0x12, base.pitch.units()),
            (0x14, base.yaw.units()),
            (0x16, base.roll.units()),
        ] {
            assert_eq!(
                actual,
                source.bus.read8(WRAM + u32::from(OWNER) + field),
                "angle={field:02X}"
            );
        }
    }

    fn run_and_compare(&mut self, source: &mut Source) {
        let before = *self.records();
        let prior_actor = self.objects.get(self.owner).unwrap().clone();
        source.run(0x06ECB0, Some(0x06EE09), 0, OWNER, true);
        self.compose().unwrap();
        self.compare(source);
        let mut expected = before;
        expected.pose.as_mut().unwrap().yaw_trim = source.bus.read8(WRAM + SLOT + 0x6AD4) as i8;
        expected.yaw_motion = Some(source.bus.read16(WRAM + SLOT + 0x6ACD));
        assert_eq!(*self.records(), expected);
        let mut actor = self.objects.get(self.owner).unwrap().clone();
        actor.base.pitch = prior_actor.base.pitch;
        actor.base.yaw = prior_actor.base.yaw;
        actor.base.roll = prior_actor.base.roll;
        assert_eq!(
            actor, prior_actor,
            "pose must not move, damage or aim the actor"
        );
        assert_eq!(self.storage().retained_shield, 94);
    }
}

#[test]
fn pose_matches_original_every_fine_pitch_for_both_chase_rates_and_every_bank_impulse_pair() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    for mode in [0x11, 0x12] {
        for seed in 0..=u16::MAX {
            native.seed(&mut source, seed, mode, 1, false, false);
            native.run_and_compare(&mut source);
        }
    }
}

#[test]
fn pose_matches_original_every_mode_and_all_action_link_contact_branches() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    for mode in 0..=u8::MAX {
        for action in [0, 1, 4, 5, 0xFA, 0xFB, 0xFE, 0xFF] {
            for linked in [false, true] {
                for ignores in [false, true] {
                    let seed = u16::from(mode) * 257;
                    native.seed(&mut source, seed, mode, action, linked, ignores);
                    native.run_and_compare(&mut source);
                }
            }
        }
    }
    for field in [
        0x6AB8, 0x6ABE, 0x6ACC, 0x6AD2, 0x6AD3, 0x6AD6, 0x6ADB, 0x6ADC, 0x6ADF, 0x6B31, 0x6B33,
    ] {
        assert_eq!(
            source.bus.read8(WRAM + SLOT + field),
            0xA5,
            "neighbor={field:04X}"
        );
    }
}

#[test]
fn pose_missing_input_prefixes_match_original_store_order() {
    let mut source = Source::new(&rom(), 0xA5);
    for missing in 0..5 {
        // Restore the missing field owners between cases.
        let mut native = Native::new(&mut source);
        for byte in 0..=u8::MAX {
            native.seed(&mut source, u16::from(byte) * 257, 0x11, 1, false, false);
            let (stop, error) = match missing {
                0 => {
                    native.world.player_pitch_target = None;
                    (0x06ECF4, PoseError::MissingPitchTarget)
                }
                1 => {
                    native.records().charge = None;
                    (
                        0x06ED0B,
                        PoseError::World(WorldInputError::MissingPlayerCharge(native.owner)),
                    )
                }
                2 => {
                    native.world.player_yaw_increment = None;
                    (0x06ED63, PoseError::MissingYawIncrement)
                }
                3 => {
                    native.records().contact = None;
                    (
                        0x06ED99,
                        PoseError::World(WorldInputError::MissingPlayerContact(native.owner)),
                    )
                }
                _ => {
                    native.records().roll = None;
                    (0x06EDC3, PoseError::MissingRoll(native.owner))
                }
            };
            source.run(0x06ECB0, Some(stop), 0, OWNER, true);
            assert_eq!(native.compose(), Err(error));
            native.compare(&source);
            // Restore only the intentionally missing owner. Pose and
            // retained fine fields are reseeded from source on the next case.
            match missing {
                1 => native.records().charge = Some(Default::default()),
                3 => native.records().contact = Some(Default::default()),
                4 => native.records().roll = Some(Default::default()),
                _ => (),
            }
        }
    }
}

#[test]
fn continuous_original_roll_and_pose_use_entering_protection_and_updated_bank() {
    for shoulder in [Button::LeftShoulder, Button::RightShoulder] {
        for initial_bank in [0u8, 1, 127, 128, 255] {
            let mut source = Source::new(&rom(), 0xA5);
            let mut native = Native::new(&mut source);
            native.seed(&mut source, u16::from(initial_bank), 0x11, 1, false, false);
            *native.records().roll.as_mut().unwrap() = Default::default();
            native.records().protection = Some(Default::default());
            for field in [0x6B7E, 0x6ADC, 0x6ADD, 0x6C02] {
                source.bus.write8(WRAM + SLOT + field, 0);
            }
            for visit in 0..160u16 {
                let bit = shoulder as u16;
                let (held, pressed) = match visit % 40 {
                    0 | 2 => (bit, bit),
                    3..=20 => (bit, 0),
                    _ => (0, 0),
                };
                native.world.processed_player_input = Some(InputState {
                    held: Buttons::from_bits(held),
                    pressed: Buttons::from_bits(pressed),
                });
                source.bus.write16(WRAM + 0x1938, held);
                source.bus.write16(WRAM + 0x1936, pressed);
                source.run(0x069075, None, 0, OWNER, true);
                player_roll::prepare_shoulders(&native.objects, &mut native.world, native.owner)
                    .unwrap();
                let spinning = source.bus.read8(WRAM + SLOT + 0x6ADD) != 0;
                source.run(
                    0x06E7FF,
                    Some(if spinning { 0x06E8F4 } else { 0x06E8C8 }),
                    0,
                    OWNER,
                    true,
                );
                player_roll::advance(&native.objects, &mut native.world, native.owner).unwrap();
                native.run_and_compare(&mut source);
                let records = native.records();
                for (field, value) in [
                    (0x6B7E, records.roll.unwrap().shoulders.bits()),
                    (0x6ADC, records.roll.unwrap().tap_window),
                    (0x6ADD, records.roll.unwrap().impulse as u8),
                    (0x6B77, records.auxiliary.unwrap().action_flags),
                    (0x6C02, records.protection.unwrap().control()),
                ] {
                    assert_eq!(
                        value,
                        source.bus.read8(WRAM + SLOT + field),
                        "visit={visit} field={field:04X}"
                    );
                }
            }
        }
    }
}
