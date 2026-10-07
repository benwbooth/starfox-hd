//! The entire original shared flight routine, including nested allocation,
//! collision, sound, corridor and grid calls. No expected result is patched
//! into the source, and continuous visits never copy source state to Rust.

use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::collision_surface::SurfaceMode;
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_protection::DeflectionProtection;
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_flight::{self, FlightContext, FlightResult};
use sf2_game::player_motion::{self, PlayerSurfaceSupport};
use sf2_game::player_roll::ShoulderControl;
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::surface_motion::SurfaceTilt;
use sf2_game::view_transition::ViewTransitionMode;
use sf2_game::weapon_dispatch::WeaponState;
use sf2_game::world_occupancy::{MarkerCoverage, OccupancyChange, WorldOccupancy, WorldRectangle};
use sf2_game::{Angle, Buttons, InputState, ObjectId, RandomState, ShapeId, SoundEvent, Vector3};
use sf_oracle::{call_near, Entry};

struct Fixture {
    native: Native,
    runtime: PathRuntime,
    proxy: ObjectId,
    collider: ObjectId,
    view: ObjectId,
    context: FlightContext,
}

impl Fixture {
    fn new(source: &mut Source) -> Self {
        let mut native = Native::new(source, 4, 0, 0, 0);
        let mut runtime = PathRuntime::default();
        player_storage::initialize(
            &mut native.objects,
            &mut native.world,
            &mut runtime,
            native.owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 96,
                score: Default::default(),
            },
        )
        .unwrap();
        let others: Vec<_> = native
            .objects
            .active_ids()
            .iter()
            .copied()
            .filter(|&id| id != native.owner)
            .collect();
        let (proxy, collider, view) = (others[0], others[1], others[2]);
        let world = &mut native.world;
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        world.fixed_players[0] = Some(view);
        world.surface_mode = Some(SurfaceMode { flags: 0 });
        world.scene.player_configuration = Some(0);
        world.view_transition_mode = Some(ViewTransitionMode { flags: 0 });
        world.player_carry_mode = Some(0);
        world.environment_plane_height = Some(0);
        world.player_surface_support = Some(PlayerSurfaceSupport::default());
        world.player_surface_height = Some(0);
        world.player_pitch_target = Some(0);
        world.player_yaw_increment = Some(0);
        world.processed_player_input = Some(Default::default());
        world.contacts_enabled = Some(true);
        world.reflect_all_contacts = Some(false);
        world.action_gate = Some(Default::default());
        world.occupancy = Some(WorldOccupancy::default());
        world.random = RandomState::new([17, 63, 149, 211]);
        let records = world.player_mut(&native.objects, native.owner).unwrap();
        records.vertical.as_mut().unwrap().motion_axes = 0xE0;
        records.auxiliary.as_mut().unwrap().mode = 0x11;
        let actor = native.objects.get_mut(native.owner).unwrap();
        actor.base.position = Vector3 {
            x: 600,
            y: -100,
            z: 600,
        };
        let actor = native.objects.get_mut(collider).unwrap();
        actor.base.shape = ShapeId::from_catalog_index(7);
        actor.base.contacts.first_strategy_visit = false;
        actor.base.position = Vector3 {
            x: 600,
            y: 0,
            z: 600,
        };
        native
            .objects
            .get_mut(proxy)
            .unwrap()
            .base
            .flags
            .exclude_from_shape_footprint_search = true;
        native
            .objects
            .get_mut(view)
            .unwrap()
            .base
            .flags
            .exclude_from_shape_footprint_search = true;
        let mut context = FlightContext::default();
        context.surface.wing_child_number = Some(77);
        context.surface.alternate_thrust_target = Some(-59);
        context.motion.inherited_surface_tilt = Some(SurfaceTilt {
            pitch: Angle::from_units(37),
            roll: Angle::from_units(91),
        });
        context.occupancy.camera_override_active = Some(false);
        context.occupancy.diagonal_tie_bias = Some(0xA5);
        Self {
            native,
            runtime,
            proxy,
            collider,
            view,
            context,
        }
    }

    fn records(&mut self) -> &mut PlayerPathRecords {
        self.native
            .world
            .player_mut(&self.native.objects, self.native.owner)
            .unwrap()
    }

    // Test-only address map. Both initial seeding and subsequent comparisons
    // use the canonical native owners; the shipping game has no such map.
    fn values(&self) -> Vec<(u32, u16, bool)> {
        let native = &self.native;
        let world = &native.world;
        let records = world.player(&native.objects, native.owner).unwrap();
        let storage =
            player_storage::get(&native.objects, &self.runtime.resources, native.owner).unwrap();
        let pose = records.pose.unwrap();
        let hit = records.contact.unwrap().hit;
        let motion = records.motion.unwrap();
        let boundary = records.boundary.unwrap();
        let grid = records.occupancy.unwrap();
        let throttle = records.throttle.unwrap();
        let roll = records.roll.unwrap();
        let ambient = records.ambient.unwrap();
        let mut values = Vec::new();
        for (offset, value) in [
            (
                0x6A72,
                u8::from(records.contact.unwrap().ignores_contacts) * 0x10,
            ),
            (0x6A82, records.surface.unwrap().material),
            (0x6AA0, records.auxiliary.unwrap().mode),
            (0x6AAD, motion.lateral_impulse as u8),
            (0x6AAE, records.steering.unwrap().locked_heading.units()),
            (0x6ABD, storage.bank.units()),
            (0x6ACF, pose.pitch_lean as u8),
            (0x6AD4, pose.yaw_trim as u8),
            (0x6AD5, pose.ambient_bank as u8),
            (0x6AD6, ambient.bank_phase),
            (0x6AD7, pose.steering_bank as u8),
            (0x6ADA, pose.heading_return_bank as u8),
            (0x6ADB, ambient.offset_phase),
            (0x6ADC, roll.tap_window),
            (0x6ADD, roll.impulse as u8),
            (0x6ADE, pose.yaw_offset as u8),
            (0x6B29, grid.current_cell[0] as u8),
            (0x6B2A, grid.current_cell[1] as u8),
            (0x6B2C, grid.previous_cell[0] as u8),
            (0x6B2D, grid.previous_cell[1] as u8),
            (
                0x6B32,
                records.auxiliary.unwrap().stored_rotation.pitch.units(),
            ),
            (0x6B62, records.speed.unwrap().thrust as u8),
            (
                0x6B63,
                u8::from(records.charge.unwrap().linked_mode) * 0x80
                    | u8::from(records.charge.unwrap().linked_muzzle_disabled) * 0x40,
            ),
            (0x6B77, records.auxiliary.unwrap().action_flags),
            (0x6B78, throttle.brake_preference),
            (0x6B79, throttle.effect_flags),
            (0x6B7A, throttle.effect_level),
            (0x6B7D, u8::from(hit.hold_secondary_protection) * 0x80),
            (0x6B7E, roll.shoulders.bits()),
            (0x6B7F, throttle.entry_marker),
            (0x6B84, records.vertical.unwrap().motion_axes),
            (0x6BE3, hit.recovery),
            (0x6BE6, motion.contact_flags),
            (0x6BE7, hit.deflection_sound_cooldown),
            (0x6BEB, u8::from(records.occupancy_exempt.unwrap()) * 0x80),
            (0x6BFF, records.visit.unwrap().pilot_code),
            (0x6C00, hit.reserve_shield),
            (0x6C02, records.protection.unwrap().control()),
            (0x6C09, records.charge.unwrap().control),
            (0x6C11, hit.feedback_duration),
            (0x6C12, hit.feedback_flags),
        ] {
            values.push((WRAM + SLOT + offset, u16::from(value), true));
        }
        for (offset, value) in [
            (0x6A7D, records.surface.unwrap().plane_height),
            (0x6AAF, boundary.center.x),
            (0x6AB1, boundary.center.y),
            (0x6AB3, boundary.center.z),
            (0x6AB5, boundary.half_width),
            (0x6AB7, boundary.half_height),
            (0x6AB9, storage.fine_pitch as i16),
            (0x6ABB, storage.fine_yaw as i16),
            (0x6AC7, motion.previous_position.x),
            (0x6AC9, motion.previous_position.y),
            (0x6ACB, motion.previous_position.z),
            (0x6ACD, records.yaw_motion.unwrap() as i16),
            (0x6AD0, pose.turning_lean as i16),
            (0x6AD8, pose.shoulder_bank),
            (0x6AE2, ambient.retained_offset),
            (0x6AF9, grid.displacement[0]),
            (0x6AFB, grid.displacement[1]),
            (0x6B0B, records.flight_displacement.unwrap().x),
            (0x6B0D, records.flight_displacement.unwrap().y),
            (0x6B0F, records.flight_displacement.unwrap().z),
            (0x6B11, motion.surface_velocity[0]),
            (0x6B13, motion.surface_velocity[1]),
            (0x6B3B, hit.camera_pitch_recoil),
            (0x6BED, boundary.return_position.x),
            (0x6BEF, boundary.return_position.y),
            (0x6BF1, boundary.return_position.z),
            (
                0x6BF5,
                records.vertical.unwrap().profile.upper_height_offset,
            ),
        ] {
            values.push((WRAM + SLOT + offset, value as u16, false));
        }
        for (offset, value) in [
            (0x14D6, address(Some(self.proxy))),
            (0x12C3, address(world.primary_player)),
            (0x1B84, world.view_transition_mode.unwrap().flags),
            (0x1E0F, world.environment_plane_height.unwrap() as u16),
            (0x1E36, world.player_pitch_target.unwrap()),
            (0x1E38, world.player_yaw_increment.unwrap()),
            (0x1DAE, world.player_surface_height.unwrap() as u16),
            (
                0x1D6F,
                address(world.player_surface_support.unwrap().object),
            ),
            (0x1936, world.processed_player_input.unwrap().pressed.bits()),
            (0x1938, world.processed_player_input.unwrap().held.bits()),
        ] {
            values.push((WRAM + offset, value, false));
        }
        for (offset, value) in [
            (0x1D71, world.player_surface_support.unwrap().group),
            (0x1DE2, world.scene.player_configuration.unwrap()),
            (0x1B4D, world.surface_mode.unwrap().flags),
            (0x1E13, world.player_carry_mode.unwrap()),
            (0xD7F4, u8::from(world.contacts_enabled.unwrap())),
            (0x1D72, world.action_gate.unwrap().code),
            (0x1AA6, u8::from(world.reflect_all_contacts.unwrap()) * 2),
            (0xC4, world.strategy_clock as u8),
        ] {
            values.push((WRAM + offset, u16::from(value), true));
        }
        for (index, value) in world.random.bytes().into_iter().enumerate() {
            values.push((WRAM + 0xE0 + index as u32, u16::from(value), true));
        }
        for (id, object) in native.objects.active_objects() {
            let base = WRAM + u32::from(address(Some(id)));
            let saved = object.extension.path_state.platform_carry.saved_position;
            let delta = object.extension.path_state.motion_delta;
            for (offset, value) in [
                (
                    4,
                    (0xBC9C_u16 + object.base.shape.catalog_index() as u16 * 28) as i16,
                ),
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
                (0x1CCF, object.extension.relative_position.x),
                (0x1CD1, object.extension.relative_position.y),
                (0x1CD3, object.extension.relative_position.z),
                (0x1CE2, object.extension.path_state.motion_phase as i16),
                (
                    0x1CE8,
                    address(object.extension.surface_contact.supporting_object) as i16,
                ),
            ] {
                values.push((base + offset, value as u16, false));
            }
            for (offset, value) in [
                (0x12, object.base.pitch.units()),
                (0x14, object.base.yaw.units()),
                (0x16, object.base.roll.units()),
                (0x18, object.base.speed),
                (0x2D, object.base.hit_points),
                (0x2E, object.base.attack_power),
                (0x1CEA, object.extension.surface_contact.group),
                (0x1CEB, object.extension.surface_contact.flags),
                (0x1CEF, object.extension.clipping_plane.selector_byte()),
            ] {
                values.push((base + offset, u16::from(value), true));
            }
        }
        values
    }

    fn seed(&self, source: &mut Source) {
        for (field, value, byte) in self.values() {
            if byte {
                source.bus.write8(field, value as u8);
            } else {
                source.bus.write16(field, value);
            }
        }
        for (id, object) in self.native.objects.active_objects() {
            let base = WRAM + u32::from(address(Some(id)));
            source.bus.write8(
                base + 0x20,
                u8::from(object.base.flags.reclaim_on_pool_pressure) * 0x10
                    | u8::from(object.base.contacts.hit_marked) * 2,
            );
            source.bus.write8(
                base + 0x21,
                u8::from(object.extension.path_state.motion.carry_selected_player) * 0x20,
            );
            source.bus.write8(
                base + 0x24,
                u8::from(object.base.flags.exclude_from_shape_footprint_search) * 4
                    | u8::from(object.base.flags.standing_on_surface) * 2,
            );
            source.bus.write8(
                base + 0x25,
                u8::from(object.base.contacts.skip_contacts) * 0x10,
            );
            source.bus.write8(
                base + 0x31,
                u8::from(object.base.contacts.first_strategy_visit) * 4
                    | u8::from(object.base.contacts.credits_hit_side) * 8,
            );
            source.bus.write8(
                base + 0x1CCB,
                object
                    .extension
                    .path_state
                    .animation
                    .shape
                    .fixed_frame()
                    .map_or(0, |frame| frame | 0x80),
            );
        }
        let view = self.native.objects.get(self.view).unwrap().base.position;
        for (offset, value) in [(12, view.x), (14, view.y), (16, view.z)] {
            source.bus.write16(0x033F + offset, value as u16);
        }
        self.seed_context(source);
    }

    fn seed_context(&self, source: &mut Source) {
        source
            .bus
            .write8(0, self.context.surface.wing_child_number.unwrap());
        source.bus.write8(
            0x1DB9,
            self.context.surface.alternate_thrust_target.unwrap() as u8,
        );
        source
            .bus
            .write8(0x3F, self.context.occupancy.diagonal_tie_bias.unwrap());
        source.bus.write16(
            WRAM + SLOT + 0x6A9D,
            if self.context.occupancy.camera_override_active.unwrap() {
                0x9F68
            } else {
                0
            },
        );
        let tilt = self.context.motion.inherited_surface_tilt.unwrap();
        source
            .bus
            .write16(0x700020, 0xAB00 | u16::from(tilt.pitch.units()));
        source
            .bus
            .write16(0x700024, 0xCD00 | u16::from(tilt.roll.units()));
    }

    fn plane(&mut self, source: &mut Source) {
        let mut grid = WorldOccupancy::default();
        for z in 0..128_u16 {
            for group in 0..16_u16 {
                let mut byte = 0;
                for bit in 0..8 {
                    let x = group * 8 + bit;
                    if (x + z) % 3 == 2 {
                        byte |= 1 << bit;
                        grid.apply(
                            &MarkerCoverage::from_rectangle(WorldRectangle {
                                x: (x * 512) as i16,
                                z: (z * 512) as i16,
                                width: 1,
                                depth: 1,
                            })
                            .unwrap(),
                            OccupancyChange::Mark,
                        );
                    }
                }
                source
                    .bus
                    .write8(WRAM + 0xCF36 + u32::from(z * 16 + group), byte);
            }
        }
        self.native.world.occupancy = Some(grid);
    }

    fn step(&mut self, source: &mut Source) -> FlightResult {
        source.bus.write16(0x1D16, 0);
        let result = call_near(
            &mut source.bus,
            0x06E258,
            &Entry {
                x: OWNER,
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            },
        );
        assert!(result.returned, "original flight failed to return");
        let result = player_flight::advance(
            &mut self.native.objects,
            &mut self.native.world,
            &mut self.runtime.resources,
            self.native.owner,
            self.context,
        )
        .unwrap();
        for (field, value, byte) in self.values() {
            let original = if byte {
                u16::from(source.bus.read8(field))
            } else {
                source.bus.read16(field)
            };
            assert_eq!(value, original, "flight field {field:06X}");
        }
        for (id, actor) in self.native.objects.active_objects() {
            let base = WRAM + u32::from(address(Some(id)));
            for (offset, mask, flag) in [
                (0x20, 2, actor.base.contacts.hit_marked),
                (0x24, 2, actor.base.flags.standing_on_surface),
                (0x25, 0x10, actor.base.contacts.skip_contacts),
                (0x31, 4, actor.base.contacts.first_strategy_visit),
                (0x31, 8, actor.base.contacts.credits_hit_side),
            ] {
                assert_eq!(
                    source.bus.read8(base + offset) & mask != 0,
                    flag,
                    "flight actor {base:06X} flags {offset:X}:{mask:X}"
                );
            }
        }
        self.native.compare_pool(source);
        let events: Vec<_> = self
            .native
            .world
            .audio
            .take_events()
            .into_iter()
            .flatten()
            .map(|event| {
                let SoundEvent::Authored(cue) = event else {
                    panic!("unexpected sound")
                };
                u16::from(cue.id)
                    | u16::from(cue.parameter()) << 8
                    | if cue.target == PlayerTarget::Secondary {
                        0x8000
                    } else {
                        0
                    }
            })
            .collect();
        assert_eq!(usize::from(source.bus.read16(0x1D16)), events.len() * 2);
        for (index, event) in events.into_iter().enumerate() {
            assert_eq!(source.bus.read16(0x1CF6 + index as u32 * 2), event);
        }
        result
    }
}

#[test]
fn complete_flight_matches_original_modes_controls_and_real_surface_handoffs() {
    let mut source = Source::new(&rom(), 0);
    source.bus.enable_gsu();
    let mut effects = 0;
    let mut surface_events = 0;
    for seed in 0..2048_u16 {
        let mut f = Fixture::new(&mut source);
        let low = seed as u8;
        let world = &mut f.native.world;
        world.strategy_clock = seed;
        world.scene.player_configuration = Some(if seed & 1 != 0 { 9 } else { 0 });
        world.view_transition_mode = Some(ViewTransitionMode {
            flags: if seed % 11 == 0 { 2 } else { 0 },
        });
        world.surface_mode = Some(SurfaceMode {
            flags: (seed >> 2) as u8 & 7,
        });
        world.player_carry_mode = Some(low & 1);
        world.primary_player = (seed % 7 != 0).then_some(f.native.owner);
        world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(seed.rotate_left(5)),
            pressed: Buttons::from_bits(seed.rotate_left(9)),
        });
        world.player_pitch_target = Some(seed.wrapping_mul(313));
        world.player_yaw_increment = Some(seed.wrapping_mul(117));
        world.reflect_all_contacts = Some(seed & 2 != 0);
        let records = f.records();
        records.auxiliary.as_mut().unwrap().mode =
            [0x11, 0x10, 0x30, 0, 0xFF][usize::from(seed % 5)];
        records.auxiliary.as_mut().unwrap().action_flags = low;
        records.visit.as_mut().unwrap().pilot_code = (seed % 6) as u8;
        records.roll.as_mut().unwrap().impulse = low as i8;
        records.roll.as_mut().unwrap().shoulders = ShoulderControl::from_bits(low.rotate_left(3));
        records.roll.as_mut().unwrap().tap_window = low.rotate_left(5);
        records.pose.as_mut().unwrap().shoulder_bank = seed.wrapping_mul(311) as i16;
        records.pose.as_mut().unwrap().turning_lean = seed.wrapping_mul(127);
        records.pose.as_mut().unwrap().pitch_lean = low.rotate_left(3) as i8;
        records.motion.as_mut().unwrap().surface_velocity = [seed as i16, -(seed as i16)];
        records.motion.as_mut().unwrap().lateral_impulse = low.rotate_left(2) as i8;
        records.speed.as_mut().unwrap().thrust = low.rotate_left(4) as i8;
        records.protection = Some(DeflectionProtection::from_control(low));
        records
            .contact
            .as_mut()
            .unwrap()
            .hit
            .hold_secondary_protection = seed % 9 == 0;
        records.contact.as_mut().unwrap().hit.camera_pitch_recoil = seed as i16;
        records.surface.as_mut().unwrap().material = low & 7;
        records.surface.as_mut().unwrap().plane_height = -60;
        records.charge.as_mut().unwrap().linked_mode = seed % 5 == 0;
        let owner = f.native.owner;
        f.native.objects.get_mut(owner).unwrap().base.speed = low;
        f.native
            .objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .motion
            .carry_selected_player = seed % 3 == 0;
        let storage =
            player_storage::get_mut(&f.native.objects, &mut f.runtime.resources, owner).unwrap();
        storage.fine_pitch = seed.wrapping_mul(8191);
        storage.fine_yaw = seed.wrapping_mul(5017);
        f.seed(&mut source);
        f.plane(&mut source);
        let result = f.step(&mut source);
        effects += f.native.objects.len() - 4;
        surface_events += usize::from(result.surface_response.effect_events != 0);
    }
    assert!(
        effects > 100 && surface_events > 100,
        "actual nested effect creation and surface-to-speed handoffs required"
    );
}

#[test]
fn complete_flight_matches_original_all_live_shapes_and_damage_forecasts() {
    let mut source = Source::new(&rom(), 0);
    source.bus.enable_gsu();
    let mut contacts = 0;
    let mut traversals = 0;
    let mut damage = 0;
    for shape in 0..577_u16 {
        for case in 0..4_u16 {
            let mut f = Fixture::new(&mut source);
            let collider = f.native.objects.get_mut(f.collider).unwrap();
            collider.base.shape = ShapeId::from_catalog_index(shape);
            collider.base.position = Vector3 {
                x: 0,
                y: collider.base.shape.catalog_entry().unwrap().bounds[1] as i16,
                z: 0,
            };
            collider.base.attack_power = 15;
            collider.base.yaw = Angle::from_units((shape * 7 + case * 37) as u8);
            let owner = f.native.objects.get_mut(f.native.owner).unwrap();
            owner.base.position = Vector3 {
                x: 0,
                y: if case & 1 != 0 { 0 } else { -1 },
                z: 0,
            };
            f.records().vertical.as_mut().unwrap().motion_axes = 0;
            f.records().surface.as_mut().unwrap().material = 6;
            f.native.world.surface_mode = Some(SurfaceMode {
                flags: if case & 2 != 0 { 6 } else { 0 },
            });
            f.native.world.scene.player_configuration = Some(if case & 1 != 0 { 9 } else { 0 });
            f.seed(&mut source);
            let result = f.step(&mut source);
            contacts += usize::from(
                f.native
                    .objects
                    .get(f.native.owner)
                    .unwrap()
                    .extension
                    .surface_contact
                    .supporting_object
                    .is_some(),
            );
            traversals += usize::from(result.diagonal_tie_bias == Some(0));
            damage += usize::from(f.records().contact.unwrap().hit.reserve_shield < 96);
        }
    }
    assert!(contacts > 100 && traversals > 100 && damage > 100, "live support, broad traversal and shield damage must all execute: {contacts}, {traversals}, {damage}");
}

#[test]
fn complete_flight_matches_original_independently_retained_live_visits() {
    let mut source = Source::new(&rom(), 0);
    source.bus.enable_gsu();
    let mut f = Fixture::new(&mut source);
    f.native.world.reflect_all_contacts = Some(true);
    f.seed(&mut source);
    f.plane(&mut source);
    let mut constrained = 0;
    let mut obstructed = 0;
    for visit in 0..8192_u16 {
        // External inputs are identical. Everything produced by the preceding
        // flight visit remains independent on the two implementations.
        f.native.world.strategy_clock = visit;
        source.bus.write8(0xC4, visit as u8);
        let configuration = if visit % 13 < 7 { 9 } else { 0 };
        f.native.world.scene.player_configuration = Some(configuration);
        source.bus.write8(0x1DE2, configuration);
        let held = visit.wrapping_mul(17) & 0x0F30;
        f.native.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(held),
            pressed: Buttons::from_bits(0),
        });
        source.bus.write16(0x1938, held);
        let yaw = visit.wrapping_mul(919);
        f.native.world.player_yaw_increment = Some(yaw);
        source.bus.write16(0x1E38, yaw);
        player_motion::capture_position(&f.native.objects, &mut f.native.world, f.native.owner)
            .unwrap();
        for (position, previous) in [(12, 0x6AC7), (14, 0x6AC9), (16, 0x6ACB)] {
            source.bus.write16(
                WRAM + SLOT + previous,
                source.bus.read16(u32::from(OWNER) + position),
            );
        }
        let position = Vector3 {
            x: ((visit % 53) * 31 + 300) as i16,
            y: if visit & 1 == 0 { 0 } else { -150 },
            z: ((visit % 37) * 43 + 300) as i16,
        };
        f.native.objects.get_mut(f.collider).unwrap().base.position = position;
        for (offset, value) in [(12, position.x), (14, position.y), (16, position.z)] {
            source
                .bus
                .write16(u32::from(address(Some(f.collider))) + offset, value as u16);
        }
        // These caller-owned inputs are re-established, not copied from source
        // scratch left by a prior nested collision or grid call.
        f.seed_context(&mut source);
        let result = f.step(&mut source);
        constrained += usize::from(result.surface_motion.is_some());
        obstructed += usize::from(f.records().motion.unwrap().contact_flags & 0x50 != 0);
    }
    assert!(constrained > 4000);
    assert!(obstructed > 1000);
}
