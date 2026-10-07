//! Whole original fixed-view continuity, including both wrapped chases,
//! angular decay, real proxy side effects and final retained snapshots.
use super::{rom, Source, WRAM};
use sf2_game::scene_path_world::ScenePathWorld;
use sf2_game::view_blend::{self, ViewBlendControl};
use sf2_game::view_transition::{FixedViewAngles, ViewTransitionMode};
use sf2_game::weapon_dispatch::WeaponState;
use sf2_game::{
    Angle, Behavior, Object, ObjectId, ObjectKind, ObjectStore, RandomState, Rotation, ShapeId,
    Vector3,
};
use sf_oracle::{call, Entry};

const VIEW: u32 = 0x033F;
const PROXY: u32 = 0x03BD;

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    view: ObjectId,
    proxy: ObjectId,
}
impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let mut allocate = || {
            objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap()
        };
        let view = allocate();
        let proxy = allocate();
        let mut world = ScenePathWorld::new(RandomState::new([17, 93, 171, 213]));
        world.fixed_players[0] = Some(view);
        world.view_transition_mode = Some(ViewTransitionMode { flags: 0xAB00 });
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        Self {
            objects,
            world,
            view,
            proxy,
        }
    }
    fn actor(&mut self) -> &mut Object {
        self.objects.get_mut(self.view).unwrap()
    }
    fn flags(&self) -> u8 {
        let control = ViewBlendControl::capture(self.objects.get(self.view).unwrap());
        7 | u8::from(control.capture_position) * 8
            | u8::from(control.capture_rotation) * 0x10
            | u8::from(control.discard_capture) * 0x20
            | u8::from(control.position_active) * 0x40
            | u8::from(control.rotation_active) * 0x80
    }
    fn values(&self) -> Vec<(u32, u16, bool)> {
        let view = self.objects.get(self.view).unwrap();
        let angles = FixedViewAngles::capture(view);
        let saved = view.extension.path_state.platform_carry.saved_position;
        let previous_angles = view.extension.path_state.motion_delta;
        let relative = view.extension.relative_position;
        let delta = view.extension.relative_rotation;
        let mut values = vec![
            (
                WRAM + 0x1B84,
                self.world.view_transition_mode.unwrap().flags,
                false,
            ),
            (
                WRAM + 0x14D6,
                if self.proxy == self.view {
                    VIEW as u16
                } else {
                    PROXY as u16
                },
                false,
            ),
        ];
        for (offset, value) in [
            (12, view.base.position.x as u16),
            (14, view.base.position.y as u16),
            (16, view.base.position.z as u16),
            (0x12, angles.pitch),
            (0x14, angles.yaw),
            (0x16, angles.roll),
            (0x39, saved.x as u16),
            (0x3B, saved.y as u16),
            (0x3D, saved.z as u16),
            (0x1CC1, previous_angles.x as u16),
            (0x1CC3, previous_angles.y as u16),
            (0x1CC5, previous_angles.z as u16),
            (0x1CCF, relative.x as u16),
            (0x1CD1, relative.y as u16),
            (0x1CD3, relative.z as u16),
            (0x1CE4, view.extension.path_state.script_value),
        ] {
            values.push((WRAM + VIEW + offset, value, false));
        }
        for (offset, value) in [
            (0x0A, view.base.target_speed),
            (0x21, self.flags()),
            (
                0x22,
                0xA4 | u8::from(ViewBlendControl::capture(view).fast_position_recovery),
            ),
            (0x1CD5, delta.pitch.units()),
            (0x1CD6, delta.yaw.units()),
            (0x1CD7, delta.roll.units()),
        ] {
            values.push((WRAM + VIEW + offset, u16::from(value), true));
        }
        let proxy = self.objects.get(self.proxy).unwrap();
        let base = WRAM + if self.proxy == self.view { VIEW } else { PROXY };
        for (offset, value) in [
            (12, proxy.base.position.x),
            (14, proxy.base.position.y),
            (16, proxy.base.position.z),
        ] {
            values.push((base + offset, value as u16, false));
        }
        for (offset, value) in [
            (0x12, proxy.base.pitch.units()),
            (0x14, proxy.base.yaw.units()),
            (0x16, proxy.base.roll.units()),
        ] {
            values.push((base + offset, u16::from(value), true));
        }
        values
    }
    fn seed(&self, source: &mut Source) {
        for (field, value, byte) in self.values() {
            if byte {
                source.bus.write8(field, value as u8)
            } else {
                source.bus.write16(field, value)
            }
        }
        source.bus.write8(WRAM + VIEW + 0x20, 0xD3);
    }
    fn step(&mut self, source: &mut Source) {
        let original = call(
            &mut source.bus,
            0x0797FB,
            &Entry {
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            },
        );
        assert!(original.returned, "whole original view blend must return");
        let random = self.world.random.clone();
        view_blend::advance(&mut self.objects, &self.world).unwrap();
        assert_eq!(self.world.random, random);
        for (field, value, byte) in self.values() {
            assert_eq!(
                value,
                if byte {
                    u16::from(source.bus.read8(field))
                } else {
                    source.bus.read16(field)
                },
                "view blend field {field:06X}"
            );
        }
        assert_eq!(source.bus.read8(WRAM + VIEW + 0x20), 0xD3);
    }
}

#[test]
fn view_blend_matches_original_every_word_and_both_recovery_rates_with_all_control_branches() {
    let mut source = Source::new(&rom(), 0);
    for value in 0..=u16::MAX {
        for fast in [false, true] {
            let mut f = Fixture::new();
            let mode = &mut f.world.view_transition_mode.as_mut().unwrap().flags;
            *mode |= u16::from(value & 4 != 0) * 2;
            let view = f.actor();
            view.base.position = Vector3 {
                x: value as i16,
                y: value.rotate_left(5) as i16,
                z: !value as i16,
            };
            view.base.target_speed = value as u8;
            FixedViewAngles {
                pitch: value,
                yaw: value.rotate_left(5),
                roll: value.rotate_left(11),
            }
            .write_to(view);
            view.extension.path_state.platform_carry.saved_position = Vector3 {
                x: value.wrapping_mul(193) as i16,
                y: value.rotate_left(7) as i16,
                z: value.rotate_left(13) as i16,
            };
            view.extension.path_state.motion_delta = Vector3 {
                x: value.rotate_left(3) as i16,
                y: !value as i16,
                z: value.wrapping_mul(199) as i16,
            };
            view.extension.relative_position = Vector3 {
                x: value as i16,
                y: value.rotate_left(3) as i16,
                z: value.wrapping_mul(197) as i16,
            };
            view.extension.relative_rotation = Rotation {
                pitch: Angle::from_units(value as u8),
                yaw: Angle::from_units((value >> 8) as u8),
                roll: Angle::from_units(value.rotate_left(3) as u8),
            };
            view.extension.path_state.script_value = value.rotate_left(9);
            ViewBlendControl {
                capture_position: value & 8 != 0,
                capture_rotation: value & 0x10 != 0,
                discard_capture: value & 0x20 != 0,
                position_active: value & 0x40 != 0,
                rotation_active: value & 0x80 != 0,
                fast_position_recovery: fast,
            }
            .write_to(view);
            f.seed(&mut source);
            f.step(&mut source);
        }
    }
}

#[test]
fn view_blend_matches_original_all_saved_and_current_heading_pairs_and_proxy_aliases() {
    let mut source = Source::new(&rom(), 0);
    for value in 0..=u16::MAX {
        let mut f = Fixture::new();
        if value & 1 != 0 {
            f.proxy = f.view;
            f.world.weapons.as_mut().unwrap().fallback = Some(f.view);
        }
        let view = f.actor();
        FixedViewAngles {
            pitch: 0x734B,
            yaw: value,
            roll: 0x2DA6,
        }
        .write_to(view);
        view.base.target_speed = value as u8;
        view.base.position = Vector3 {
            x: value as i16,
            y: value.rotate_left(9) as i16,
            z: value.rotate_left(3) as i16,
        };
        view.extension.relative_position = Vector3 {
            x: value.wrapping_mul(197) as i16,
            y: value as i16,
            z: value.rotate_left(7) as i16,
        };
        ViewBlendControl {
            position_active: true,
            ..Default::default()
        }
        .write_to(view);
        f.seed(&mut source);
        f.step(&mut source);
    }
}

#[test]
fn view_blend_matches_original_retained_capture_discard_and_settling_without_output_replay() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    f.seed(&mut source);
    let mut active = 0;
    let mut settled = 0;
    for tick in 0..8192_u16 {
        let mode = 0xAB00 | u16::from(tick % 71 == 0) * 2;
        f.world.view_transition_mode.as_mut().unwrap().flags = mode;
        source.bus.write16(WRAM + 0x1B84, mode);
        let view = f.actor();
        let mut control = ViewBlendControl::capture(view);
        control.capture_position = tick % 37 == 0;
        control.capture_rotation = tick % 53 == 0;
        control.discard_capture = tick % 71 == 0;
        control.fast_position_recovery = tick % 97 < 48;
        control.write_to(view);
        let requests = u8::from(control.capture_position) * 8
            | u8::from(control.capture_rotation) * 0x10
            | u8::from(control.discard_capture) * 0x20;
        source.bus.write8(
            WRAM + VIEW + 0x21,
            (source.bus.read8(WRAM + VIEW + 0x21) & !0x38) | requests,
        );
        source.bus.write8(
            WRAM + VIEW + 0x22,
            0xA4 | u8::from(control.fast_position_recovery),
        );
        // The preceding camera mode changes each implementation's own live
        // pose by the same inputs; neither one's result seeds the other.
        for (offset, axis, delta) in [
            (12, &mut view.base.position.x, (tick % 61) as i16 - 30),
            (14, &mut view.base.position.y, (tick % 47) as i16 - 23),
            (16, &mut view.base.position.z, (tick % 89) as i16 - 44),
        ] {
            *axis = axis.wrapping_add(delta);
            source.bus.write16(
                WRAM + VIEW + offset,
                source
                    .bus
                    .read16(WRAM + VIEW + offset)
                    .wrapping_add(delta as u16),
            );
        }
        let current = FixedViewAngles::capture(view);
        let deltas = [
            tick.rotate_left(1),
            tick.rotate_left(3),
            tick.rotate_left(5),
        ];
        FixedViewAngles {
            pitch: current.pitch.wrapping_add(deltas[0]),
            yaw: current.yaw.wrapping_add(deltas[1]),
            roll: current.roll.wrapping_add(deltas[2]),
        }
        .write_to(view);
        for (offset, delta) in [(0x12, deltas[0]), (0x14, deltas[1]), (0x16, deltas[2])] {
            source.bus.write16(
                WRAM + VIEW + offset,
                source.bus.read16(WRAM + VIEW + offset).wrapping_add(delta),
            );
        }
        f.step(&mut source);
        let control = ViewBlendControl::capture(f.actor());
        active += usize::from(control.position_active || control.rotation_active);
        settled += usize::from(!control.position_active && !control.rotation_active);
    }
    assert!(
        active > 1000 && settled > 100,
        "both active and settled retained visits required: {active}, {settled}"
    );
}
