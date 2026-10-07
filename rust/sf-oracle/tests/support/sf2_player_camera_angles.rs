//! Original pitch controller, fine-pose publication and retained view changes.
use super::{rom, Fixture, Source, OWNER, SLOT, VIEW, WRAM};
use sf2_game::player_camera_angles::{self, CameraPitchProfile};
use sf2_game::view_blend;
use sf2_game::view_transition::FixedViewAngles;
use sf2_game::{Button, Buttons, InputState, Vector3};
use sf_oracle::{call_near, Entry};

struct CameraFixture {
    inner: Fixture,
}
impl CameraFixture {
    fn new() -> Self {
        let mut inner = Fixture::new();
        inner.records().camera_angles = Some(Default::default());
        inner.records().vertical = Some(Default::default());
        inner.world.processed_player_input = Some(Default::default());
        inner.world.published_camera_roll = Some(0xA55A);
        Self { inner }
    }
    fn set_angles(&mut self, angles: FixedViewAngles) {
        let records = self.inner.records();
        records.camera_angles.as_mut().unwrap().write(
            &mut records.auxiliary.as_mut().unwrap().stored_rotation,
            angles,
        );
    }
    fn values(&self) -> Vec<(u32, u16, bool)> {
        let f = &self.inner;
        let records = f.world.player(&f.objects, f.owner).unwrap();
        let camera = records.camera_angles.unwrap();
        let angles = camera.capture(records.auxiliary.unwrap().stored_rotation);
        let mut values = vec![];
        for (field, value) in [
            (0x6B30, camera.height_control),
            (0x6BFD, camera.profile.up as u8),
            (0x6BFE, camera.profile.down as u8),
            (0x6B81, records.vertical.unwrap().limit_flags),
        ] {
            values.push((WRAM + SLOT + field, u16::from(value), true));
        }
        for (field, value) in [
            (0x6B31, angles.pitch),
            (0x6B33, angles.yaw),
            (0x6B35, angles.roll),
            (0x6B3F, camera.pitch_increment as u16),
            (0x6B3D, camera.yaw_offset as u16),
            (
                0x6B3B,
                records.contact.unwrap().hit.camera_pitch_recoil as u16,
            ),
            (
                0x6AC1,
                records.auxiliary.unwrap().stored_world_position.x as u16,
            ),
            (
                0x6AC3,
                records.auxiliary.unwrap().stored_world_position.y as u16,
            ),
            (
                0x6AC5,
                records.auxiliary.unwrap().stored_world_position.z as u16,
            ),
        ] {
            values.push((WRAM + SLOT + field, value, false));
        }
        for (field, value) in [
            (
                WRAM + 0x1938,
                f.world.processed_player_input.unwrap().held.bits(),
            ),
            (
                WRAM + 0x1936,
                f.world.processed_player_input.unwrap().pressed.bits(),
            ),
            (WRAM + 0x1E0B, f.world.published_camera_roll.unwrap()),
            (
                WRAM + VIEW + 0x29,
                f.objects.get(f.view).unwrap().base.view_rear_distance as u16,
            ),
        ] {
            values.push((field, value, false));
        }
        values
    }
    fn seed(&self, source: &mut Source) {
        self.inner.seed(source);
        for (field, value, byte) in self.values() {
            if byte {
                source.bus.write8(field, value as u8)
            } else {
                source.bus.write16(field, value)
            }
        }
    }
    fn verify(&self, source: &Source, phase: &str) {
        self.inner.verify(source, phase);
        for (field, value, byte) in self.values() {
            let original = if byte {
                u16::from(source.bus.read8(field))
            } else {
                source.bus.read16(field)
            };
            assert_eq!(value, original, "{phase} field={field:06X}");
        }
    }
    fn near(source: &mut Source, address: u32) {
        let result = call_near(
            &mut source.bus,
            address,
            &Entry {
                x: OWNER,
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            },
        );
        assert!(result.returned, "{address:06X}");
    }
    fn pitch(&mut self, source: &mut Source) {
        Self::near(source, 0x0786B7);
        let f = &mut self.inner;
        player_camera_angles::advance_pitch(&f.objects, &mut f.world, f.owner).unwrap();
        self.verify(source, "pitch");
    }
    fn publish(&mut self, source: &mut Source) {
        Self::near(source, 0x07968B);
        Self::near(source, 0x0796F0);
        let f = &mut self.inner;
        player_camera_angles::publish(&mut f.objects, &mut f.world, f.owner).unwrap();
        self.verify(source, "publish");
    }
    fn existing_roll(&mut self, source: &mut Source) {
        Self::near(source, 0x0796DC);
        let f = &mut self.inner;
        player_camera_angles::publish_existing_roll(&f.objects, &mut f.world).unwrap();
        self.verify(source, "existing roll");
    }
}

#[test]
fn camera_pitch_matches_original_every_height_limit_and_linked_control_combination() {
    let mut source = Source::new(&rom(), 0);
    let mut f = CameraFixture::new();
    for linked in 0..4_u8 {
        for height in 0..=255_u16 {
            for limit in 0..=255_u16 {
                f.inner.set_flags(linked << 6 | 0x1B);
                f.inner.records().vertical.as_mut().unwrap().limit_flags = limit as u8;
                let camera = f.inner.records().camera_angles.as_mut().unwrap();
                camera.height_control = height as u8;
                camera.profile = CameraPitchProfile {
                    up: (height as u8).rotate_left(3) as i8,
                    down: limit.wrapping_mul(73) as i8,
                };
                camera.pitch_increment = height.wrapping_mul(257).wrapping_add(limit) as i16;
                f.inner.world.processed_player_input = Some(InputState {
                    held: Buttons::from_bits((limit & 3) << 10),
                    pressed: Buttons::from_bits(0x13A5),
                });
                f.set_angles(FixedViewAngles {
                    pitch: height.wrapping_mul(733) ^ limit.wrapping_mul(143),
                    yaw: 0xACBD,
                    roll: 0xCEAF,
                });
                f.seed(&mut source);
                f.pitch(&mut source);
            }
        }
    }
}

#[test]
fn camera_pitch_matches_original_every_word_in_increment_integration_and_recovery() {
    let mut source = Source::new(&rom(), 0);
    let mut f = CameraFixture::new();
    for branch in 0..6 {
        for word in 0..=u16::MAX {
            f.inner.set_flags(if branch <= 3 { 0x80 } else { 0 });
            let camera = f.inner.records().camera_angles.as_mut().unwrap();
            camera.profile = CameraPitchProfile {
                up: 127,
                down: -128,
            };
            camera.pitch_increment = if branch < 3 { word as i16 } else { -73 };
            camera.height_control = if branch == 4 { 1 } else { 0x18 };
            f.inner.records().vertical.as_mut().unwrap().limit_flags =
                if branch == 3 { 0x0C } else { 0 };
            f.inner.world.processed_player_input = Some(InputState {
                held: Buttons::from_bits(match branch {
                    0 | 3 => Button::Up as u16,
                    1 => Button::Down as u16,
                    _ => 0,
                }),
                pressed: Buttons::default(),
            });
            f.set_angles(FixedViewAngles {
                pitch: word,
                yaw: word.rotate_left(5),
                roll: !word,
            });
            f.seed(&mut source);
            f.pitch(&mut source);
        }
    }
}

#[test]
fn camera_pose_publication_matches_original_every_recoil_word_and_all_fine_alias_bytes() {
    let mut source = Source::new(&rom(), 0);
    let mut f = CameraFixture::new();
    for word in 0..=u16::MAX {
        f.set_angles(FixedViewAngles {
            pitch: word,
            yaw: word.rotate_left(5),
            roll: !word,
        });
        f.inner
            .records()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .camera_pitch_recoil = word as i16;
        f.inner.records().camera_angles.as_mut().unwrap().yaw_offset = word.rotate_left(7) as i16;
        f.inner
            .records()
            .auxiliary
            .as_mut()
            .unwrap()
            .stored_world_position = Vector3 {
            x: word as i16,
            y: word.wrapping_mul(171) as i16,
            z: word.rotate_left(8) as i16,
        };
        f.inner
            .objects
            .get_mut(f.inner.view)
            .unwrap()
            .base
            .view_rear_distance = -313;
        f.seed(&mut source);
        f.publish(&mut source);
        // The scripted branch reads current view rather than retained
        // player roll; change each side's live view by the same external delta.
        let view = f.inner.objects.get_mut(f.inner.view).unwrap();
        let current = FixedViewAngles::capture(view);
        FixedViewAngles {
            roll: current.roll.wrapping_add(73),
            ..current
        }
        .write_to(view);
        let field = WRAM + VIEW + 0x16;
        source
            .bus
            .write16(field, source.bus.read16(field).wrapping_add(73));
        f.existing_roll(&mut source);
    }
}

#[test]
fn camera_pitch_publication_recoil_and_view_transitions_retain_independent_continuous_state() {
    let mut source = Source::new(&rom(), 0);
    let mut f = CameraFixture::new();
    f.inner.records().camera_angles.as_mut().unwrap().profile =
        CameraPitchProfile { up: 12, down: -12 };
    f.inner.records().auxiliary.as_mut().unwrap().mode = 0x11;
    f.inner
        .records()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .camera_pitch_recoil = 31999;
    f.seed(&mut source);
    f.inner.step(&mut source, true, false);
    let mut active = 0;
    let mut transition = 0;
    for visit in 0..8192_u16 {
        // Real upstream inputs only. Height tracking remains its own caller;
        // no pitch, response increment, distance or blend result is replayed.
        let held = [0, Button::Down as u16, Button::Up as u16, 0x0C00][usize::from(visit / 7 & 3)];
        let input = InputState {
            held: Buttons::from_bits(held),
            pressed: Buttons::default(),
        };
        f.inner.world.processed_player_input = Some(input);
        source.bus.write16(WRAM + 0x1938, held);
        source.bus.write16(WRAM + 0x1936, 0);
        let height = (visit / 19) as u8;
        let limits = (visit / 31) as u8;
        f.inner
            .records()
            .camera_angles
            .as_mut()
            .unwrap()
            .height_control = height;
        f.inner.records().vertical.as_mut().unwrap().limit_flags = limits;
        source.bus.write8(WRAM + SLOT + 0x6B30, height);
        source.bus.write8(WRAM + SLOT + 0x6B81, limits);
        if visit % 29 == 0 {
            let request = 0x21 | if visit & 1 == 0 { 0x80 } else { 0 };
            f.inner.world.scene.player_view_control = Some(request);
            source.bus.write8(WRAM + 0x1DE0, request);
        }
        let delta = Vector3 {
            x: 37,
            y: -9,
            z: 53,
        };
        let stored = &mut f
            .inner
            .records()
            .auxiliary
            .as_mut()
            .unwrap()
            .stored_world_position;
        stored.x = stored.x.wrapping_add(delta.x);
        stored.y = stored.y.wrapping_add(delta.y);
        stored.z = stored.z.wrapping_add(delta.z);
        for (field, value) in [(0x6AC1, delta.x), (0x6AC3, delta.y), (0x6AC5, delta.z)] {
            let address = WRAM + SLOT + field;
            source.bus.write16(
                address,
                source.bus.read16(address).wrapping_add(value as u16),
            );
        }
        f.pitch(&mut source);
        source.run(0x079AAB, None, 0, OWNER, true);
        f.inner
            .records()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .advance_pitch_recoil();
        f.publish(&mut source);
        f.inner.step(&mut source, false, true);
        f.verify(&source, "retained view");
        let records = f.inner.records();
        active += usize::from(records.camera_angles.unwrap().pitch_increment != 0);
        transition += usize::from(records.charge.unwrap().linked_muzzle_disabled);
    }
    assert!(active > 1000, "{active}");
    assert!(transition > 1000, "{transition}");
    // Continuity is a consumer, not a producer of stored camera pitch.
    let before = *f.inner.records();
    view_blend::advance(&mut f.inner.objects, &f.inner.world).unwrap();
    assert_eq!(*f.inner.records(), before);
}
