//! Position helpers and their unmodified common-camera caller prefix.
#[path = "sf2_player_camera_ground.rs"]
mod ground_tests;
use super::super::super::PROXY;
use super::super::CameraFixture;
use super::{rom, HeightFixture, Source, OWNER, SLOT, WRAM};
use sf2_game::player_camera_position::{self, PlayerCameraPosition};
use sf2_game::player_camera_tracking::TrackingStyle;
use sf2_game::{Angle, Buttons, InputState, Vector3};

struct PositionFixture {
    height: HeightFixture,
    prepared: Vector3,
}
impl PositionFixture {
    fn new() -> Self {
        let mut height = HeightFixture::new();
        let f = &mut height.camera.inner;
        f.records().camera_position = Some(Default::default());
        f.records().occupancy_exempt = Some(false);
        f.records().motion = Some(Default::default());
        f.records().steering = Some(Default::default());
        f.records().ambient = Some(Default::default());
        Self {
            height,
            prepared: Vector3::default(),
        }
    }
    fn values(&self) -> Vec<(u32, u16, bool)> {
        let f = &self.height.camera.inner;
        let records = f.world.player(&f.objects, f.owner).unwrap();
        let state = records.camera_position.unwrap();
        let actor = f.objects.get(f.owner).unwrap();
        let proxy = f.objects.get(f.proxy).unwrap();
        let mut values = vec![];
        for (field, value) in [
            (0x6B4A, state.lateral_offset),
            (0x6B4C, state.secondary_lateral_offset),
            (0x6AE7, state.lateral_accumulator),
            (0x6B52, state.longitudinal_offset),
            (0x6B15, records.steering.unwrap().lateral_offset),
            (0x6AE2, records.ambient.unwrap().retained_offset),
        ] {
            values.push((WRAM + SLOT + field, value as u16, false));
        }
        for (field, value) in [
            (
                0x6BEB,
                0x57 | u8::from(records.occupancy_exempt.unwrap()) * 0x80,
            ),
            (
                0x6B7D,
                records
                    .camera_ground
                    .map(|ground| {
                        3 | u8::from(ground.hold_pitch) * 0x10
                            | u8::from(ground.follow_environment_plane) * 8
                            | u8::from(records.consumable.unwrap().recovery_blocked) * 0x40
                            | u8::from(
                                records
                                    .camera_surface
                                    .map(|surface| surface.returning_below_plane)
                                    .unwrap_or(false),
                            ) * 0x20
                    })
                    .unwrap_or(0x53)
                    | u8::from(records.contact.unwrap().hit.hold_secondary_protection) * 0x80,
            ),
            (0x6BE6, records.motion.unwrap().contact_flags),
            (0x6AAD, records.motion.unwrap().lateral_impulse as u8),
            (0x6AAE, 0xED),
        ] {
            values.push((WRAM + SLOT + field, u16::from(value), true));
        }
        for (field, value) in [
            (WRAM + 0x1DC2, self.prepared.x),
            (WRAM + 0x1DC6, self.prepared.z),
            (WRAM + u32::from(OWNER) + 12, actor.base.position.x),
            (WRAM + u32::from(OWNER) + 16, actor.base.position.z),
            (WRAM + PROXY + 12, proxy.base.position.x),
            (WRAM + PROXY + 14, proxy.base.position.y),
            (WRAM + PROXY + 16, proxy.base.position.z),
        ] {
            values.push((field, value as u16, false));
        }
        values.push((
            WRAM + u32::from(OWNER) + 0x14,
            u16::from(actor.base.yaw.units()),
            true,
        ));
        values.push((
            WRAM + u32::from(OWNER) + 0x1CE8,
            if actor.extension.surface_contact.supporting_object.is_some() {
                PROXY as u16
            } else {
                0
            },
            false,
        ));
        if !self.height.check_prepared {
            values.retain(|(field, _, _)| ![WRAM + 0x1DC2, WRAM + 0x1DC6].contains(field));
        }
        values
    }
    fn seed(&mut self, source: &mut Source) {
        self.height.prepared = self.prepared.y;
        self.height.seed(source);
        for (field, value, byte) in self.values() {
            if byte {
                source.bus.write8(field, value as u8)
            } else {
                source.bus.write16(field, value)
            }
        }
    }
    fn verify(&self, source: &Source, phase: &str) {
        self.height.verify(source, phase);
        for (field, value, byte) in self.values() {
            let original = if byte {
                u16::from(source.bus.read8(field))
            } else {
                source.bus.read16(field)
            };
            assert_eq!(value, original, "{phase} field={field:06X}");
        }
    }
    fn lateral(&mut self, source: &mut Source) {
        CameraFixture::near(source, 0x0788BF);
        let f = &mut self.height.camera.inner;
        self.prepared = player_camera_position::advance_lateral(
            &f.objects,
            &mut f.world,
            f.owner,
            self.prepared,
        )
        .unwrap();
        self.height.prepared = self.prepared.y;
        self.verify(source, "lateral");
    }
    fn distance(&mut self, source: &mut Source) {
        CameraFixture::near(source, 0x078A7A);
        let f = &mut self.height.camera.inner;
        self.prepared = player_camera_position::advance_distance(
            &f.objects,
            &mut f.world,
            f.owner,
            self.prepared,
            self.height.style,
        )
        .unwrap();
        self.height.prepared = self.prepared.y;
        self.verify(source, "distance");
    }
    fn boost(&mut self, source: &mut Source) {
        CameraFixture::near(source, 0x078BD3);
        let f = &mut self.height.camera.inner;
        player_camera_position::advance_boost(&mut f.objects, &mut f.world, f.owner).unwrap();
        self.verify(source, "boost");
    }
    fn prepare(&mut self, source: &mut Source) {
        source.run(0x0784EC, Some(0x078530), 0, OWNER, true);
        let f = &mut self.height.camera.inner;
        self.prepared = player_camera_position::prepare(
            &mut f.objects,
            &mut f.world,
            f.owner,
            self.height.style,
            self.height.auxiliary_camera,
        )
        .unwrap();
        self.height.prepared = self.prepared.y;
        self.verify(source, "complete position prefix");
    }
}

#[test]
fn camera_lateral_matches_original_all_modes_linked_gates_and_contact_recovery_branches() {
    let mut source = Source::new(&rom(), 0);
    let mut f = PositionFixture::new();
    for mode in 0..=255_u16 {
        for gates in 0..128_u16 {
            let inner = &mut f.height.camera.inner;
            inner.records().auxiliary.as_mut().unwrap().mode = mode as u8;
            inner.records().auxiliary.as_mut().unwrap().action_flags =
                0xE0 | ((gates as u8 >> 2) & 1) | ((gates as u8 >> 1) & 4);
            inner.set_flags((gates as u8 & 3) << 6);
            inner.records().motion.as_mut().unwrap().contact_flags = ((gates & 0x30) << 2) as u8;
            inner.records().motion.as_mut().unwrap().lateral_impulse = mode as i8;
            inner.records().steering.as_mut().unwrap().lateral_offset =
                mode.wrapping_mul(537) as i16;
            inner.world.processed_player_input = Some(InputState {
                held: Buttons::from_bits(if gates & 64 != 0 { 0x30 } else { 0 }),
                pressed: Buttons::default(),
            });
            inner
                .records()
                .auxiliary
                .as_mut()
                .unwrap()
                .stored_rotation
                .yaw = Angle::from_units(mode as u8);
            inner.records().camera_position = Some(PlayerCameraPosition {
                lateral_offset: gates.wrapping_mul(521) as i16,
                secondary_lateral_offset: 719,
                lateral_accumulator: mode.wrapping_mul(977).wrapping_sub(gates) as i16,
                longitudinal_offset: -2319,
            });
            inner.records().occupancy_exempt = Some(true);
            f.prepared = Vector3 {
                x: 31991,
                y: -73,
                z: -32761,
            };
            f.seed(&mut source);
            f.lateral(&mut source);
        }
    }
}

#[test]
fn camera_lateral_matches_original_every_word_in_integration_clamps_and_both_recovery_rates() {
    let mut source = Source::new(&rom(), 0);
    let mut f = PositionFixture::new();
    for branch in 0..4 {
        for word in 0..=u16::MAX {
            let inner = &mut f.height.camera.inner;
            inner.records().auxiliary.as_mut().unwrap().action_flags =
                if branch == 1 { 5 } else { 1 };
            inner
                .records()
                .auxiliary
                .as_mut()
                .unwrap()
                .stored_rotation
                .yaw = Angle::from_units(word as u8);
            inner.records().motion.as_mut().unwrap().contact_flags =
                if branch == 2 { 0xC0 } else { 0 };
            inner.records().motion.as_mut().unwrap().lateral_impulse = word as i8;
            inner.records().steering.as_mut().unwrap().lateral_offset = word.rotate_left(3) as i16;
            inner.world.processed_player_input = Some(InputState {
                held: Buttons::from_bits(if branch == 3 { 0x20 } else { 0 }),
                pressed: Buttons::default(),
            });
            let state = inner.records().camera_position.as_mut().unwrap();
            state.lateral_offset = word.rotate_left(7) as i16;
            state.lateral_accumulator = word as i16;
            state.secondary_lateral_offset = 319;
            f.prepared = Vector3 {
                x: word as i16,
                y: 97,
                z: word.wrapping_mul(511) as i16,
            };
            f.seed(&mut source);
            f.lateral(&mut source);
        }
    }
}

#[test]
fn camera_distance_matches_original_every_pitch_yaw_pair_and_all_camera_styles() {
    let mut source = Source::new(&rom(), 0);
    let mut f = PositionFixture::new();
    for style in [
        TrackingStyle::Normal,
        TrackingStyle::ProjectionCorrected,
        TrackingStyle::Surface,
    ] {
        f.height.style = style;
        for word in 0..=u16::MAX {
            let inner = &mut f.height.camera.inner;
            let rotation = &mut inner.records().auxiliary.as_mut().unwrap().stored_rotation;
            rotation.pitch = Angle::from_units(word as u8);
            rotation.yaw = Angle::from_units((word >> 8) as u8);
            inner
                .records()
                .camera_tracking
                .as_mut()
                .unwrap()
                .vertical_offset = word.wrapping_mul(193) as i16;
            inner.records().view_distance.as_mut().unwrap().distance = word as i16;
            inner
                .records()
                .camera_position
                .as_mut()
                .unwrap()
                .longitudinal_offset = word.rotate_left(9) as i16;
            inner.records().occupancy_exempt = Some(word & 1 != 0);
            inner
                .records()
                .contact
                .as_mut()
                .unwrap()
                .hit
                .hold_secondary_protection = word & 2 != 0;
            inner.records().contact.as_mut().unwrap().ignores_contacts = word & 4 != 0;
            inner.set_flags(((word >> 3) as u8 & 3) << 6);
            inner.records().motion.as_mut().unwrap().contact_flags =
                if word & 32 != 0 { 0x80 } else { 0 };
            let proxy = inner.proxy;
            let actor = inner.objects.get_mut(inner.owner).unwrap();
            actor.extension.surface_contact.supporting_object = (word & 64 != 0).then_some(proxy);
            actor.extension.path_state.motion.carry_selected_player = word & 128 != 0;
            inner.world.player_carry_mode = Some(if word & 256 != 0 { 1 } else { 2 });
            f.prepared = Vector3 {
                x: word.wrapping_mul(79) as i16,
                y: word.rotate_left(5) as i16,
                z: !word as i16,
            };
            f.seed(&mut source);
            f.distance(&mut source);
        }
    }
}

#[test]
fn camera_boost_matches_original_every_action_timer_and_linked_gate_combination() {
    let mut source = Source::new(&rom(), 0);
    let mut f = PositionFixture::new();
    for linked in 0..4_u8 {
        for action in 0..=255_u16 {
            for timer in 0..=255_u16 {
                let inner = &mut f.height.camera.inner;
                inner.set_flags(linked << 6);
                inner.records().auxiliary.as_mut().unwrap().action_flags = action as u8;
                inner.records().charge.as_mut().unwrap().speed_impulse_ticks = timer as u8;
                inner.records().charge.as_mut().unwrap().speed_impulse =
                    action.wrapping_mul(631).wrapping_add(timer) as i16;
                inner
                    .records()
                    .view_distance
                    .as_mut()
                    .unwrap()
                    .boost_response = 20;
                inner
                    .records()
                    .view_distance
                    .as_mut()
                    .unwrap()
                    .brake_response = -40;
                inner
                    .records()
                    .camera_position
                    .as_mut()
                    .unwrap()
                    .longitudinal_offset = timer.wrapping_mul(257).wrapping_add(action) as i16;
                let actor = inner.objects.get_mut(inner.owner).unwrap();
                actor.base.pitch = Angle::from_units(action as u8);
                actor.base.yaw = Angle::from_units(timer as u8);
                inner
                    .records()
                    .auxiliary
                    .as_mut()
                    .unwrap()
                    .stored_world_position = Vector3 {
                    x: 31999,
                    y: -123,
                    z: -32761,
                };
                f.seed(&mut source);
                f.boost(&mut source);
            }
        }
    }
}

#[test]
fn camera_boost_matches_original_every_impulse_response_and_profile_word() {
    let mut source = Source::new(&rom(), 0);
    let mut f = PositionFixture::new();
    for branch in 0..4 {
        for word in 0..=u16::MAX {
            let inner = &mut f.height.camera.inner;
            inner.set_flags(if branch == 3 { 0x80 } else { 0 });
            inner.records().auxiliary.as_mut().unwrap().action_flags = match branch {
                0 => 0,
                1 => 0x40,
                _ => 0x20,
            };
            inner.records().charge.as_mut().unwrap().speed_impulse = word as i16;
            inner.records().charge.as_mut().unwrap().speed_impulse_ticks =
                if branch == 2 { 1 } else { 0 };
            inner
                .records()
                .camera_position
                .as_mut()
                .unwrap()
                .longitudinal_offset = word.rotate_left(3) as i16;
            inner
                .records()
                .view_distance
                .as_mut()
                .unwrap()
                .boost_response = word as i16;
            inner
                .records()
                .view_distance
                .as_mut()
                .unwrap()
                .brake_response = word.rotate_left(5) as i16;
            inner.objects.get_mut(inner.owner).unwrap().base.pitch = Angle::from_units(word as u8);
            inner.objects.get_mut(inner.owner).unwrap().base.yaw =
                Angle::from_units((word >> 8) as u8);
            inner
                .records()
                .auxiliary
                .as_mut()
                .unwrap()
                .stored_world_position = Vector3 {
                x: word as i16,
                y: !word as i16,
                z: word.wrapping_mul(933) as i16,
            };
            f.seed(&mut source);
            f.boost(&mut source);
        }
    }
}

#[test]
fn complete_camera_position_prefix_matches_original_with_independent_retained_consumers() {
    let mut source = Source::new(&rom(), 0);
    let mut f = PositionFixture::new();
    f.height
        .camera
        .inner
        .records()
        .camera_angles
        .as_mut()
        .unwrap()
        .profile = sf2_game::player_camera_angles::CameraPitchProfile { up: 12, down: -12 };
    f.height
        .camera
        .inner
        .records()
        .vertical
        .as_mut()
        .unwrap()
        .profile
        .lower_height_offset = -150;
    f.seed(&mut source);
    f.height.camera.inner.step(&mut source, true, false);
    let mut displaced = 0;
    let mut changed_lateral = 0;
    let mut changing_view = 0;
    for visit in 0..16384_u16 {
        // Update only upstream actor/control fields on each side. The actual
        // original caller, not a hand-copied schedule, owns all position work.
        let inner = &mut f.height.camera.inner;
        let held = [0, 0x0800, 0x0400, 0x0030][usize::from(visit / 31 & 3)];
        inner.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(held),
            pressed: Buttons::default(),
        });
        source.bus.write16(WRAM + 0x1938, held);
        source.bus.write16(WRAM + 0x1936, 0);
        let action = [1, 0x41, 0x21, 5, 0][usize::from(visit / 37 % 5)];
        inner.records().auxiliary.as_mut().unwrap().action_flags = action;
        source.bus.write8(WRAM + SLOT + 0x6B77, action);
        let mode = [0x11, 0x11, 0x11, 0x21, 0x31][usize::from(visit / 199 % 5)];
        inner.records().auxiliary.as_mut().unwrap().mode = mode;
        source.bus.write8(WRAM + SLOT + 0x6AA0, mode);
        f.height.style = [
            TrackingStyle::Normal,
            TrackingStyle::ProjectionCorrected,
            TrackingStyle::Surface,
        ][usize::from(visit / 151 % 3)];
        source.bus.write16(
            WRAM + SLOT + 0x6A9A,
            match f.height.style {
                TrackingStyle::Normal => 0x8048,
                TrackingStyle::ProjectionCorrected => 0x8089,
                TrackingStyle::Surface => 0x80B3,
            },
        );
        f.height.auxiliary_camera = visit / 107 % 5 == 4;
        source.bus.write16(
            WRAM + SLOT + 0x6A9D,
            if f.height.auxiliary_camera { 0xB917 } else { 0 },
        );
        let pitch = (visit / 7) as u8;
        let yaw = (visit / 11) as u8;
        let actor = inner.objects.get_mut(inner.owner).unwrap();
        actor.base.pitch = Angle::from_units(pitch);
        actor.base.yaw = Angle::from_units(yaw);
        source.bus.write8(WRAM + u32::from(OWNER) + 0x12, pitch);
        source.bus.write8(WRAM + u32::from(OWNER) + 0x14, yaw);
        for (field, delta, coordinate) in [
            (12, 13_i16, &mut actor.base.position.x),
            (
                14,
                if held & 0x0800 != 0 { 7 } else { -9 },
                &mut actor.base.position.y,
            ),
            (16, -11, &mut actor.base.position.z),
        ] {
            *coordinate = coordinate.wrapping_add(delta);
            let address = WRAM + u32::from(OWNER) + field;
            source.bus.write16(
                address,
                source.bus.read16(address).wrapping_add(delta as u16),
            );
        }
        let steer = ((visit / 17) as i8) as i16;
        let impulse = (visit / 13) as i8;
        let flags = ((visit / 251) as u8 & 3) << 6;
        inner.records().steering.as_mut().unwrap().lateral_offset = steer;
        inner.records().motion.as_mut().unwrap().lateral_impulse = impulse;
        inner.records().motion.as_mut().unwrap().contact_flags = flags;
        source.bus.write16(WRAM + SLOT + 0x6B15, steer as u16);
        source.bus.write8(WRAM + SLOT + 0x6AAD, impulse as u8);
        source.bus.write8(WRAM + SLOT + 0x6BE6, flags);
        let ambient = ((visit / 3) as i8) as i16;
        inner.records().ambient.as_mut().unwrap().retained_offset = ambient;
        source.bus.write16(WRAM + SLOT + 0x6AE2, ambient as u16);
        if visit % 67 == 0 {
            let request = 0x21 | if visit & 1 == 0 { 0x80 } else { 0 };
            inner.world.scene.player_view_control = Some(request);
            source.bus.write8(WRAM + 0x1DE0, request);
        }
        f.prepare(&mut source);
        f.height.camera.pitch(&mut source);
        f.height.camera.publish(&mut source);
        f.height.camera.inner.step(&mut source, false, true);
        f.verify(&source, "retained position/pitch/view");
        let records = f.height.camera.inner.records();
        displaced += usize::from(records.auxiliary.unwrap().stored_world_position != f.prepared);
        changed_lateral += usize::from(records.camera_position.unwrap().lateral_offset != 0);
        changing_view += usize::from(records.charge.unwrap().linked_muzzle_disabled);
    }
    assert!(displaced > 1000, "{displaced}");
    assert!(changed_lateral > 1000, "{changed_lateral}");
    assert!(changing_view > 1000, "{changing_view}");
}
