//! Complete original height tracking with real retained pitch/view consumers.
#[path = "sf2_player_camera_position.rs"]
mod position_tests;

use super::{rom, CameraFixture, Source, OWNER, SLOT, WRAM};
use sf2_game::player_camera_tracking::{self, PlayerCameraTracking, TrackingStyle};
use sf2_game::{Angle, Button, Buttons, InputState};

struct HeightFixture {
    camera: CameraFixture,
    prepared: i16,
    style: TrackingStyle,
    auxiliary_camera: bool,
}
impl HeightFixture {
    fn new() -> Self {
        let mut camera = CameraFixture::new();
        camera.inner.records().camera_tracking = Some(Default::default());
        camera.inner.records().auxiliary.as_mut().unwrap().mode = 0x11;
        camera.inner.world.contacts_enabled = Some(true);
        camera.inner.world.player_carry_mode = Some(1);
        Self {
            camera,
            prepared: -317,
            style: TrackingStyle::Normal,
            auxiliary_camera: false,
        }
    }
    fn values(&self) -> Vec<(u32, u16, bool)> {
        let f = &self.camera.inner;
        let records = f.world.player(&f.objects, f.owner).unwrap();
        let tracking = records.camera_tracking.unwrap();
        let actor = f.objects.get(f.owner).unwrap();
        vec![
            (WRAM + SLOT + 0x6B45, tracking.anchor_height as u16, false),
            (
                WRAM + SLOT + 0x6B47,
                tracking.height_difference as u16,
                false,
            ),
            (WRAM + SLOT + 0x6B4E, tracking.vertical_offset as u16, false),
            (
                WRAM + SLOT + 0x6BF7,
                records.vertical.unwrap().profile.lower_height_offset as u16,
                false,
            ),
            (
                WRAM + SLOT + 0x6A9A,
                match self.style {
                    TrackingStyle::Normal => 0x8048,
                    TrackingStyle::ProjectionCorrected => 0x8089,
                    TrackingStyle::Surface => 0x80B3,
                },
                false,
            ),
            (WRAM + SLOT + 0x6A9C, 7, true),
            (
                WRAM + SLOT + 0x6A9D,
                if self.auxiliary_camera { 0xB917 } else { 0 },
                false,
            ),
            (WRAM + 0x1DC4, self.prepared as u16, false),
            (
                WRAM + 0xD7F4,
                u16::from(f.world.contacts_enabled().unwrap()),
                true,
            ),
            (
                WRAM + 0x1E13,
                u16::from(f.world.player_carry_mode.unwrap()),
                true,
            ),
            (
                WRAM + u32::from(OWNER) + 0x0E,
                actor.base.position.y as u16,
                false,
            ),
            (
                WRAM + u32::from(OWNER) + 0x12,
                u16::from(actor.base.pitch.units()),
                true,
            ),
            (
                WRAM + u32::from(OWNER) + 0x21,
                0xC7 | u16::from(actor.extension.path_state.motion.carry_selected_player) * 0x20,
                true,
            ),
        ]
    }
    fn seed(&self, source: &mut Source) {
        self.camera.seed(source);
        for (field, value, byte) in self.values() {
            if byte {
                source.bus.write8(field, value as u8)
            } else {
                source.bus.write16(field, value)
            }
        }
    }
    fn verify(&self, source: &Source, phase: &str) {
        self.camera.verify(source, phase);
        for (field, value, byte) in self.values() {
            let original = if byte {
                u16::from(source.bus.read8(field))
            } else {
                source.bus.read16(field)
            };
            assert_eq!(value, original, "{phase} field={field:06X}");
        }
    }
    fn height(&mut self, source: &mut Source) {
        CameraFixture::near(source, 0x078D66);
        let f = &mut self.camera.inner;
        self.prepared = player_camera_tracking::advance_height(
            &f.objects,
            &mut f.world,
            f.owner,
            self.prepared,
            self.style,
            self.auxiliary_camera,
        )
        .unwrap();
        self.verify(source, "height");
    }
}

#[test]
fn camera_height_matches_original_all_mode_bytes_and_early_gate_combinations() {
    let mut source = Source::new(&rom(), 0);
    let mut f = HeightFixture::new();
    for mode in 0..=255_u16 {
        for gates in 0..64_u8 {
            let inner = &mut f.camera.inner;
            inner.records().auxiliary.as_mut().unwrap().mode = mode as u8;
            inner.records().auxiliary.as_mut().unwrap().action_flags = 0xF6 | ((gates >> 3) & 1);
            inner.set_flags((gates & 3) << 6 | 0x1B);
            inner.world.contacts_enabled = Some(gates & 4 != 0);
            inner.world.player_carry_mode = Some(if gates & 16 != 0 {
                1
            } else {
                mode as u8 & 0xFE
            });
            let actor = inner.objects.get_mut(inner.owner).unwrap();
            actor.extension.path_state.motion.carry_selected_player = mode & 1 != 0;
            actor.base.pitch = Angle::from_units(mode as u8);
            actor.base.position.y = mode.wrapping_mul(971).wrapping_add(u16::from(gates)) as i16;
            inner.records().camera_tracking = Some(PlayerCameraTracking {
                anchor_height: 23719,
                height_difference: -2191,
                vertical_offset: mode.wrapping_mul(613) as i16,
            });
            inner
                .records()
                .camera_angles
                .as_mut()
                .unwrap()
                .height_control = mode as u8;
            f.auxiliary_camera = gates & 32 != 0;
            f.prepared = mode.wrapping_mul(1091).wrapping_sub(u16::from(gates) * 73) as i16;
            f.seed(&mut source);
            f.height(&mut source);
        }
    }
}

#[test]
fn camera_height_matches_original_all_control_and_pitch_bytes_with_every_direction_and_style() {
    let mut source = Source::new(&rom(), 0);
    let mut f = HeightFixture::new();
    for style in [
        TrackingStyle::Normal,
        TrackingStyle::ProjectionCorrected,
        TrackingStyle::Surface,
    ] {
        f.style = style;
        for input in 0..4_u16 {
            f.camera.inner.world.processed_player_input = Some(InputState {
                held: Buttons::from_bits(input << 10),
                pressed: Buttons::from_bits(0xAE73),
            });
            for control in 0..=255_u16 {
                for pitch in 0..=255_u16 {
                    let inner = &mut f.camera.inner;
                    inner.objects.get_mut(inner.owner).unwrap().base.pitch =
                        Angle::from_units(pitch as u8);
                    inner
                        .records()
                        .camera_angles
                        .as_mut()
                        .unwrap()
                        .height_control = control as u8;
                    inner.records().camera_tracking = Some(PlayerCameraTracking {
                        anchor_height: pitch.wrapping_mul(931) as i16,
                        height_difference: pitch.wrapping_mul(257).wrapping_add(control) as i16,
                        vertical_offset: -73,
                    });
                    inner
                        .records()
                        .vertical
                        .as_mut()
                        .unwrap()
                        .profile
                        .lower_height_offset = control.wrapping_mul(19) as i16;
                    f.prepared = control.wrapping_mul(773).wrapping_sub(pitch * 31) as i16;
                    f.seed(&mut source);
                    f.height(&mut source);
                }
            }
        }
    }
}

#[test]
fn camera_height_matches_original_every_word_in_tracking_recovery_decay_and_carried_scaling() {
    let mut source = Source::new(&rom(), 0);
    let mut f = HeightFixture::new();
    for branch in 0..8 {
        for word in 0..=u16::MAX {
            let inner = &mut f.camera.inner;
            inner.records().auxiliary.as_mut().unwrap().mode = if branch == 5 {
                0x21
            } else if branch == 6 {
                0x31
            } else {
                0x11
            };
            inner.records().auxiliary.as_mut().unwrap().action_flags = u8::from(branch != 4);
            inner.set_flags(if branch == 7 { 0x80 } else { 0 });
            inner
                .objects
                .get_mut(inner.owner)
                .unwrap()
                .extension
                .path_state
                .motion
                .carry_selected_player = true;
            inner.objects.get_mut(inner.owner).unwrap().base.pitch = Angle::from_units(word as u8);
            inner.records().camera_tracking = Some(PlayerCameraTracking {
                anchor_height: word.rotate_left(5) as i16,
                height_difference: word as i16,
                vertical_offset: word as i16,
            });
            inner
                .records()
                .camera_angles
                .as_mut()
                .unwrap()
                .height_control = if branch == 0 { 0xFE } else { 0 };
            inner
                .records()
                .vertical
                .as_mut()
                .unwrap()
                .profile
                .lower_height_offset = if branch == 0 { -100 } else { -10001 };
            inner.world.processed_player_input = Some(InputState {
                held: Buttons::from_bits(match branch {
                    2 => Button::Up as u16,
                    3 => Button::Down as u16,
                    _ => 0,
                }),
                pressed: Buttons::default(),
            });
            f.style = if branch == 2 || branch == 3 {
                TrackingStyle::ProjectionCorrected
            } else {
                TrackingStyle::Normal
            };
            f.prepared = if branch == 0 { 0 } else { word as i16 };
            f.seed(&mut source);
            f.height(&mut source);
        }
    }
}

#[test]
fn camera_height_and_pitch_retain_independent_state_through_real_view_changes() {
    let mut source = Source::new(&rom(), 0);
    let mut f = HeightFixture::new();
    f.camera
        .inner
        .records()
        .camera_angles
        .as_mut()
        .unwrap()
        .profile = sf2_game::player_camera_angles::CameraPitchProfile { up: 12, down: -12 };
    f.camera
        .inner
        .records()
        .vertical
        .as_mut()
        .unwrap()
        .profile
        .lower_height_offset = -150;
    f.seed(&mut source);
    f.camera.inner.step(&mut source, true, false);
    let mut tracking_visits = 0;
    let mut linked_visits = 0;
    let mut pitch_visits = 0;
    for visit in 0..8192_u16 {
        // Only actual upstream inputs change. Every anchor, difference, height
        // flag, fine angle, increment, linked transition and blend result stays
        // independently live on its own side throughout the sequence.
        let held = [0, Button::Up as u16, Button::Down as u16, 0x0C00][usize::from(visit / 37 & 3)];
        f.camera.inner.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(held),
            pressed: Buttons::default(),
        });
        source.bus.write16(WRAM + 0x1938, held);
        source.bus.write16(WRAM + 0x1936, 0);
        let actor = f
            .camera
            .inner
            .objects
            .get_mut(f.camera.inner.owner)
            .unwrap();
        actor.base.position.y =
            actor
                .base
                .position
                .y
                .wrapping_add(if held & 0x0800 != 0 { 13 } else { -7 });
        let field = WRAM + u32::from(OWNER) + 0x0E;
        source.bus.write16(
            field,
            source
                .bus
                .read16(field)
                .wrapping_add(if held & 0x0800 != 0 {
                    13
                } else {
                    (-7_i16) as u16
                }),
        );
        actor.base.pitch = Angle::from_units((visit / 13) as u8);
        source
            .bus
            .write8(WRAM + u32::from(OWNER) + 0x12, (visit / 13) as u8);
        f.prepared = actor.base.position.y;
        source.bus.write16(WRAM + 0x1DC4, source.bus.read16(field));
        let limits = (visit / 31) as u8;
        f.camera
            .inner
            .records()
            .vertical
            .as_mut()
            .unwrap()
            .limit_flags = limits;
        source.bus.write8(WRAM + SLOT + 0x6B81, limits);
        if visit % 131 == 0 {
            let request = 0x21 | if visit & 1 == 0 { 0x80 } else { 0 };
            f.camera.inner.world.scene.player_view_control = Some(request);
            source.bus.write8(WRAM + 0x1DE0, request);
        }
        f.height(&mut source);
        f.camera.pitch(&mut source);
        f.camera.publish(&mut source);
        f.camera.inner.step(&mut source, false, true);
        f.verify(&source, "retained height/pitch/view");
        let records = f.camera.inner.records();
        tracking_visits += usize::from(records.camera_tracking.unwrap().height_difference != 0);
        linked_visits += usize::from(records.charge.unwrap().linked_mode);
        pitch_visits += usize::from(records.camera_angles.unwrap().pitch_increment != 0);
    }
    assert!(tracking_visits > 500, "{tracking_visits}");
    assert!(linked_visits > 500, "{linked_visits}");
    assert!(pitch_visits > 500, "{pitch_visits}");
}
