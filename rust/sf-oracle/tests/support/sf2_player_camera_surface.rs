//! Independent surface-camera height and complete placement caller.
use super::{rom, CameraFixture, GroundFixture, Source, OWNER, SLOT, WRAM};
use sf2_game::player_camera_surface;
use sf2_game::player_camera_tracking::TrackingStyle;
use sf2_game::player_storage;
use sf2_game::view_transition::FixedViewAngles;
use sf2_game::Vector3;

struct SurfaceFixture {
    ground: GroundFixture,
}
impl SurfaceFixture {
    fn new() -> Self {
        let mut ground = GroundFixture::new();
        ground.position.height.style = TrackingStyle::Surface;
        ground.position.height.check_prepared = false;
        let f = &mut ground.position.height.camera.inner;
        f.records().camera_surface = Some(Default::default());
        f.world.scene.player_configuration = Some(9);
        Self { ground }
    }
    fn values(&self) -> Vec<(u32, u16, bool)> {
        let f = &self.ground.position.height.camera.inner;
        let records = f.world.player(&f.objects, f.owner).unwrap();
        vec![
            (
                WRAM + SLOT + 0x6AF7,
                records.motion.unwrap().surface_height as u16,
                false,
            ),
            (
                WRAM + SLOT + 0x6A7D,
                records.surface.unwrap().plane_height as u16,
                false,
            ),
            (
                WRAM + 0x1DE2,
                u16::from(f.world.scene.player_configuration.unwrap()),
                true,
            ),
        ]
    }
    fn seed(&mut self, source: &mut Source) {
        self.ground.seed(source);
        for (field, value, byte) in self.values() {
            if byte {
                source.bus.write8(field, value as u8);
            } else {
                source.bus.write16(field, value);
            }
        }
    }
    fn verify(&self, source: &Source, phase: &str) {
        self.ground.verify(source, phase);
        for (field, value, byte) in self.values() {
            let actual = if byte {
                u16::from(source.bus.read8(field))
            } else {
                source.bus.read16(field)
            };
            assert_eq!(value, actual, "{phase} field={field:06X}");
        }
    }
    fn height(&mut self, source: &mut Source) {
        CameraFixture::near(source, 0x0790A3);
        let f = &mut self.ground.position.height.camera.inner;
        player_camera_surface::advance_height(
            &f.objects,
            &mut f.world,
            f.owner,
            self.ground.position.height.style,
            self.ground.position.height.auxiliary_camera,
        )
        .unwrap();
        self.verify(source, "surface camera height");
    }
    fn prepare(&mut self, source: &mut Source) {
        CameraFixture::near(source, 0x07812C);
        let f = &mut self.ground.position.height.camera.inner;
        player_camera_surface::prepare(
            &f.objects,
            &mut f.world,
            &self.ground.runtime,
            f.owner,
            self.ground.position.height.style,
            self.ground.position.height.auxiliary_camera,
        )
        .unwrap();
        self.verify(source, "surface camera placement");
    }
}

#[test]
fn surface_camera_height_matches_original_all_control_and_mode_bytes() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = SurfaceFixture::new();
    for mode in 0..=255_u16 {
        for flags in 0..=255_u16 {
            let f = &mut fixture.ground.position.height.camera.inner;
            f.records().auxiliary.as_mut().unwrap().mode = mode as u8;
            f.records().auxiliary.as_mut().unwrap().action_flags = (mode as u8 >> 3) | 0xE0;
            f.records()
                .auxiliary
                .as_mut()
                .unwrap()
                .stored_world_position
                .y = flags.wrapping_mul(733) as i16;
            f.records()
                .contact
                .as_mut()
                .unwrap()
                .hit
                .hold_secondary_protection = flags & 0x80 != 0;
            f.records().contact.as_mut().unwrap().ignores_contacts = mode & 1 != 0;
            f.records().consumable.as_mut().unwrap().recovery_blocked = flags & 0x40 != 0;
            f.records()
                .camera_surface
                .as_mut()
                .unwrap()
                .returning_below_plane = flags & 0x20 != 0;
            let ground = f.records().camera_ground.as_mut().unwrap();
            ground.hold_pitch = flags & 0x10 != 0;
            ground.follow_environment_plane = flags & 8 != 0;
            ground.carried_target_height = mode.wrapping_mul(367) as i16;
            f.set_flags((flags as u8 & 3) << 6);
            f.records().surface.as_mut().unwrap().plane_height = flags.wrapping_mul(977) as i16;
            f.records().motion.as_mut().unwrap().surface_height = mode.wrapping_mul(919) as i16;
            f.records().motion.as_mut().unwrap().walker_contact_control = mode as u8;
            f.world.surface_mode.as_mut().unwrap().flags = mode as u8 & 7;
            f.world.player_carry_mode = Some(if mode & 8 != 0 { 1 } else { 2 });
            f.world.scene.player_configuration = Some(if mode & 4 != 0 { 9 } else { 0 });
            f.world.environment_plane_height = Some(mode.wrapping_mul(257) as i16);
            f.world.contacts_enabled = Some(mode & 16 != 0);
            let actor = f.objects.get_mut(f.owner).unwrap();
            actor.base.position.y = mode.wrapping_mul(733).wrapping_add(flags) as i16;
            actor.base.flags.standing_on_surface = flags & 4 != 0;
            actor.extension.path_state.motion.carry_selected_player = mode & 2 != 0;
            actor.extension.surface_contact.supporting_object = (mode & 32 != 0).then_some(f.proxy);
            fixture.ground.position.height.auxiliary_camera = mode & 64 != 0;
            fixture.seed(&mut source);
            fixture.height(&mut source);
        }
    }
}

#[test]
fn surface_camera_height_matches_original_every_word_in_all_transition_and_follow_paths() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = SurfaceFixture::new();
    for branch in 0..13 {
        for word in 0..=u16::MAX {
            let f = &mut fixture.ground.position.height.camera.inner;
            f.records().auxiliary.as_mut().unwrap().mode = if branch == 8 { 0x11 } else { 0x21 };
            f.records().auxiliary.as_mut().unwrap().action_flags = 1;
            f.records()
                .auxiliary
                .as_mut()
                .unwrap()
                .stored_world_position
                .y = word as i16;
            f.records()
                .contact
                .as_mut()
                .unwrap()
                .hit
                .hold_secondary_protection = branch == 6 || branch == 7;
            f.records().contact.as_mut().unwrap().ignores_contacts = branch == 6;
            f.records().consumable.as_mut().unwrap().recovery_blocked = branch == 3 || branch == 4;
            f.records()
                .camera_surface
                .as_mut()
                .unwrap()
                .returning_below_plane = branch == 5;
            let ground = f.records().camera_ground.as_mut().unwrap();
            ground.hold_pitch = branch == 2;
            ground.follow_environment_plane = branch == 1;
            ground.carried_target_height = word.rotate_left(7) as i16;
            f.set_flags(if branch == 0 { 0x80 } else { 0 });
            f.records().surface.as_mut().unwrap().plane_height = word.rotate_left(11) as i16;
            f.records().motion.as_mut().unwrap().surface_height = word.rotate_left(3) as i16;
            f.records().motion.as_mut().unwrap().walker_contact_control =
                if branch == 12 { 1 } else { 0 };
            f.world.surface_mode.as_mut().unwrap().flags = if branch == 10 { 2 } else { 0 };
            f.world.player_carry_mode = Some(1);
            f.world.scene.player_configuration = Some(9);
            f.world.environment_plane_height = Some(-133);
            let actor = f.objects.get_mut(f.owner).unwrap();
            actor.base.position.y = word as i16;
            actor.base.flags.standing_on_surface = branch == 1;
            actor.extension.path_state.motion.carry_selected_player = branch == 3 || branch == 9;
            actor.extension.surface_contact.supporting_object =
                (branch == 11 || branch == 12).then_some(f.proxy);
            fixture.seed(&mut source);
            fixture.height(&mut source);
        }
    }
}

#[test]
fn surface_camera_placement_matches_original_every_yaw_difference_and_stored_fine_angle() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = SurfaceFixture::new();
    for word in 0..=u16::MAX {
        let f = &mut fixture.ground.position.height.camera.inner;
        f.records().auxiliary.as_mut().unwrap().mode = 0x21;
        f.records().boundary.as_mut().unwrap().return_position = Vector3 {
            x: word.wrapping_mul(73) as i16,
            y: word as i16,
            z: !word as i16,
        };
        f.records().camera_angles.as_mut().unwrap().yaw_difference = word as i16;
        f.records().camera_angles.as_mut().unwrap().yaw_offset = word.rotate_left(3) as i16;
        f.records().view_distance.as_mut().unwrap().distance = word as i16;
        f.records()
            .camera_tracking
            .as_mut()
            .unwrap()
            .vertical_offset = word.rotate_left(7) as i16;
        player_storage::get_mut(&f.objects, &mut fixture.ground.runtime.resources, f.owner)
            .unwrap()
            .fine_yaw = word.wrapping_mul(733);
        fixture
            .ground
            .position
            .height
            .camera
            .set_angles(FixedViewAngles {
                pitch: word.rotate_left(11),
                yaw: !word,
                roll: word,
            });
        fixture.seed(&mut source);
        fixture.prepare(&mut source);
    }
}

#[test]
fn common_and_surface_camera_callers_preserve_independent_live_transition_state() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = SurfaceFixture::new();
    fixture.seed(&mut source);
    fixture
        .ground
        .position
        .height
        .camera
        .inner
        .step(&mut source, true, false);
    let mut protection_releases = 0;
    let mut plane_returns = 0;
    let mut changed_height = 0;
    for visit in 0..8192_u16 {
        let style = if visit / 211 & 1 == 0 {
            TrackingStyle::Surface
        } else {
            TrackingStyle::Normal
        };
        fixture.ground.position.height.style = style;
        source.bus.write16(
            WRAM + SLOT + 0x6A9A,
            if style == TrackingStyle::Surface {
                0x80B3
            } else {
                0x8048
            },
        );
        let f = &mut fixture.ground.position.height.camera.inner;
        let mode = [0x11, 0x21, 0x31][usize::from(visit / 151 % 3)];
        f.records().auxiliary.as_mut().unwrap().mode = mode;
        source.bus.write8(WRAM + SLOT + 0x6AA0, mode);
        f.records().auxiliary.as_mut().unwrap().action_flags = 1;
        source.bus.write8(WRAM + SLOT + 0x6B77, 1);
        let y = ((visit / 3) % 400) as i16 - 450;
        let position = Vector3 {
            x: visit.wrapping_mul(19) as i16,
            y,
            z: visit.wrapping_mul(73) as i16,
        };
        f.objects.get_mut(f.owner).unwrap().base.position = position;
        for (offset, value) in [(12, position.x), (14, position.y), (16, position.z)] {
            source
                .bus
                .write16(WRAM + u32::from(OWNER) + offset, value as u16);
        }
        f.records().boundary.as_mut().unwrap().return_position = position;
        for (offset, value) in [
            (0x6BED, position.x),
            (0x6BEF, position.y),
            (0x6BF1, position.z),
        ] {
            source.bus.write16(WRAM + SLOT + offset, value as u16);
        }
        f.records()
            .camera_ground
            .as_mut()
            .unwrap()
            .carried_target_height = y.wrapping_add(32);
        source
            .bus
            .write16(WRAM + SLOT + 0x6BF3, y.wrapping_add(32) as u16);
        f.records().motion.as_mut().unwrap().surface_height = y;
        source.bus.write16(WRAM + SLOT + 0x6AF7, y as u16);
        f.records().surface.as_mut().unwrap().plane_height = -315;
        source.bus.write16(WRAM + SLOT + 0x6A7D, (-315_i16) as u16);
        f.world.environment_plane_height = Some(-350);
        source.bus.write16(WRAM + 0x1E0F, (-350_i16) as u16);
        let standing = visit / 13 & 1 != 0;
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
        let carried = visit / 37 % 3 == 0;
        f.objects
            .get_mut(f.owner)
            .unwrap()
            .extension
            .path_state
            .motion
            .carry_selected_player = carried;
        source.bus.write8(
            WRAM + u32::from(OWNER) + 0x21,
            0xC7 | u8::from(carried) * 0x20,
        );
        for (period, flag) in [(127, 0x80), (191, 0x40), (53, 8), (89, 0x10)] {
            if visit % period != 0 {
                continue;
            }
            let r = f.records();
            match flag {
                0x80 => r.contact.as_mut().unwrap().hit.hold_secondary_protection = true,
                0x40 => r.consumable.as_mut().unwrap().recovery_blocked = true,
                8 => r.camera_ground.as_mut().unwrap().follow_environment_plane = true,
                _ => r.camera_ground.as_mut().unwrap().hold_pitch = true,
            }
            let address = WRAM + SLOT + 0x6B7D;
            source.bus.write8(address, source.bus.read8(address) | flag);
        }
        if visit % 67 == 0 {
            let request = 0x21 | if visit & 1 == 0 { 0x80 } else { 0 };
            f.world.scene.player_view_control = Some(request);
            source.bus.write8(WRAM + 0x1DE0, request);
        }
        let before = *f.world.player(&f.objects, f.owner).unwrap();
        if style == TrackingStyle::Surface {
            f.step(&mut source, false, false);
            fixture.prepare(&mut source);
        } else {
            fixture.ground.common(&mut source);
        }
        fixture.ground.position.height.camera.publish(&mut source);
        let f = &mut fixture.ground.position.height.camera.inner;
        if style != TrackingStyle::Surface {
            f.step(&mut source, false, false);
        }
        source.run(0x0797FB, None, 0, OWNER, true);
        sf2_game::view_blend::advance(&mut f.objects, &f.world).unwrap();
        let after = *f.world.player(&f.objects, f.owner).unwrap();
        protection_releases += usize::from(
            before.contact.unwrap().hit.hold_secondary_protection
                && !after.contact.unwrap().hit.hold_secondary_protection,
        );
        plane_returns += usize::from(
            before.camera_surface.unwrap().returning_below_plane
                && !after.camera_surface.unwrap().returning_below_plane,
        );
        changed_height += usize::from(
            before.auxiliary.unwrap().stored_world_position.y
                != after.auxiliary.unwrap().stored_world_position.y,
        );
        fixture.verify(&source, "retained surface/common/view transition");
    }
    assert!(protection_releases > 5, "{protection_releases}");
    assert!(plane_returns > 5, "{plane_returns}");
    assert!(changed_height > 1000, "{changed_height}");
}
