//! Complete original pitch, history, profile and terrain routines, including
//! the original signed multiply and chase helpers. No source bytes patched.

use super::{actor, rom, Source, OWNER, WRAM};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::player_vertical::{
    self, PlayerVerticalControl, VerticalError, VerticalMode, VerticalProfile,
};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::{Buttons, InputState, ObjectId, ObjectStore, RandomState};

const SLOT: u32 = 64;

#[derive(Clone, Copy, Debug)]
enum Operation {
    History,
    FlightPitch,
    HeldPitch,
    Soft,
    Hard,
}

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
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn seed(&mut self, source: &mut Source, seed: u16, height: i16, held: u16, pressed: u16) {
        let low = seed as u8;
        let high = (seed >> 8) as u8;
        let record = PlayerVerticalControl {
            profile: VerticalProfile {
                upper_height_offset: seed.rotate_left(3) as i16,
                lower_height_offset: seed.rotate_left(5) as i16,
                up_pitch: low,
                down_pitch: high,
            },
            limit_flags: low,
            control_flags: high,
            motion_axes: low.rotate_left(3),
            pitch_adjustment: high.rotate_left(3),
            latched_input: seed.rotate_left(7),
            previous_held: seed.rotate_left(11),
        };
        self.records().vertical = Some(record);
        self.records().auxiliary.as_mut().unwrap().action_flags = low;
        self.records()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .hold_secondary_protection = high & 1 != 0;
        let pose = self.records().pose.as_mut().unwrap();
        pose.pitch_lean = low as i8;
        pose.steering_bank = high as i8;
        self.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(held),
            pressed: Buttons::from_bits(pressed),
        });
        self.world.player_pitch_target = Some(seed.rotate_left(9));
        self.world.environment_plane_height = Some(seed.rotate_left(13) as i16);
        self.world.strategy_clock = seed;
        self.objects.get_mut(self.owner).unwrap().base.position.y = height;
        player_storage::get_mut(&self.objects, &mut self.runtime.resources, self.owner)
            .unwrap()
            .fine_pitch = seed;
        for (field, value) in [
            (0x6BF5, record.profile.upper_height_offset as u16),
            (0x6BF7, record.profile.lower_height_offset as u16),
            (0x6B87, record.latched_input),
            (0x6B89, record.previous_held),
            (0x6AB9, seed),
        ] {
            source.bus.write16(WRAM + SLOT + field, value);
        }
        for (field, value) in [
            (0x6BF9, record.profile.up_pitch),
            (0x6BFA, record.profile.down_pitch),
            (0x6B81, record.limit_flags),
            (0x6B82, record.control_flags),
            (0x6B84, record.motion_axes),
            (0x6ABF, record.pitch_adjustment),
            (0x6ACF, low),
            (0x6AD7, high),
            (0x6B77, low),
            (0x6B7D, if high & 1 != 0 { 0x80 } else { 0 }),
        ] {
            source.bus.write8(WRAM + SLOT + field, value);
        }
        for (field, value) in [
            (0x1936, pressed),
            (0x1938, held),
            (0x1E36, self.world.player_pitch_target.unwrap()),
            (0x1E0F, self.world.environment_plane_height.unwrap() as u16),
            (0xC4, seed),
            (u32::from(OWNER) + 0x0E, height as u16),
        ] {
            source.bus.write16(WRAM + field, value);
        }
    }
    fn set_profile(&mut self, source: &mut Source, upper: i16, lower: i16) {
        let profile = &mut self.records().vertical.as_mut().unwrap().profile;
        profile.upper_height_offset = upper;
        profile.lower_height_offset = lower;
        source.bus.write16(WRAM + SLOT + 0x6BF5, upper as u16);
        source.bus.write16(WRAM + SLOT + 0x6BF7, lower as u16);
    }
    fn target(&mut self, source: &mut Source, value: u16) {
        self.world.player_pitch_target = Some(value);
        source.bus.write16(WRAM + 0x1E36, value);
    }
    fn source_records(source: &Source, mut before: PlayerPathRecords) -> PlayerPathRecords {
        if let Some(record) = &mut before.vertical {
            record.profile = VerticalProfile {
                upper_height_offset: source.bus.read16(WRAM + SLOT + 0x6BF5) as i16,
                lower_height_offset: source.bus.read16(WRAM + SLOT + 0x6BF7) as i16,
                up_pitch: source.bus.read8(WRAM + SLOT + 0x6BF9),
                down_pitch: source.bus.read8(WRAM + SLOT + 0x6BFA),
            };
            record.limit_flags = source.bus.read8(WRAM + SLOT + 0x6B81);
            record.control_flags = source.bus.read8(WRAM + SLOT + 0x6B82);
            record.motion_axes = source.bus.read8(WRAM + SLOT + 0x6B84);
            record.pitch_adjustment = source.bus.read8(WRAM + SLOT + 0x6ABF);
            record.latched_input = source.bus.read16(WRAM + SLOT + 0x6B87);
            record.previous_held = source.bus.read16(WRAM + SLOT + 0x6B89);
        }
        if let Some(pose) = &mut before.pose {
            pose.pitch_lean = source.bus.read8(WRAM + SLOT + 0x6ACF) as i8;
            pose.steering_bank = source.bus.read8(WRAM + SLOT + 0x6AD7) as i8;
        }
        before
    }
    fn compare(&mut self, source: &Source, before: PlayerPathRecords) {
        assert_eq!(*self.records(), Self::source_records(source, before));
        assert_eq!(
            self.world.player_pitch_target,
            Some(source.bus.read16(WRAM + 0x1E36))
        );
    }
    fn original(source: &mut Source, op: Operation) {
        let (entry, stop) = match op {
            Operation::History => (0x06F2D9, None),
            Operation::FlightPitch => (0x06E3F7, Some(0x06E4B0)),
            Operation::HeldPitch => (0x06E39A, Some(0x06E3F6)),
            Operation::Soft => (0x06E9F6, Some(0x06EA0F)),
            Operation::Hard => (0x06EA10, Some(0x06EA43)),
        };
        source.run(entry, stop, 0, OWNER, true);
    }
    fn advance(&mut self, op: Operation) -> Result<(), VerticalError> {
        match op {
            Operation::History => {
                player_vertical::retain_input(&self.objects, &mut self.world, self.owner)
            }
            Operation::FlightPitch => {
                player_vertical::flight_pitch(&self.objects, &mut self.world, self.owner)
            }
            Operation::HeldPitch => player_vertical::held_pitch(
                &self.objects,
                &mut self.world,
                &self.runtime.resources,
                self.owner,
            ),
            Operation::Soft => {
                player_vertical::soft_limits(&self.objects, &mut self.world, self.owner)
            }
            Operation::Hard => {
                player_vertical::hard_limits(&self.objects, &mut self.world, self.owner)
            }
        }
    }
    fn run(&mut self, source: &mut Source, op: Operation) {
        let before = *self.records();
        let actor = self.objects.get(self.owner).unwrap().clone();
        let storage =
            *player_storage::get(&self.objects, &self.runtime.resources, self.owner).unwrap();
        Self::original(source, op);
        self.advance(op).unwrap();
        self.compare(source, before);
        assert_eq!(*self.objects.get(self.owner).unwrap(), actor);
        assert_eq!(
            *player_storage::get(&self.objects, &self.runtime.resources, self.owner).unwrap(),
            storage
        );
        assert_eq!(storage.fine_pitch, source.bus.read16(WRAM + SLOT + 0x6AB9));
    }
}

#[test]
fn vertical_profile_copy_and_full_word_history_match_original() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    for seed in 0..=u16::MAX {
        native.seed(&mut source, seed, 17, seed.rotate_left(9), !seed);
        let profile = VerticalProfile {
            upper_height_offset: seed as i16,
            lower_height_offset: !seed as i16,
            up_pitch: seed as u8,
            down_pitch: (seed >> 8) as u8,
        };
        source
            .bus
            .write16(WRAM + 0x08, profile.upper_height_offset as u16);
        source
            .bus
            .write16(WRAM + 0x0A, profile.lower_height_offset as u16);
        source.bus.write8(WRAM + 0x02, profile.up_pitch);
        source.bus.write8(WRAM + 0x04, profile.down_pitch);
        let before = *native.records();
        source.run(0x069A44, Some(0x069A5E), 0, OWNER, true);
        player_vertical::configure(&native.objects, &mut native.world, native.owner, profile)
            .unwrap();
        native.compare(&source, before);
        native.run(&mut source, Operation::History);
    }
}

#[test]
fn both_pitch_modes_match_original_for_all_flags_input_edges_history_and_protection() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    for flag in 0..=u8::MAX {
        for held in [0, 0x400, 0x800, 0xC00] {
            for pressed in [0, 0x400, 0x800, 0xC00] {
                for retained in [0, 0x400] {
                    for protected in [false, true] {
                        for op in [Operation::FlightPitch, Operation::HeldPitch] {
                            native.seed(
                                &mut source,
                                u16::from(flag) * 257,
                                flag as i16,
                                held,
                                pressed,
                            );
                            native.records().vertical.as_mut().unwrap().latched_input = retained;
                            source.bus.write16(WRAM + SLOT + 0x6B87, retained);
                            native
                                .records()
                                .contact
                                .as_mut()
                                .unwrap()
                                .hit
                                .hold_secondary_protection = protected;
                            source
                                .bus
                                .write8(WRAM + SLOT + 0x6B7D, if protected { 0x80 } else { 0 });
                            native.run(&mut source, op);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn protected_pitch_matches_original_every_height_and_wrapped_plane_threshold() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    for plane in [i16::MIN, 0, 50, i16::MAX] {
        for height in 0..=u16::MAX {
            native.seed(&mut source, 0xF101, height as i16, 0x800, 0);
            native.world.environment_plane_height = Some(plane);
            source.bus.write16(WRAM + 0x1E0F, plane as u16);
            native.run(&mut source, Operation::FlightPitch);
        }
    }
}

#[test]
fn both_terrain_controllers_match_original_every_height_and_pitch_word() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    // Negative/positive/neutral targets cover admission. Full-word height
    // sweeps include wrapped offsets, response clamping and both boundaries.
    for op in [Operation::Soft, Operation::Hard] {
        for target in [0, 0xF101, 0x0F03] {
            for height in 0..=u16::MAX {
                native.seed(
                    &mut source,
                    height,
                    height as i16,
                    height.rotate_left(7),
                    !height,
                );
                native.set_profile(&mut source, 600, 12);
                native.target(&mut source, target);
                native.run(&mut source, op);
            }
        }
        // Every fine target, with alternating near-boundary and clamped
        // response, and all byte leans/banks across both clock parities.
        for target in 0..=u16::MAX {
            let height = [-1, -33, -65, -120, -400, 12, 64, 32767][usize::from(target & 7)];
            native.seed(&mut source, target, height, target.rotate_left(7), !target);
            native.set_profile(&mut source, 0, 0);
            native.target(&mut source, target);
            native.run(&mut source, op);
        }
    }
}

#[test]
fn terrain_response_table_is_the_original_signed_cosine_bytes() {
    let rom = rom();
    for distance in 0..128 {
        assert_eq!(
            sf_core::snes_trig::COSTAB[distance] as u8,
            rom[0xE66 + distance]
        );
    }
}

#[test]
fn vertical_pairs_match_original_continuous_history_configuration_and_pose_consumption() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    for mode in [VerticalMode::Flight, VerticalMode::RetainedPitch] {
        native.seed(&mut source, 0xF101, -200, 0, 0);
        native.set_profile(&mut source, 600, 12);
        native.world.player_yaw_increment = Some(0);
        // Unaffected pose fields begin at the allocation's real clear state.
        native.records().auxiliary.as_mut().unwrap().mode = 0x11;
        native.records().charge.as_mut().unwrap().linked_mode = false;
        native.records().contact.as_mut().unwrap().ignores_contacts = false;
        source.bus.write8(WRAM + SLOT + 0x6AA0, 0x11);
        source.bus.write8(WRAM + SLOT + 0x6A72, 0);
        source.bus.write8(WRAM + SLOT + 0x6B63, 0);
        source.bus.write16(WRAM + 0x1E38, 0);
        for field in [0x6ABB, 0x6ACD, 0x6AD0, 0x6AD8] {
            source.bus.write16(WRAM + SLOT + field, 0);
        }
        for field in [0x6ABD, 0x6AD4, 0x6AD5, 0x6ADA, 0x6ADE, 0x6ADD] {
            source.bus.write8(WRAM + SLOT + field, 0);
        }
        // Each side composes its own pose and retains its own fine pitch.
        // No native output is fed back into the reference execution.
        for visit in 0..768u16 {
            let held = [0, 0x400, 0x800, 0xC00][usize::from((visit / 11) & 3)];
            let previous = native.world.processed_player_input.unwrap().held.bits();
            let pressed = held & !previous;
            native.world.processed_player_input = Some(InputState {
                held: Buttons::from_bits(held),
                pressed: Buttons::from_bits(pressed),
            });
            source.bus.write16(WRAM + 0x1938, held);
            source.bus.write16(WRAM + 0x1936, pressed);
            native.world.strategy_clock = visit;
            source.bus.write16(WRAM + 0xC4, visit);
            let height = (visit as i16).wrapping_mul(3).wrapping_sub(800);
            native
                .objects
                .get_mut(native.owner)
                .unwrap()
                .base
                .position
                .y = height;
            source
                .bus
                .write16(WRAM + u32::from(OWNER) + 0x0E, height as u16);
            native.run(&mut source, Operation::History);
            let before = *native.records();
            let (pitch, terrain) = match mode {
                VerticalMode::Flight => (Operation::FlightPitch, Operation::Soft),
                VerticalMode::RetainedPitch => (Operation::HeldPitch, Operation::Hard),
            };
            Native::original(&mut source, pitch);
            Native::original(&mut source, terrain);
            player_vertical::advance(
                &native.objects,
                &mut native.world,
                &native.runtime.resources,
                native.owner,
                mode,
            )
            .unwrap();
            native.compare(&source, before);
            source.run(0x06ECB0, Some(0x06EE09), 0, OWNER, true);
            sf2_game::player_pose::compose(
                &mut native.objects,
                &mut native.world,
                &mut native.runtime.resources,
                native.owner,
            )
            .unwrap();
            let storage =
                player_storage::get(&native.objects, &native.runtime.resources, native.owner)
                    .unwrap();
            assert_eq!(storage.fine_pitch, source.bus.read16(WRAM + SLOT + 0x6AB9));
            assert_eq!(storage.fine_yaw, source.bus.read16(WRAM + SLOT + 0x6ABB));
            assert_eq!(storage.bank.units(), source.bus.read8(WRAM + SLOT + 0x6ABD));
            let base = &native.objects.get(native.owner).unwrap().base;
            for (field, actual) in [
                (0x12, base.pitch.units()),
                (0x14, base.yaw.units()),
                (0x16, base.roll.units()),
            ] {
                assert_eq!(
                    actual,
                    source.bus.read8(WRAM + u32::from(OWNER) + field),
                    "mode={mode:?} visit={visit} field={field}"
                );
            }
        }
    }
}

#[test]
fn missing_pitch_dependencies_preserve_original_prefix() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = Native::new(&mut source);
    for seed in 0..=u8::MAX {
        native.seed(&mut source, u16::from(seed) * 257, 0, 0x800, 0xC00);
        let before = *native.records();
        native.world.processed_player_input = None;
        source.run(0x06E3F7, Some(0x06E40E), 0, OWNER, true);
        assert_eq!(
            native.advance(Operation::FlightPitch),
            Err(VerticalError::MissingProcessedInput)
        );
        native.compare(&source, before);
        native.seed(&mut source, u16::from(seed) * 257, 0, 0x800, 0xC00);
        native
            .records()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .hold_secondary_protection = true;
        source.bus.write8(WRAM + SLOT + 0x6B7D, 0x80);
        let before = *native.records();
        native.world.environment_plane_height = None;
        source.run(0x06E3F7, Some(0x06E48B), 0, OWNER, true);
        assert_eq!(
            native.advance(Operation::FlightPitch),
            Err(VerticalError::MissingEnvironmentPlane)
        );
        native.compare(&source, before);
    }
}
