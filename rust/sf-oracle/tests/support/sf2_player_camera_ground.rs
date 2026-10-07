//! Ground pitch, the real aiming proxy, and complete common-camera caller.
#[path = "sf2_player_camera_surface.rs"]
mod surface_tests;
#[path = "sf2_player_camera_auxiliary.rs"]
mod auxiliary_tests;
use super::super::super::super::VIEW;
use super::{rom, CameraFixture, PositionFixture, Source, OWNER, SLOT, WRAM};
use sf2_game::path_appearance::AnimationControl;
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_camera_common;
use sf2_game::player_camera_ground::{self, PlayerCameraGround};
use sf2_game::player_storage::{self, PlayerStorage};
use sf2_game::program_state::ProgramData;
use sf2_game::view_transition::FixedViewAngles;
use sf2_game::{Angle, Buttons, InputState, Vector3};

struct GroundFixture {
    position: PositionFixture,
    runtime: PathRuntime,
}
impl GroundFixture {
    fn new() -> Self {
        let mut position = PositionFixture::new();
        let mut runtime = PathRuntime::default();
        let f = &mut position.height.camera.inner;
        let mut records = *f.world.player(&f.objects, f.owner).unwrap();
        let storage = runtime
            .resources
            .allocate_owned(
                f.owner,
                472,
                ProgramData::PlayerStorage(PlayerStorage {
                    fine_pitch: 0,
                    fine_yaw: 0,
                    bank: Angle::ZERO,
                    retained_shield: 37,
                }),
            )
            .unwrap();
        f.objects.get_mut(f.owner).unwrap().base.player_storage = Some(storage);
        records.camera_ground = Some(Default::default());
        records.boundary = Some(Default::default());
        records.surface = Some(Default::default());
        records.consumable = Some(Default::default());
        records.pose = Some(Default::default());
        f.world.bind_player(&f.objects, f.owner, records).unwrap();
        f.world.environment_plane_height = Some(0);
        f.world.surface_mode = Some(Default::default());
        Self { position, runtime }
    }
    fn values(&self) -> Vec<(u32, u16, bool)> {
        let f = &self.position.height.camera.inner;
        let r = f.world.player(&f.objects, f.owner).unwrap();
        let ground = r.camera_ground.unwrap();
        let actor = f.objects.get(f.owner).unwrap();
        let storage = player_storage::get(&f.objects, &self.runtime.resources, f.owner).unwrap();
        let mut values = vec![];
        for (field, value) in [
            (0x6B5A, ground.height_offset as u16),
            (0x6B5C, ground.height_target_adjustment as u16),
            (0x6BF3, ground.carried_target_height as u16),
            (0x6B37, ground.animation_pitch as u16),
            (0x6BED, r.boundary.unwrap().return_position.x as u16),
            (0x6BEF, r.boundary.unwrap().return_position.y as u16),
            (0x6BF1, r.boundary.unwrap().return_position.z as u16),
            (0x6AB9, storage.fine_pitch),
            (0x6ABB, storage.fine_yaw),
            (0x6AD0, r.pose.unwrap().turning_lean),
        ] {
            values.push((WRAM + SLOT + field, value, false));
        }
        for (field, value) in [
            (WRAM + 0x149D, self.runtime.steering.unchanged_axes),
            (WRAM + 0x1B4D, f.world.surface_mode.unwrap().flags),
            (WRAM + SLOT + 0x6A82, r.surface.unwrap().material),
            (
                WRAM + SLOT + 0x6AC0,
                r.steering.unwrap().camera_bank_target as u8,
            ),
            (
                WRAM + u32::from(OWNER) + 0x24,
                0xAD | u8::from(actor.base.flags.standing_on_surface) * 2,
            ),
            (WRAM + u32::from(OWNER) + 0x16, actor.base.roll.units()),
            (
                WRAM + u32::from(OWNER) + 0x1CCB,
                actor.extension.path_state.animation.shape.packed(),
            ),
        ] {
            values.push((field, u16::from(value), true));
        }
        values.push((
            WRAM + 0x1E0F,
            f.world.environment_plane_height.unwrap() as u16,
            false,
        ));
        values
    }
    fn seed(&mut self, source: &mut Source) {
        self.position.seed(source);
        for (field, value, byte) in self.values() {
            if byte {
                source.bus.write8(field, value as u8);
            } else {
                source.bus.write16(field, value);
            }
        }
    }
    fn verify(&self, source: &Source, phase: &str) {
        self.position.verify(source, phase);
        for (field, value, byte) in self.values() {
            let actual = if byte {
                u16::from(source.bus.read8(field))
            } else {
                source.bus.read16(field)
            };
            assert_eq!(value, actual, "{phase} field={field:06X}");
        }
    }
    fn pitch(&mut self, source: &mut Source) {
        source.run(0x0781F1, None, 0, OWNER, true);
        let f = &mut self.position.height.camera.inner;
        player_camera_ground::advance_pitch(
            &mut f.objects,
            &mut f.world,
            &mut self.runtime,
            f.owner,
        )
        .unwrap();
        self.verify(source, "ground pitch");
    }
    fn orient(&mut self, source: &mut Source) {
        source.run_with_y(0x078546, Some(0x0786B4), 0, OWNER, true, Some(SLOT as u16));
        let f = &mut self.position.height.camera.inner;
        player_camera_common::advance_yaw_roll(&f.objects, &mut f.world, &self.runtime, f.owner)
            .unwrap();
        self.verify(source, "common yaw/roll");
    }
    fn common(&mut self, source: &mut Source) {
        CameraFixture::near(source, 0x0784EC);
        let f = &mut self.position.height.camera.inner;
        player_camera_common::advance(
            &mut f.objects,
            &mut f.world,
            &mut self.runtime,
            f.owner,
            self.position.height.style,
            self.position.height.auxiliary_camera,
        )
        .unwrap();
        // The caller consumes the prepared vector as local scratch. Compare
        // every retained owner, never copy original outputs into native state.
        // Separate prefix tests compare the local vector itself directly.
        self.position.height.check_prepared = false;
        self.verify(source, "complete common caller");
    }
}

#[test]
fn camera_aim_target_matches_original_all_byte_offsets_and_yaws_in_both_families() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = GroundFixture::new();
    for mode in [0x11, 0x21] {
        for word in 0..=u16::MAX {
            let f = &mut fixture.position.height.camera.inner;
            f.records().auxiliary.as_mut().unwrap().mode = mode;
            let storage =
                player_storage::get_mut(&f.objects, &mut fixture.runtime.resources, f.owner)
                    .unwrap();
            storage.fine_yaw = word;
            f.records().boundary.as_mut().unwrap().return_position = Vector3 {
                x: word as i16,
                y: 0x1539,
                z: word.rotate_left(3) as i16,
            };
            f.records()
                .camera_ground
                .as_mut()
                .unwrap()
                .carried_target_height = !word as i16;
            let actor = f.objects.get_mut(f.owner).unwrap();
            actor.base.position = Vector3 {
                x: word as i16,
                y: word.rotate_left(7) as i16,
                z: !word as i16,
            };
            actor.base.yaw = Angle::from_units((word >> 8) as u8);
            actor.base.pitch = Angle::from_units(word as u8);
            actor.base.roll = Angle::from_units(word.rotate_left(3) as u8);
            f.objects.get_mut(f.view).unwrap().base.position = Vector3 {
                x: word.wrapping_mul(73) as i16,
                y: -133,
                z: word.wrapping_mul(977) as i16,
            };
            fixture.runtime.steering.unchanged_axes = 0xA7;
            fixture.seed(&mut source);
            let lateral = word as i8;
            let vertical = word.rotate_left(3) as i8;
            let forward = word.rotate_left(5) as i8;
            source.bus.write8(2, lateral as u8);
            source.bus.write8(8, vertical as u8);
            source.bus.write8(0x97, forward as u8);
            source.run_with_y(0x079721, None, 0, OWNER, true, Some(VIEW as u16));
            let f = &mut fixture.position.height.camera.inner;
            let actual = player_camera_ground::aim_target(
                &mut f.objects,
                &f.world,
                &mut fixture.runtime,
                f.owner,
                lateral,
                vertical,
                forward,
            )
            .unwrap();
            assert_eq!(
                actual.pitch,
                source.bus.read16(4),
                "mode={mode:02X} word={word:04X}"
            );
            assert_eq!(
                actual.yaw,
                source.bus.read16(10),
                "mode={mode:02X} word={word:04X}"
            );
            fixture.verify(&source, "aim target");
        }
    }
}

#[test]
fn camera_ground_pitch_matches_original_all_control_and_mode_bytes() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = GroundFixture::new();
    for mode in 0..=255_u16 {
        for flags in 0..=255_u16 {
            let f = &mut fixture.position.height.camera.inner;
            f.records().auxiliary.as_mut().unwrap().mode = mode as u8;
            f.records().camera_ground = Some(PlayerCameraGround {
                hold_pitch: flags & 0x10 != 0,
                follow_environment_plane: flags & 8 != 0,
                height_offset: mode.wrapping_mul(367) as i16,
                height_target_adjustment: flags.wrapping_mul(613) as i16,
                carried_target_height: mode.wrapping_mul(911) as i16,
                animation_pitch: flags.wrapping_mul(197) as i16,
            });
            f.records()
                .contact
                .as_mut()
                .unwrap()
                .hit
                .hold_secondary_protection = flags & 0x80 != 0;
            f.records().consumable.as_mut().unwrap().recovery_blocked = flags & 0x40 != 0;
            f.records().motion.as_mut().unwrap().walker_contact_control = mode as u8;
            f.records().surface.as_mut().unwrap().material = flags as u8 & 3;
            f.set_flags((flags as u8 & 3) << 6);
            f.world.surface_mode.as_mut().unwrap().flags = (mode & 7) as u8;
            f.world.player_carry_mode = Some(if flags & 4 != 0 { 1 } else { 2 });
            f.world.environment_plane_height = Some(mode.wrapping_mul(257) as i16);
            let actor = f.objects.get_mut(f.owner).unwrap();
            actor.base.flags.standing_on_surface = flags & 0x20 != 0;
            actor.extension.path_state.motion.carry_selected_player = flags & 1 != 0;
            actor.extension.path_state.animation.shape = AnimationControl::from_packed(mode as u8);
            actor.base.position.y = flags.wrapping_mul(367) as i16;
            fixture.position.height.camera.set_angles(FixedViewAngles {
                pitch: mode.wrapping_mul(233).wrapping_add(flags),
                yaw: 0xCDEF,
                roll: 0xABCD,
            });
            fixture.seed(&mut source);
            fixture.pitch(&mut source);
        }
    }
}

#[test]
fn camera_ground_pitch_matches_original_every_word_in_height_follow_and_animation_blend() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = GroundFixture::new();
    for branch in 0..6 {
        for word in 0..=u16::MAX {
            let f = &mut fixture.position.height.camera.inner;
            f.records().auxiliary.as_mut().unwrap().mode =
                if branch & 1 != 0 { 0x21 } else { 0x31 };
            f.records().camera_ground = Some(PlayerCameraGround {
                height_offset: word as i16,
                height_target_adjustment: if branch < 2 {
                    0
                } else {
                    word.rotate_left(7) as i16
                },
                carried_target_height: word.wrapping_mul(193) as i16,
                animation_pitch: word as i16,
                follow_environment_plane: branch == 3,
                ..Default::default()
            });
            f.records().consumable.as_mut().unwrap().recovery_blocked = branch >= 4;
            f.records().motion.as_mut().unwrap().walker_contact_control =
                if branch == 1 { 8 } else { 0 };
            f.records().surface.as_mut().unwrap().material = 1;
            f.world.surface_mode.as_mut().unwrap().flags = if branch >= 4 { 2 } else { 0 };
            f.world.environment_plane_height = Some(word.rotate_left(5) as i16);
            f.records()
                .view_distance
                .as_mut()
                .unwrap()
                .pitch_height_offset = word.rotate_left(11) as i16;
            let actor = f.objects.get_mut(f.owner).unwrap();
            actor.base.flags.standing_on_surface = branch != 1;
            actor.extension.path_state.animation.shape = AnimationControl::from_packed(word as u8);
            actor.base.position = Vector3 {
                x: word as i16,
                y: word.rotate_left(3) as i16,
                z: !word as i16,
            };
            fixture.position.height.camera.set_angles(FixedViewAngles {
                pitch: word,
                yaw: 0x1273,
                roll: 0xACDF,
            });
            fixture.seed(&mut source);
            fixture.pitch(&mut source);
        }
    }
}

#[test]
fn camera_common_orientation_matches_original_all_action_and_gate_combinations() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = GroundFixture::new();
    for action in 0..=255_u16 {
        for gates in 0..128_u16 {
            let f = &mut fixture.position.height.camera.inner;
            f.records().auxiliary.as_mut().unwrap().action_flags = action as u8;
            f.set_flags((gates as u8 & 3) << 6);
            f.records().contact.as_mut().unwrap().ignores_contacts = gates & 4 != 0;
            f.records().camera_position.as_mut().unwrap().lateral_offset =
                [-100, -90, 0, 90, 100, -32768, 32767, 89][usize::from(gates / 8 & 7)];
            f.world.processed_player_input = Some(InputState {
                held: Buttons::from_bits((gates & 3) << 8),
                pressed: Default::default(),
            });
            f.records().pose.as_mut().unwrap().turning_lean =
                action.wrapping_mul(613).wrapping_add(gates);
            f.records().steering.as_mut().unwrap().camera_bank_target = action as i8;
            f.records().camera_angles.as_mut().unwrap().yaw_offset =
                gates.wrapping_mul(1237) as i16;
            f.records().camera_angles.as_mut().unwrap().yaw_difference = -79;
            player_storage::get_mut(&f.objects, &mut fixture.runtime.resources, f.owner)
                .unwrap()
                .fine_yaw = action.wrapping_mul(733).wrapping_add(gates);
            fixture.position.height.camera.set_angles(FixedViewAngles {
                pitch: 0x1234,
                yaw: 0xCDEF,
                roll: action.wrapping_mul(977).wrapping_add(gates),
            });
            fixture.seed(&mut source);
            fixture.orient(&mut source);
        }
    }
}

#[test]
fn camera_common_orientation_matches_original_every_word_in_lean_yaw_and_roll() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = GroundFixture::new();
    for branch in 0..6 {
        for word in 0..=u16::MAX {
            let f = &mut fixture.position.height.camera.inner;
            f.records().auxiliary.as_mut().unwrap().action_flags = if branch == 0 { 1 } else { 5 };
            f.set_flags(if branch == 1 {
                0x80
            } else if branch == 2 {
                0xC0
            } else {
                0
            });
            f.records().contact.as_mut().unwrap().ignores_contacts = branch == 5;
            f.records().camera_position.as_mut().unwrap().lateral_offset =
                if branch == 3 { word as i16 } else { -100 };
            f.world.processed_player_input = Some(InputState {
                held: Buttons::from_bits(0x300),
                pressed: Default::default(),
            });
            f.records().pose.as_mut().unwrap().turning_lean = word;
            f.records().steering.as_mut().unwrap().camera_bank_target = (word >> 8) as i8;
            f.records().camera_angles.as_mut().unwrap().yaw_offset = word.rotate_left(3) as i16;
            f.records().camera_angles.as_mut().unwrap().yaw_difference = -91;
            player_storage::get_mut(&f.objects, &mut fixture.runtime.resources, f.owner)
                .unwrap()
                .fine_yaw = word;
            fixture.position.height.camera.set_angles(FixedViewAngles {
                pitch: 0xFEDC,
                yaw: !word,
                roll: word,
            });
            fixture.seed(&mut source);
            fixture.orient(&mut source);
        }
    }
}

#[test]
fn common_camera_original_caller_and_live_view_consumers_retain_independent_state() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = GroundFixture::new();
    fixture.seed(&mut source);
    for visit in 0..8192_u16 {
        let f = &mut fixture.position.height.camera.inner;
        let mode = [0x11, 0x21, 0x31][usize::from(visit / 97 % 3)];
        f.records().auxiliary.as_mut().unwrap().mode = mode;
        source.bus.write8(WRAM + SLOT + 0x6AA0, mode);
        let action = [1, 5, 0x41, 0x21][usize::from(visit / 43 & 3)];
        f.records().auxiliary.as_mut().unwrap().action_flags = action;
        source.bus.write8(WRAM + SLOT + 0x6B77, action);
        let held = [0, 0x100, 0x200, 0x800, 0x400][usize::from(visit / 31 % 5)];
        f.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(held),
            pressed: Default::default(),
        });
        source.bus.write16(WRAM + 0x1938, held);
        for (field, value, coord) in [
            (12, visit.wrapping_mul(13), 0),
            (14, visit.wrapping_mul(3), 1),
            (16, visit.wrapping_mul(19), 2),
        ] {
            let position = &mut f.objects.get_mut(f.owner).unwrap().base.position;
            match coord {
                0 => position.x = value as i16,
                1 => position.y = value as i16,
                _ => position.z = value as i16,
            };
            source.bus.write16(WRAM + u32::from(OWNER) + field, value);
        }
        let yaw = visit.wrapping_mul(47);
        player_storage::get_mut(&f.objects, &mut fixture.runtime.resources, f.owner)
            .unwrap()
            .fine_yaw = yaw;
        source.bus.write16(WRAM + SLOT + 0x6ABB, yaw);
        let lean = visit.wrapping_mul(173);
        f.records().pose.as_mut().unwrap().turning_lean = lean;
        source.bus.write16(WRAM + SLOT + 0x6AD0, lean);
        let stance = (visit / 53) as u8;
        f.records().motion.as_mut().unwrap().walker_contact_control = stance;
        source.bus.write8(WRAM + SLOT + 0x6B94, stance);
        let standing = visit / 59 & 1 != 0;
        f.objects
            .get_mut(f.owner)
            .unwrap()
            .base
            .flags
            .standing_on_surface = standing;
        source.bus.write8(
            WRAM + u32::from(OWNER) + 0x24,
            0xAD | u8::from(standing) * 2,
        );
        let frame = (visit / 7) as u8;
        f.objects
            .get_mut(f.owner)
            .unwrap()
            .extension
            .path_state
            .animation
            .shape = AnimationControl::from_packed(frame);
        source.bus.write8(WRAM + u32::from(OWNER) + 0x1CCB, frame);
        fixture.common(&mut source);
        fixture.position.height.camera.publish(&mut source);
        fixture
            .position
            .height
            .camera
            .inner
            .step(&mut source, false, true);
        fixture.verify(&source, "retained whole common/view");
    }
}
