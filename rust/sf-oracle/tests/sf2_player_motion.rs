//! Original flight translation, including both live proxy-vector calls and
//! the complete host/graphics collision response. No ROM code is replaced.

use sf2_game::collision_surface::{ActorSurfaceContact, SurfaceMode};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_motion::{self, MotionContext, PlayerMotion, PlayerSurfaceSupport};
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::scene_path_world::ScenePathWorld;
use sf2_game::surface_motion::{SurfaceMotionResult, SurfaceTilt};
use sf2_game::view_transition::ViewTransitionMode;
use sf2_game::weapon_dispatch::WeaponState;
use sf2_game::{
    Angle, Behavior, Object, ObjectId, ObjectKind, ObjectStore, RandomState, ShapeId, Vector3,
};
use sf_oracle::{call_near, Entry, SnesBus};

const WRAM: u32 = 0x7E0000;
const OWNER: u16 = 0x03BD;
const SLOT: u32 = 64;

fn address(id: Option<ObjectId>) -> u16 {
    id.map_or(0, |id| OWNER + id.index() as u16 * 0x3F)
}

fn source() -> SnesBus {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .unwrap();
    let mut source = SnesBus::new(rom.clone());
    for (index, &byte) in rom[0x10000..0x17E00].iter().enumerate() {
        source.write8(0x7F0000 + index as u32, byte);
    }
    source.enable_gsu();
    source
}

struct Native {
    objects: ObjectStore,
    world: ScenePathWorld,
    runtime: PathRuntime,
    owner: ObjectId,
    proxy: ObjectId,
    collider: ObjectId,
    tilt: SurfaceTilt,
}

impl Native {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let mut allocate = |kind| {
            objects
                .allocate(Object::new(kind, ShapeId::EMPTY, Behavior::Unassigned))
                .unwrap()
        };
        let owner = allocate(ObjectKind::Player);
        let proxy = allocate(ObjectKind::Effect);
        let collider = allocate(ObjectKind::Enemy);
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut runtime = PathRuntime::default();
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 0,
                score: Default::default(),
            },
        )
        .unwrap();
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        world.view_transition_mode = Some(ViewTransitionMode { flags: 0 });
        world.surface_mode = Some(SurfaceMode { flags: 1 });
        world.player_surface_support = Some(PlayerSurfaceSupport::default());
        world.player_carry_mode = Some(0);
        world.environment_plane_height = Some(0);
        world.scene.player_configuration = Some(0);
        world
            .player_mut(&objects, owner)
            .unwrap()
            .vertical
            .as_mut()
            .unwrap()
            .motion_axes = 0xE0;
        Self {
            objects,
            world,
            runtime,
            owner,
            proxy,
            collider,
            tilt: SurfaceTilt::default(),
        }
    }

    fn controls(&mut self, speed: u8, thrust: i8, fine_pitch: u16, yaw: u8, axes: u8) {
        let actor = self.objects.get_mut(self.owner).unwrap();
        actor.base.speed = speed;
        actor.base.pitch = Angle::from_units((fine_pitch as u8).wrapping_add(71));
        actor.base.yaw = Angle::from_units(yaw);
        actor.base.roll = Angle::from_units(yaw.wrapping_mul(17));
        let record = self.world.player_mut(&self.objects, self.owner).unwrap();
        record.speed.as_mut().unwrap().thrust = thrust;
        record.vertical.as_mut().unwrap().motion_axes = axes;
        player_storage::get_mut(&self.objects, &mut self.runtime.resources, self.owner)
            .unwrap()
            .fine_pitch = fine_pitch;
    }

    fn seed_controls(&self, source: &mut SnesBus) {
        let actor = self.objects.get(self.owner).unwrap();
        let record = self.world.player(&self.objects, self.owner).unwrap();
        let storage =
            player_storage::get(&self.objects, &self.runtime.resources, self.owner).unwrap();
        source.write8(u32::from(OWNER) + 0x18, actor.base.speed);
        for (offset, value) in [
            (0x12, actor.base.pitch.units()),
            (0x14, actor.base.yaw.units()),
            (0x16, actor.base.roll.units()),
        ] {
            source.write8(u32::from(OWNER) + offset, value);
        }
        source.write8(WRAM + SLOT + 0x6B62, record.speed.unwrap().thrust as u8);
        source.write8(WRAM + SLOT + 0x6B84, record.vertical.unwrap().motion_axes);
        source.write16(WRAM + SLOT + 0x6AB9, storage.fine_pitch);
        source.write8(
            WRAM + SLOT + 0x6B7D,
            if record.contact.unwrap().hit.hold_secondary_protection {
                0xA5
            } else {
                0x25
            },
        );
        source.write8(
            u32::from(OWNER) + 0x21,
            if actor.extension.path_state.motion.carry_selected_player {
                0x20
            } else {
                0
            },
        );
        source.write8(0x1E13, self.world.player_carry_mode.unwrap());
        source.write16(0x1E0F, self.world.environment_plane_height.unwrap() as u16);
        source.write16(
            0x1B84,
            if self.world.scripted_view_active().unwrap() {
                2
            } else {
                0
            },
        );
        source.write8(0x1DE2, self.world.scene.player_configuration.unwrap());
        source.write8(0x1B4D, self.world.surface_mode.unwrap().flags);
        source.write8(0xC4, self.world.strategy_clock as u8);
    }

    fn seed(&self, source: &mut SnesBus) {
        source.write16(0x12A8, address(self.objects.active_ids().first().copied()));
        source.write16(0x14D6, address(Some(self.proxy)));
        source.write16(u32::from(OWNER) + 0x2B, SLOT as u16);
        for (id, object) in self.objects.active_objects() {
            let base = WRAM + u32::from(address(Some(id)));
            source.write16(base, address(object.base.next));
            source.write16(base + 2, address(object.base.previous));
            source.write16(
                base + 4,
                0xBC9C + object.base.shape.catalog_index() as u16 * 28,
            );
            source.write8(base + 0x14, object.base.yaw.units());
            source.write8(
                base + 0x24,
                u8::from(object.base.flags.standing_on_surface) * 2
                    | u8::from(object.base.flags.exclude_from_shape_footprint_search) * 4,
            );
            source.write8(
                base + 0x31,
                u8::from(object.base.contacts.first_strategy_visit) * 4,
            );
            let delta = object.extension.path_state.motion_delta;
            let saved = object.extension.path_state.platform_carry.saved_position;
            for (offset, value) in [
                (12, object.base.position.x),
                (14, object.base.position.y),
                (16, object.base.position.z),
                (0x32, object.base.velocity.x),
                (0x34, object.base.velocity.y),
                (0x36, object.base.velocity.z),
                (0x39, saved.x),
                (0x3B, saved.y),
                (0x3D, saved.z),
                (0x1CC1, delta.x),
                (0x1CC3, delta.y),
                (0x1CC5, delta.z),
            ] {
                source.write16(base + offset, value as u16);
            }
            source.write16(
                base + 0x1CE8,
                address(object.extension.surface_contact.supporting_object),
            );
            source.write8(base + 0x1CEA, object.extension.surface_contact.group);
            source.write8(base + 0x1CEB, object.extension.surface_contact.flags);
        }
        let record = self.world.player(&self.objects, self.owner).unwrap();
        let displacement = record.flight_displacement.unwrap();
        for (offset, value) in [
            (0x6B0B, displacement.x),
            (0x6B0D, displacement.y),
            (0x6B0F, displacement.z),
            (0x6B11, record.motion.unwrap().surface_velocity[0]),
            (0x6B13, record.motion.unwrap().surface_velocity[1]),
        ] {
            source.write16(WRAM + SLOT + offset, value as u16);
        }
        source.write8(WRAM + SLOT + 0x6BE6, record.motion.unwrap().contact_flags);
        source.write8(WRAM + SLOT + 0x6B61, 0xFF); // Adjacent low byte must not decide thrust sign.
        source.write8(WRAM + SLOT + 0x6B63, 0xA5);
        let support = self.world.player_surface_support.unwrap();
        source.write16(0x1D6F, address(support.object));
        source.write8(0x1D71, support.group);
        source.write16(0x700020, u16::from(self.tilt.pitch.units()) | 0xAB00);
        source.write16(0x700024, u16::from(self.tilt.roll.units()) | 0xCD00);
        self.seed_controls(source);
    }

    fn step(&mut self, source: &mut SnesBus) -> Option<SurfaceMotionResult> {
        let result = call_near(
            source,
            0x06EE0A,
            &Entry {
                x: OWNER,
                p: 0x20,
                dbr: 0x7E,
                ..Default::default()
            },
        );
        assert!(result.returned, "whole motion routine must return");
        let response = player_motion::advance(
            &mut self.objects,
            &mut self.world,
            &self.runtime.resources,
            self.owner,
            MotionContext {
                inherited_surface_tilt: Some(self.tilt),
            },
        )
        .unwrap();
        if let Some(response) = response {
            self.tilt = response.inherited_tilt;
        }
        for id in [self.owner, self.proxy] {
            let object = self.objects.get(id).unwrap();
            let base = WRAM + u32::from(address(Some(id)));
            let saved = object.extension.path_state.platform_carry.saved_position;
            let delta = object.extension.path_state.motion_delta;
            for (offset, value) in [
                (12, object.base.position.x),
                (14, object.base.position.y),
                (16, object.base.position.z),
                (0x32, object.base.velocity.x),
                (0x34, object.base.velocity.y),
                (0x36, object.base.velocity.z),
                (0x39, saved.x),
                (0x3B, saved.y),
                (0x3D, saved.z),
                (0x1CC1, delta.x),
                (0x1CC3, delta.y),
                (0x1CC5, delta.z),
            ] {
                assert_eq!(
                    value as u16,
                    source.read16(base + offset),
                    "object {id:?}, field {offset:X}"
                );
            }
            for (offset, value) in [
                (0x12, object.base.pitch.units()),
                (0x14, object.base.yaw.units()),
                (0x16, object.base.roll.units()),
                (0x18, object.base.speed),
            ] {
                assert_eq!(
                    value,
                    source.read8(base + offset),
                    "object {id:?}, field {offset:X}"
                );
            }
            assert_eq!(
                object.base.flags.standing_on_surface,
                source.read8(base + 0x24) & 2 != 0
            );
            assert_eq!(
                address(object.extension.surface_contact.supporting_object),
                source.read16(base + 0x1CE8)
            );
            assert_eq!(
                object.extension.surface_contact.group,
                source.read8(base + 0x1CEA)
            );
            assert_eq!(
                object.extension.surface_contact.flags,
                source.read8(base + 0x1CEB)
            );
        }
        let record = self.world.player(&self.objects, self.owner).unwrap();
        let displacement = record.flight_displacement.unwrap();
        for (offset, value) in [
            (0x6B0B, displacement.x),
            (0x6B0D, displacement.y),
            (0x6B0F, displacement.z),
            (0x6B11, record.motion.unwrap().surface_velocity[0]),
            (0x6B13, record.motion.unwrap().surface_velocity[1]),
        ] {
            assert_eq!(
                value as u16,
                source.read16(WRAM + SLOT + offset),
                "player field {offset:X}"
            );
        }
        assert_eq!(
            record.motion.unwrap().contact_flags,
            source.read8(WRAM + SLOT + 0x6BE6)
        );
        assert_eq!(
            record.speed.unwrap().thrust as u8,
            source.read8(WRAM + SLOT + 0x6B62)
        );
        assert_eq!(
            record.vertical.unwrap().motion_axes,
            source.read8(WRAM + SLOT + 0x6B84)
        );
        assert_eq!(source.read8(WRAM + SLOT + 0x6B63), 0xA5);
        let support = self.world.player_surface_support.unwrap();
        assert_eq!(address(support.object), source.read16(0x1D6F));
        assert_eq!(support.group, source.read8(0x1D71));
        assert_eq!(self.tilt.pitch.units(), source.read8(0x700020));
        assert_eq!(self.tilt.roll.units(), source.read8(0x700024));
        response
    }
}

#[test]
fn flight_translation_matches_original_all_base_speed_and_thrust_pairs() {
    let mut source = source();
    let mut native = Native::new();
    for speed in 0..=u8::MAX {
        for thrust in i8::MIN..=i8::MAX {
            native.controls(
                speed,
                thrust,
                u16::from(speed).wrapping_mul(301),
                (thrust as u8).wrapping_mul(13),
                0xFF,
            );
            native.seed(&mut source);
            native.step(&mut source);
        }
    }
}

#[test]
fn flight_translation_matches_original_all_angles_and_axis_permissions() {
    let mut source = source();
    let mut native = Native::new();
    for yaw in 0..=u8::MAX {
        for pitch in 0..=u8::MAX {
            native.controls(
                pitch.wrapping_add(yaw),
                yaw as i8,
                u16::from_le_bytes([yaw, pitch]),
                yaw,
                pitch,
            );
            native.seed(&mut source);
            native.step(&mut source);
        }
    }
}

#[test]
fn flight_protection_matches_original_all_heights_carry_gates_and_scripted_view() {
    let mut source = source();
    let mut native = Native::new();
    native
        .world
        .player_mut(&native.objects, native.owner)
        .unwrap()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .hold_secondary_protection = true;
    for value in 0..=u16::MAX {
        for scripted in [false, true] {
            native.world.view_transition_mode = Some(ViewTransitionMode {
                flags: if scripted { 2 } else { 0 },
            });
            native.world.player_carry_mode = Some((value as u8) & 3);
            native.world.environment_plane_height = Some(value.rotate_left(9) as i16);
            let actor = native.objects.get_mut(native.owner).unwrap();
            actor.base.position.y = value as i16;
            actor.extension.path_state.motion.carry_selected_player = value & 4 != 0;
            native.controls(127, -128, value, (value >> 8) as u8, value as u8);
            native.seed(&mut source);
            native.step(&mut source);
        }
    }
}

#[test]
fn constrained_flight_matches_original_all_shapes_and_full_retained_words() {
    let mut source = source();
    let mut native = Native::new();
    native.world.scene.player_configuration = Some(9);
    native
        .objects
        .get_mut(native.collider)
        .unwrap()
        .base
        .contacts
        .first_strategy_visit = false;
    let mut contacts = 0;
    let mut obstructions = 0;
    for value in 0..=u16::MAX {
        let collider = native.objects.get_mut(native.collider).unwrap();
        collider.base.shape =
            ShapeId::from_catalog_index(value % sf2_data::shape_data::SHAPE_DATA_COUNT as u16);
        collider.base.yaw = Angle::from_units((value >> 3) as u8);
        collider.base.velocity.y = (value % 15) as i16 - 7;
        let actor = native.objects.get_mut(native.owner).unwrap();
        actor.base.position = Vector3 {
            x: (value % 101) as i16 - 50,
            y: value as i16,
            z: (value % 151) as i16 - 75,
        };
        actor.extension.path_state.motion_delta = Vector3 {
            x: 197,
            y: value.rotate_left(7) as i16,
            z: -197,
        };
        actor.extension.path_state.platform_carry.saved_position = Vector3 { x: -2, y: 3, z: 5 };
        actor.extension.surface_contact = ActorSurfaceContact {
            supporting_object: Some(native.proxy),
            group: 197,
            flags: 0xA5,
        };
        native.world.player_surface_support = Some(PlayerSurfaceSupport {
            object: (value & 1 != 0).then_some(native.collider),
            group: (value / 3) as u8,
        });
        native
            .world
            .player_mut(&native.objects, native.owner)
            .unwrap()
            .motion = Some(PlayerMotion {
            surface_velocity: [value.rotate_left(3) as i16, !value as i16],
            contact_flags: value as u8,
        });
        native.world.strategy_clock = value;
        native.tilt = SurfaceTilt {
            pitch: Angle::from_units(value as u8),
            roll: Angle::from_units((value >> 8) as u8),
        };
        native.controls(
            value as u8,
            (value >> 8) as i8,
            value.wrapping_mul(131),
            value as u8,
            0xFF,
        );
        native.seed(&mut source);
        let response = native.step(&mut source).unwrap();
        obstructions += usize::from(response.obstructed);
        contacts += usize::from(
            native
                .world
                .player_surface_support
                .unwrap()
                .object
                .is_some(),
        );
    }
    assert!(
        contacts > 0 && obstructions > 0,
        "{contacts} collider contacts, {obstructions} obstructions"
    );
}

#[test]
fn constrained_flight_matches_original_continuous_live_geometry_and_mode_changes() {
    let mut source = source();
    let mut contacts = 0;
    for scenario in 0..32_u16 {
        let mut native = Native::new();
        native
            .objects
            .get_mut(native.collider)
            .unwrap()
            .base
            .contacts
            .first_strategy_visit = false;
        native.objects.get_mut(native.collider).unwrap().base.shape =
            ShapeId::from_catalog_index(142 + scenario);
        native
            .objects
            .get_mut(native.owner)
            .unwrap()
            .base
            .position
            .y = -70;
        native.tilt = SurfaceTilt {
            pitch: Angle::from_units(17),
            roll: Angle::from_units(231),
        };
        native.seed(&mut source);
        for visit in 0..256_u16 {
            native.world.scene.player_configuration = Some(if visit & 8 != 0 { 9 } else { 0 });
            native.world.strategy_clock = visit;
            native.controls(
                (visit % 30) as u8,
                (visit % 41) as i8 - 20,
                visit.wrapping_mul(131),
                (visit + scenario) as u8,
                0xFF,
            );
            native.seed_controls(&mut source);
            native.step(&mut source);
            contacts += usize::from(
                native
                    .world
                    .player_surface_support
                    .unwrap()
                    .object
                    .is_some(),
            );
        }
    }
    assert!(
        contacts > 0,
        "continuous visits must reach eligible collider geometry"
    );
}

#[test]
fn near_call_harness_sets_data_bank_at_both_accumulator_widths() {
    let mut source = source();
    // A synthetic three-instruction harness test, separate from all original
    // gameplay comparisons. The routine reads the requested data bank.
    for (offset, byte) in [0xAD, 0x61, 0x6A, 0x60].into_iter().enumerate() {
        source.write8(0x7F7D80 + offset as u32, byte);
    }
    source.write16(0x7E6A61, 0xA53C);
    source.write16(0x7F6A61, 0x2EC7);
    for (dbr, expected) in [(0x7E, 0xA53C), (0x7F, 0x2EC7)] {
        for p in [0, 0x20] {
            let actual = call_near(
                &mut source,
                0x7F7D80,
                &Entry {
                    dbr,
                    p,
                    ..Default::default()
                },
            );
            assert!(actual.returned);
            assert_eq!(actual.c, if p == 0 { expected } else { expected & 255 });
        }
    }
}
