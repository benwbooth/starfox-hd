//! Complete original view selection/distance service and its authored profile
//! initializer. Retained tests also run the real continuity consumer; neither
//! side is reseeded from the other's camera, request, or muzzle state.

use super::{rom, Source, WRAM};
use sf2_game::path_program::SelectedAuxiliaryState;
use sf2_game::player_view_distance::{self, PlayerViewDistance};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::view_blend::{self, ViewBlendControl};
use sf2_game::view_transition::{FixedViewAngles, ViewTransitionMode};
use sf2_game::weapon_dispatch::WeaponState;
use sf2_game::{
    Behavior, Object, ObjectId, ObjectKind, ObjectStore, RandomState, ShapeId, Vector3,
};

const VIEW: u32 = 0x033F;
const OWNER: u16 = 0x03BD;
const PROXY: u32 = 0x043B;
const SLOT: u32 = 0x0200;

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    view: ObjectId,
    owner: ObjectId,
    proxy: ObjectId,
    unowned_linked_flags: u8,
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
        let owner = allocate();
        let proxy = allocate();
        let mut world = ScenePathWorld::new(RandomState::new([19, 53, 177, 211]));
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 0,
                        action_flags: 0x97,
                        stored_rotation: Default::default(),
                        stored_world_position: Default::default(),
                    }),
                    contact: Some(Default::default()),
                    charge: Some(Default::default()),
                    view_distance: Some(Default::default()),
                    ..Default::default()
                },
            )
            .unwrap();
        world.player_view_options_enabled = Some(true);
        world.scene.player_view_control = Some(0x83);
        world.published_linked_view = Some(0xCD);
        world.fixed_players[0] = Some(view);
        world.view_transition_mode = Some(ViewTransitionMode { flags: 0xAE00 });
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        Self {
            objects,
            world,
            view,
            owner,
            proxy,
            unowned_linked_flags: 0,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn set_flags(&mut self, flags: u8) {
        self.unowned_linked_flags = flags & 0x1B;
        let records = self.records();
        let charge = records.charge.as_mut().unwrap();
        charge.linked_mode = flags & 0x80 != 0;
        charge.linked_muzzle_disabled = flags & 0x40 != 0;
        let distance = records.view_distance.as_mut().unwrap();
        distance.capture_pending = flags & 0x20 != 0;
        distance.switch_requested = flags & 0x04 != 0;
    }
    fn values(&self) -> Vec<(u32, u16, bool)> {
        let records = self.world.player(&self.objects, self.owner).unwrap();
        let state = records.view_distance.unwrap();
        let charge = records.charge.unwrap();
        let view = self.objects.get(self.view).unwrap();
        let control = ViewBlendControl::capture(view);
        let angles = FixedViewAngles::capture(view);
        let flags = self.unowned_linked_flags
            | u8::from(charge.linked_mode) * 0x80
            | u8::from(charge.linked_muzzle_disabled) * 0x40
            | u8::from(state.capture_pending) * 0x20
            | u8::from(state.switch_requested) * 0x04;
        let mut values = vec![];
        for (address, value) in [
            (WRAM + SLOT + 0x6B63, flags),
            (WRAM + SLOT + 0x6AA0, records.auxiliary.unwrap().mode),
            (
                WRAM + SLOT + 0x6B77,
                records.auxiliary.unwrap().action_flags,
            ),
            (
                WRAM + SLOT + 0x6A72,
                0xED | u8::from(records.contact.unwrap().ignores_contacts) * 0x10,
            ),
            (
                WRAM + SLOT + 0x6B94,
                records.auxiliary.unwrap().mode.rotate_left(3),
            ),
            (WRAM + 0x1DE0, self.world.scene.player_view_control.unwrap()),
            (
                WRAM + 0x1DE1,
                0x5B | u8::from(self.world.player_view_options_enabled.unwrap()) * 0x80,
            ),
            (WRAM + 0xF542, self.world.published_linked_view.unwrap()),
            (
                WRAM + u32::from(OWNER) + 0x25,
                0xD7 | u8::from(
                    self.objects
                        .get(self.owner)
                        .unwrap()
                        .base
                        .flags
                        .view_side_filter,
                ) * 0x20,
            ),
            (
                WRAM + VIEW + 0x21,
                7 | u8::from(control.capture_position) * 8
                    | u8::from(control.capture_rotation) * 0x10
                    | u8::from(control.discard_capture) * 0x20
                    | u8::from(control.position_active) * 0x40
                    | u8::from(control.rotation_active) * 0x80,
            ),
            (
                WRAM + VIEW + 0x22,
                0xA4 | u8::from(control.fast_position_recovery),
            ),
            (WRAM + VIEW + 0x0A, view.base.target_speed),
            (
                WRAM + VIEW + 0x1CD5,
                view.extension.relative_rotation.pitch.units(),
            ),
            (
                WRAM + VIEW + 0x1CD6,
                view.extension.relative_rotation.yaw.units(),
            ),
            (
                WRAM + VIEW + 0x1CD7,
                view.extension.relative_rotation.roll.units(),
            ),
            (WRAM + SLOT + 0x6C09, charge.control),
            (WRAM + SLOT + 0x6B58, charge.speed_impulse_ticks),
            (WRAM + SLOT + 0x6B60, charge.rapid_control),
        ] {
            values.push((address, u16::from(value), true));
        }
        for (field, value) in [
            (0x6B67, state.distance),
            (0x6B69, state.boost_response),
            (0x6B6B, state.brake_response),
            (0x6B6D, state.linked_target),
            (0x6B6F, state.external_target),
            (0x6B71, state.reserved_profile[0]),
            (0x6B73, state.reserved_profile[1]),
            (0x6B75, state.pitch_height_offset),
            (0x6B56, charge.speed_impulse),
            (0x6C07, charge.progress as i16),
        ] {
            values.push((WRAM + SLOT + field, value as u16, false));
        }
        for (field, value) in [
            (12, view.base.position.x),
            (14, view.base.position.y),
            (16, view.base.position.z),
            (0x12, angles.pitch as i16),
            (0x14, angles.yaw as i16),
            (0x16, angles.roll as i16),
            (
                0x39,
                view.extension.path_state.platform_carry.saved_position.x,
            ),
            (
                0x3B,
                view.extension.path_state.platform_carry.saved_position.y,
            ),
            (
                0x3D,
                view.extension.path_state.platform_carry.saved_position.z,
            ),
            (0x1CC1, view.extension.path_state.motion_delta.x),
            (0x1CC3, view.extension.path_state.motion_delta.y),
            (0x1CC5, view.extension.path_state.motion_delta.z),
            (0x1CCF, view.extension.relative_position.x),
            (0x1CD1, view.extension.relative_position.y),
            (0x1CD3, view.extension.relative_position.z),
        ] {
            values.push((WRAM + VIEW + field, value as u16, false));
        }
        values.push((
            WRAM + 0x1B84,
            self.world.view_transition_mode.unwrap().flags,
            false,
        ));
        let proxy = self.objects.get(self.proxy).unwrap();
        for (field, value) in [
            (0x12, proxy.base.pitch),
            (0x14, proxy.base.yaw),
            (0x16, proxy.base.roll),
        ] {
            values.push((WRAM + PROXY + field, u16::from(value.units()), true));
        }
        values
    }
    fn seed(&self, source: &mut Source) {
        source
            .bus
            .write16(WRAM + u32::from(OWNER) + 0x2B, SLOT as u16);
        source.bus.write16(WRAM + 0x14D6, PROXY as u16);
        source.bus.write8(WRAM + u32::from(OWNER) + 0x21, 0xA5);
        source.bus.write8(WRAM + u32::from(OWNER) + 0x24, 0xAD);
        source.bus.write8(WRAM + u32::from(OWNER) + 0x2D, 91);
        source.bus.write8(WRAM + 0x1E13, 1);
        for (field, value, byte) in self.values() {
            if byte {
                source.bus.write8(field, value as u8)
            } else {
                source.bus.write16(field, value)
            }
        }
    }
    fn verify(&self, source: &Source, phase: &str) {
        for (field, value, byte) in self.values() {
            let actual = if byte {
                u16::from(source.bus.read8(field))
            } else {
                source.bus.read16(field)
            };
            assert_eq!(value, actual, "{phase} field={field:06X}");
        }
        assert_eq!(self.world.random.bytes(), [19, 53, 177, 211]);
    }
    fn step(&mut self, source: &mut Source, initialize: bool, blend: bool) {
        if initialize {
            source.run(0x079CCA, None, 0, OWNER, true);
            player_view_distance::initialize(&self.objects, &mut self.world, self.owner).unwrap();
        } else {
            source.run(0x079AEF, None, 0, OWNER, true);
            player_view_distance::advance(&mut self.objects, &mut self.world, self.owner).unwrap();
        }
        self.verify(source, if initialize { "initialize" } else { "advance" });
        if blend {
            source.run(0x0797FB, None, 0, OWNER, true);
            view_blend::advance(&mut self.objects, &self.world).unwrap();
            self.verify(source, "blend");
        }
    }
}

#[test]
fn view_distance_initializer_matches_original_all_modes_and_preserves_live_requests() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for mode in 0..=255_u16 {
        for flags in 0..16_u8 {
            let records = f.records();
            records.auxiliary.as_mut().unwrap().mode = mode as u8;
            *records.view_distance.as_mut().unwrap() = PlayerViewDistance {
                distance: -31917,
                boost_response: 29513,
                brake_response: -227,
                linked_target: 14213,
                external_target: -917,
                reserved_profile: [1713, -919],
                pitch_height_offset: 12227,
                ..Default::default()
            };
            f.set_flags(0x1B | (flags << 4 & 0xE0) | ((flags & 1) << 2));
            f.seed(&mut source);
            f.step(&mut source, true, false);
        }
    }
}

#[test]
fn view_distance_matches_original_every_shared_and_player_control_byte() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for options_enabled in [false, true] {
        for flags in 0..=255_u16 {
            for control in 0..=255_u16 {
                f.set_flags(flags as u8);
                let state = f.records().view_distance.as_mut().unwrap();
                state.distance = (flags.wrapping_mul(253) ^ control.wrapping_mul(19)) as i16;
                state.linked_target = -40;
                state.external_target = -240;
                f.records().auxiliary.as_mut().unwrap().mode = control as u8;
                f.records().contact.as_mut().unwrap().ignores_contacts = flags & 1 != 0;
                f.world.player_view_options_enabled = Some(options_enabled);
                f.world.scene.player_view_control = Some(control as u8);
                f.seed(&mut source);
                f.step(&mut source, false, false);
            }
        }
    }
}

#[test]
fn view_distance_matches_original_all_wrapped_current_and_target_words() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for variable_target in [false, true] {
        for word in 0..=u16::MAX {
            f.set_flags(0xDA);
            let state = f.records().view_distance.as_mut().unwrap();
            state.distance = if variable_target { -32760 } else { word as i16 };
            state.linked_target = if variable_target { word as i16 } else { 32760 };
            state.external_target = -240;
            f.world.scene.player_view_control = Some(0x81);
            f.seed(&mut source);
            f.step(&mut source, false, false);
        }
    }
}

#[test]
fn view_distance_retains_real_menu_transitions_muzzle_state_and_continuity_without_replay() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    f.records().auxiliary.as_mut().unwrap().mode = 0x11;
    f.seed(&mut source);
    f.step(&mut source, true, false);
    let mut switched = 0;
    let mut completed = 0;
    for visit in 0..8192_u16 {
        // Input only: menus, contact activity, camera pose deltas and mode
        // entry. Retained distance, requests, linked state and blend history
        // evolve independently in the original and native systems.
        if visit % 17 == 0 {
            let control = 0x2D | if visit & 1 == 0 { 0x80 } else { 0 };
            f.world.scene.player_view_control = Some(control);
            source.bus.write8(WRAM + 0x1DE0, control);
        }
        let enabled = visit % 71 >= 5;
        f.world.player_view_options_enabled = Some(enabled);
        source
            .bus
            .write8(WRAM + 0x1DE1, 0x5B | u8::from(enabled) * 0x80);
        let ignored = visit % 97 < 3;
        f.records().contact.as_mut().unwrap().ignores_contacts = ignored;
        source
            .bus
            .write8(WRAM + SLOT + 0x6A72, 0xED | u8::from(ignored) * 0x10);
        if visit % 251 == 0 {
            let mode = ((visit / 251) % 4 * 16 + 1) as u8;
            f.records().auxiliary.as_mut().unwrap().mode = mode;
            source.bus.write8(WRAM + SLOT + 0x6AA0, mode);
            source.bus.write8(WRAM + SLOT + 0x6B94, mode.rotate_left(3));
            f.step(&mut source, true, false);
        }
        let delta = Vector3 {
            x: (visit as i16).wrapping_mul(17),
            y: -13,
            z: 29,
        };
        let view = f.objects.get_mut(f.view).unwrap();
        view.base.position.x = view.base.position.x.wrapping_add(delta.x);
        view.base.position.y = view.base.position.y.wrapping_add(delta.y);
        view.base.position.z = view.base.position.z.wrapping_add(delta.z);
        for (field, change) in [(12, delta.x), (14, delta.y), (16, delta.z)] {
            let address = WRAM + VIEW + field;
            source.bus.write16(
                address,
                source.bus.read16(address).wrapping_add(change as u16),
            );
        }
        let angles = FixedViewAngles::capture(view);
        FixedViewAngles {
            pitch: angles.pitch.wrapping_add(713),
            yaw: angles.yaw.wrapping_sub(517),
            roll: angles.roll.wrapping_add(311),
        }
        .write_to(view);
        for (field, change) in [(0x12, 713_i16), (0x14, -517), (0x16, 311)] {
            let address = WRAM + VIEW + field;
            source.bus.write16(
                address,
                source.bus.read16(address).wrapping_add(change as u16),
            );
        }
        let before = f.records().charge.unwrap();
        f.step(&mut source, false, true);
        let after = f.records().charge.unwrap();
        switched += usize::from(before.linked_mode != after.linked_mode);
        completed += usize::from(before.linked_muzzle_disabled && !after.linked_muzzle_disabled);
    }
    assert!(switched > 300, "{switched}");
    assert!(completed > 300, "{completed}");
}
