//! Original contact/pose/charge sharing, flight recoil and full post-flight
//! surface damage. The only synthetic code is the existing call bootstrap.

use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::collision_surface::{ActorSurfaceContact, SurfaceMode};
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_protection::DeflectionProtection;
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_hit_control::Impact;
use sf2_game::player_motion::{self, MotionContext, PlayerSurfaceSupport};
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::view_transition::ViewTransitionMode;
use sf2_game::weapon_dispatch::WeaponState;
use sf2_game::{
    player_impact, player_surface_damage, Angle, ObjectId, RandomState, ShapeId, SoundEvent,
    Vector3,
};
use sf_oracle::{call_near, Entry};

struct Fixture {
    native: Native,
    runtime: PathRuntime,
    proxy: ObjectId,
    collider: ObjectId,
}
impl Fixture {
    fn new(source: &mut Source) -> Self {
        source.bus.enable_gsu();
        let mut native = Native::new(source, 3, 0, 0, 0);
        let mut runtime = PathRuntime::default();
        player_storage::initialize(
            &mut native.objects,
            &mut native.world,
            &mut runtime,
            native.owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 0,
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
        let proxy = others[0];
        let collider = others[1];
        native.world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        native.world.surface_mode = Some(SurfaceMode { flags: 1 });
        native.world.scene.player_configuration = Some(0);
        native.world.primary_player = Some(native.owner);
        native.world.view_transition_mode = Some(ViewTransitionMode { flags: 0 });
        native.world.player_carry_mode = Some(0);
        native.world.environment_plane_height = Some(0);
        native.world.player_surface_support = Some(PlayerSurfaceSupport::default());
        native.world.random = RandomState::new([17, 63, 149, 211]);
        Self {
            native,
            runtime,
            proxy,
            collider,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.native
            .world
            .player_mut(&self.native.objects, self.native.owner)
            .unwrap()
    }
    fn seed(&self, source: &mut Source) {
        let native = &self.native;
        source.bus.write16(0x14D6, address(Some(self.proxy)));
        source
            .bus
            .write16(0x12C3, address(native.world.primary_player));
        source
            .bus
            .write8(0x1DE2, native.world.scene.player_configuration.unwrap());
        source
            .bus
            .write8(0x1B4D, native.world.surface_mode.unwrap().flags);
        source.bus.write8(0xC4, native.world.strategy_clock as u8);
        source.bus.write16(0x1B84, 0);
        source.bus.write16(0x1E0F, 0);
        source.bus.write8(0x1E13, 0);
        for (i, value) in native.world.random.bytes().into_iter().enumerate() {
            source.bus.write8(0xE0 + i as u32, value);
        }
        for (id, object) in native.objects.active_objects() {
            let base = WRAM + u32::from(address(Some(id)));
            source.bus.write16(
                base + 4,
                0xBC9C + object.base.shape.catalog_index() as u16 * 28,
            );
            source.bus.write8(
                base + 0x24,
                u8::from(object.base.flags.exclude_from_shape_footprint_search) * 4,
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
            for (offset, value) in [
                (12, object.base.position.x),
                (14, object.base.position.y),
                (16, object.base.position.z),
                (0x32, object.base.velocity.x),
                (0x34, object.base.velocity.y),
                (0x36, object.base.velocity.z),
            ] {
                source.bus.write16(base + offset, value as u16);
            }
            for (offset, value) in [
                (0x12, object.base.pitch.units()),
                (0x14, object.base.yaw.units()),
                (0x16, object.base.roll.units()),
                (0x18, object.base.speed),
                (0x2D, object.base.hit_points),
                (0x2E, object.base.attack_power),
            ] {
                source.bus.write8(base + offset, value);
            }
            source
                .bus
                .write16(base + 0x1CE2, object.extension.path_state.motion_phase);
            source.bus.write16(
                base + 0x1CE8,
                address(object.extension.surface_contact.supporting_object),
            );
            source
                .bus
                .write8(base + 0x1CEA, object.extension.surface_contact.group);
            source
                .bus
                .write8(base + 0x1CEB, object.extension.surface_contact.flags);
        }
        let record = native.world.player(&native.objects, native.owner).unwrap();
        let pose = record.pose.unwrap();
        let hit = record.contact.unwrap().hit;
        let motion = record.motion.unwrap();
        let previous = motion.previous_position;
        let displacement = record.flight_displacement.unwrap();
        let storage =
            player_storage::get(&native.objects, &self.runtime.resources, native.owner).unwrap();
        for (offset, value) in [
            (0x6AAD, motion.lateral_impulse as u8),
            (0x6AD4, pose.yaw_trim as u8),
            (0x6ADA, pose.heading_return_bank as u8),
            (0x6C09, record.charge.unwrap().control),
            (0x6BE3, hit.recovery),
            (0x6C11, hit.feedback_duration),
            (0x6C12, hit.feedback_flags),
            (0x6C00, hit.reserve_shield),
            (0x6BE7, hit.deflection_sound_cooldown),
            (0x6AA0, record.auxiliary.unwrap().mode),
            (0x6C02, record.protection.unwrap().control()),
            (0x6BE6, motion.contact_flags),
            (0x6B62, record.speed.unwrap().thrust as u8),
            (0x6B84, record.vertical.unwrap().motion_axes),
            (0x6B7D, 0),
            (0x6B77, record.auxiliary.unwrap().action_flags),
            (0x6ABD, storage.bank.units()),
            (0x6ACF, pose.pitch_lean as u8),
            (0x6AD5, pose.ambient_bank as u8),
            (0x6AD7, pose.steering_bank as u8),
            (0x6ADE, pose.yaw_offset as u8),
            (0x6ADD, record.roll.unwrap().impulse as u8),
        ] {
            source.bus.write8(WRAM + SLOT + offset, value);
        }
        for (offset, value) in [
            (0x6AB9, storage.fine_pitch),
            (0x6ABB, storage.fine_yaw),
            (0x6ACD, record.yaw_motion.unwrap()),
            (0x6AD0, pose.turning_lean),
            (0x6AD8, pose.shoulder_bank as u16),
            (0x6B3B, hit.camera_pitch_recoil as u16),
            (0x6AC7, previous.x as u16),
            (0x6AC9, previous.y as u16),
            (0x6ACB, previous.z as u16),
            (0x6B0B, displacement.x as u16),
            (0x6B0D, displacement.y as u16),
            (0x6B0F, displacement.z as u16),
        ] {
            source.bus.write16(WRAM + SLOT + offset, value);
        }
        source.bus.write16(
            0x1DAE,
            native.world.player_surface_height.unwrap_or(0x1234) as u16,
        );
    }
    fn compare(&mut self, source: &mut Source, mut previous: PlayerPathRecords) {
        let byte = |offset| source.bus.read8(WRAM + SLOT + offset);
        let word = |offset| source.bus.read16(WRAM + SLOT + offset);
        previous.motion.as_mut().unwrap().lateral_impulse = byte(0x6AAD) as i8;
        previous.motion.as_mut().unwrap().contact_flags = byte(0x6BE6);
        previous.motion.as_mut().unwrap().previous_position = Vector3 {
            x: word(0x6AC7) as i16,
            y: word(0x6AC9) as i16,
            z: word(0x6ACB) as i16,
        };
        previous.flight_displacement = Some(Vector3 {
            x: word(0x6B0B) as i16,
            y: word(0x6B0D) as i16,
            z: word(0x6B0F) as i16,
        });
        previous.pose.as_mut().unwrap().yaw_trim = byte(0x6AD4) as i8;
        previous.pose.as_mut().unwrap().heading_return_bank = byte(0x6ADA) as i8;
        previous.charge.as_mut().unwrap().control = byte(0x6C09);
        previous.yaw_motion = Some(word(0x6ACD));
        let hit = &mut previous.contact.as_mut().unwrap().hit;
        hit.recovery = byte(0x6BE3);
        hit.feedback_duration = byte(0x6C11);
        hit.feedback_flags = byte(0x6C12);
        hit.camera_pitch_recoil = word(0x6B3B) as i16;
        hit.reserve_shield = byte(0x6C00);
        hit.deflection_sound_cooldown = byte(0x6BE7);
        assert_eq!(*self.records(), previous);
        let native = &self.native;
        assert_eq!(
            player_storage::get(&native.objects, &self.runtime.resources, native.owner)
                .unwrap()
                .fine_pitch,
            source.bus.read16(WRAM + SLOT + 0x6AB9)
        );
        let storage =
            player_storage::get(&native.objects, &self.runtime.resources, native.owner).unwrap();
        assert_eq!(storage.fine_yaw, source.bus.read16(WRAM + SLOT + 0x6ABB));
        assert_eq!(storage.bank.units(), source.bus.read8(WRAM + SLOT + 0x6ABD));
        if let Some(height) = native.world.player_surface_height {
            assert_eq!(height as u16, source.bus.read16(0x1DAE));
        }
        for (id, object) in native.objects.active_objects() {
            let base = WRAM + u32::from(address(Some(id)));
            for (offset, value) in [
                (12, object.base.position.x),
                (14, object.base.position.y),
                (16, object.base.position.z),
                (0x32, object.base.velocity.x),
                (0x34, object.base.velocity.y),
                (0x36, object.base.velocity.z),
            ] {
                assert_eq!(
                    value as u16,
                    source.bus.read16(base + offset),
                    "actor {id:?} field {offset:X}"
                );
            }
            assert_eq!(object.base.hit_points, source.bus.read8(base + 0x2D));
            for (offset, angle) in [
                (0x12, object.base.pitch),
                (0x14, object.base.yaw),
                (0x16, object.base.roll),
            ] {
                assert_eq!(
                    angle.units(),
                    source.bus.read8(base + offset),
                    "actor {id:?} angle {offset:X}"
                );
            }
            assert_eq!(
                object.extension.path_state.motion_phase,
                source.bus.read16(base + 0x1CE2)
            );
            assert_eq!(
                address(object.extension.surface_contact.supporting_object),
                source.bus.read16(base + 0x1CE8)
            );
            assert_eq!(
                object.extension.surface_contact.group,
                source.bus.read8(base + 0x1CEA)
            );
            assert_eq!(
                object.extension.surface_contact.flags,
                source.bus.read8(base + 0x1CEB)
            );
        }
        assert_eq!(
            native.world.random.bytes(),
            std::array::from_fn(|i| source.bus.read8(0xE0 + i as u32))
        );
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
        assert_eq!(source.bus.read16(0x1D16) as usize, events.len() * 2);
        for (i, event) in events.into_iter().enumerate() {
            assert_eq!(event, source.bus.read16(0x1CF6 + i as u32 * 2));
        }
    }
    fn damage(&mut self, source: &mut Source) {
        let before = *self.records();
        source.bus.write16(0x1D16, 0);
        source.run(0x07E18E, None, 0, OWNER, true);
        player_surface_damage::advance(
            &mut self.native.objects,
            &mut self.native.world,
            &mut self.runtime.resources,
            self.native.owner,
        )
        .unwrap();
        self.compare(source, before);
    }
    fn box_contact(&mut self) {
        let shape = (1..sf2_data::shape_data::SHAPE_DATA_COUNT)
            .map(|index| ShapeId::from_catalog_index(index as u16))
            .find(|shape| {
                shape
                    .catalog_entry()
                    .unwrap()
                    .bounds
                    .iter()
                    .all(|&size| size > 4)
                    && sf2_data::collision_data::collision_profile_by_index(shape.catalog_index())
                        .is_none()
            })
            .unwrap();
        let collider = self.native.objects.get_mut(self.collider).unwrap();
        collider.base.shape = shape;
        collider.base.contacts.first_strategy_visit = false;
        collider.base.position = Vector3 {
            x: 0,
            y: shape.catalog_entry().unwrap().bounds[1] as i16,
            z: 0,
        };
        self.native
            .objects
            .get_mut(self.native.owner)
            .unwrap()
            .base
            .position = Vector3 { x: 1, y: 1, z: 1 };
    }
}

#[test]
fn recoil_matches_original_every_yaw_and_signed_impulse_including_retained_vertical_integration() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = Fixture::new(&mut source);
    for value in 0..=u16::MAX {
        let owner = fixture
            .native
            .objects
            .get_mut(fixture.native.owner)
            .unwrap();
        owner.base.yaw = Angle::from_units(value as u8);
        owner.base.position = Vector3 {
            x: value as i16,
            y: value.rotate_left(5) as i16,
            z: !value as i16,
        };
        owner.base.velocity = Vector3 {
            x: 71,
            y: value.rotate_left(9) as i16,
            z: -99,
        };
        owner.extension.path_state.motion_phase = 0xA55A;
        fixture.records().motion.as_mut().unwrap().lateral_impulse = (value >> 8) as i8;
        fixture.seed(&mut source);
        let before = *fixture.records();
        source.run(0x06E273, Some(0x06E2D0), 0, OWNER, true);
        player_impact::advance_recoil(
            &mut fixture.native.objects,
            &mut fixture.native.world,
            fixture.native.owner,
        )
        .unwrap();
        fixture.compare(&mut source, before);
    }
}

#[test]
fn impacts_match_original_all_charge_recovery_bytes_and_real_pose_bank_publication() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = Fixture::new(&mut source);
    for value in 0..=u16::MAX {
        for impact in [Impact::Light, Impact::Heavy] {
            fixture.native.world.strategy_clock = value;
            let record = fixture.records();
            record.charge.as_mut().unwrap().control = value as u8;
            record.pose.as_mut().unwrap().heading_return_bank = !(value as u8) as i8;
            let hit = &mut record.contact.as_mut().unwrap().hit;
            hit.recovery = (value >> 8) as u8;
            hit.camera_pitch_recoil = if value & 1 == 0 { 0 } else { value as i16 };
            hit.feedback_flags = value.rotate_left(3) as u8;
            fixture.seed(&mut source);
            let before = *fixture.records();
            source.run(
                if impact == Impact::Light {
                    0x06AA6F
                } else {
                    0x06AAAE
                },
                None,
                0,
                OWNER,
                true,
            );
            player_impact::impact(
                &fixture.native.objects,
                &mut fixture.native.world,
                fixture.native.owner,
                impact,
            )
            .unwrap();
            fixture.compare(&mut source, before);
        }
    }
}

#[test]
fn contact_turn_matches_original_gates_shared_phase_and_full_coordinate_words() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = Fixture::new(&mut source);
    for value in 0..=u16::MAX {
        fixture.native.world.scene.player_configuration = Some(if value & 7 == 0 { 9 } else { 0 });
        fixture.records().auxiliary.as_mut().unwrap().mode = (value >> 8) as u8;
        fixture
            .native
            .objects
            .get_mut(fixture.collider)
            .unwrap()
            .base
            .contacts
            .credits_hit_side = value & 3 == 0;
        fixture
            .native
            .objects
            .get_mut(fixture.collider)
            .unwrap()
            .base
            .position = Vector3 {
            x: value as i16,
            y: 171,
            z: value.rotate_left(5) as i16,
        };
        let owner = fixture
            .native
            .objects
            .get_mut(fixture.native.owner)
            .unwrap();
        owner.base.yaw = Angle::from_units(value as u8);
        owner.extension.path_state.motion_phase = value;
        fixture.seed(&mut source);
        let before = *fixture.records();
        source.run_with_y(
            0x069842,
            None,
            0,
            OWNER,
            true,
            Some(address(Some(fixture.collider))),
        );
        player_impact::turn_from_actor(
            &mut fixture.native.objects,
            &mut fixture.native.world,
            fixture.native.owner,
            fixture.collider,
        )
        .unwrap();
        fixture.compare(&mut source, before);
    }
}

#[test]
fn surface_damage_matches_original_all_reserve_attack_pairs_without_spill_and_unsigned_sound_selection(
) {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = Fixture::new(&mut source);
    fixture.box_contact();
    for value in 0..=u16::MAX {
        let owner = fixture
            .native
            .objects
            .get_mut(fixture.native.owner)
            .unwrap();
        owner.base.hit_points = 91;
        owner.base.yaw = Angle::from_units(value as u8);
        owner.extension.surface_contact.group = (value >> 8) as u8;
        fixture
            .native
            .objects
            .get_mut(fixture.collider)
            .unwrap()
            .base
            .attack_power = value as u8;
        fixture.native.world.primary_player = (value & 1 == 0).then_some(fixture.native.owner);
        fixture.native.world.strategy_clock = value;
        let record = fixture.records();
        record.contact.as_mut().unwrap().hit.reserve_shield = (value >> 8) as u8;
        record.charge.as_mut().unwrap().control = value as u8;
        fixture.seed(&mut source);
        fixture.damage(&mut source);
        assert_eq!(
            fixture
                .native
                .objects
                .get(fixture.native.owner)
                .unwrap()
                .extension
                .surface_contact
                .supporting_object,
            Some(fixture.collider)
        );
    }
}

#[test]
fn surface_damage_matches_original_all_shape_headers_and_forecast_gate_combinations() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = Fixture::new(&mut source);
    let mut contacts = 0;
    let mut forecasts = 0;
    for index in 0..sf2_data::shape_data::SHAPE_DATA_COUNT {
        for case in 0..32_u16 {
            let shape = ShapeId::from_catalog_index(index as u16);
            let collider = fixture.native.objects.get_mut(fixture.collider).unwrap();
            collider.base.shape = shape;
            collider.base.position = Vector3 {
                x: 0,
                y: shape.catalog_entry().unwrap().bounds[1] as i16,
                z: 0,
            };
            collider.base.yaw = Angle::from_units((case * 37) as u8);
            collider.base.contacts.first_strategy_visit = false;
            collider.base.attack_power = (case * 11) as u8;
            let owner = fixture
                .native
                .objects
                .get_mut(fixture.native.owner)
                .unwrap();
            owner.base.position = Vector3 {
                x: (case as i16 & 7) - 3,
                y: [-32, -1, 0, 1, 2, 255, 16384, -16384][usize::from(case & 7)],
                z: 0,
            };
            owner.base.hit_points = 13;
            owner.base.contacts.skip_contacts = case & 4 != 0;
            owner.extension.surface_contact = ActorSurfaceContact {
                group: 197,
                flags: 0xFF,
                supporting_object: Some(fixture.proxy),
            };
            fixture
                .native
                .objects
                .get_mut(fixture.proxy)
                .unwrap()
                .base
                .position = Vector3 {
                x: 123,
                y: -123,
                z: 321,
            };
            fixture.native.world.scene.player_configuration =
                Some(if case & 1 == 0 { 9 } else { 0 });
            fixture.native.world.surface_mode = Some(SurfaceMode {
                flags: (case / 8) as u8,
            });
            let record = fixture.records();
            record.protection = Some(DeflectionProtection::from_control((case * 13) as u8));
            record.contact.as_mut().unwrap().hit.reserve_shield = 32;
            record
                .contact
                .as_mut()
                .unwrap()
                .hit
                .deflection_sound_cooldown = (case % 3) as u8;
            record.motion.as_mut().unwrap().contact_flags = 0xFF;
            record.motion.as_mut().unwrap().previous_position = Vector3 {
                x: 0,
                y: (case & 3) as i16 - 1,
                z: 0,
            };
            record.flight_displacement = Some(Vector3 { x: -1, y: 1, z: 1 });
            fixture.seed(&mut source);
            fixture.damage(&mut source);
            contacts += usize::from(
                fixture
                    .native
                    .objects
                    .get(fixture.native.owner)
                    .unwrap()
                    .extension
                    .surface_contact
                    .supporting_object
                    .is_some(),
            );
            forecasts += usize::from(
                fixture
                    .native
                    .objects
                    .get(fixture.proxy)
                    .unwrap()
                    .base
                    .position
                    .x
                    != 123,
            );
        }
    }
    assert!(
        contacts > 0 && forecasts > 0,
        "real collider and forecast branches required"
    );
}

#[test]
fn continuous_pose_translation_recoil_damage_and_history_share_original_retained_state() {
    let mut source = Source::new(&rom(), 0);
    let mut contacts = 0;
    let mut recoils = 0;
    for scenario in 0..32_u16 {
        let mut fixture = Fixture::new(&mut source);
        fixture.box_contact();
        fixture.records().auxiliary.as_mut().unwrap().mode = 0x10;
        fixture.records().vertical.as_mut().unwrap().motion_axes = 0xE0;
        fixture
            .native
            .objects
            .get_mut(fixture.collider)
            .unwrap()
            .base
            .attack_power = 1 + (scenario % 7) as u8;
        fixture.seed(&mut source);
        for visit in 0..128_u16 {
            let target = scenario
                .wrapping_mul(257)
                .wrapping_add(visit.wrapping_mul(29));
            let turn = (visit as i16 % 11 - 5) as u16;
            let speed = 1 + (visit % 7) as u8;
            let thrust = (visit % 5) as i8 - 2;
            fixture.native.world.strategy_clock = visit;
            fixture.native.world.player_pitch_target = Some(target);
            fixture.native.world.player_yaw_increment = Some(turn);
            fixture
                .native
                .objects
                .get_mut(fixture.native.owner)
                .unwrap()
                .base
                .speed = speed;
            fixture.records().speed.as_mut().unwrap().thrust = thrust;
            source.bus.write16(0x1E36, target);
            source.bus.write16(0x1E38, turn);
            source.bus.write8(0xC4, visit as u8);
            source.bus.write8(u32::from(OWNER) + 0x18, speed);
            source.bus.write8(WRAM + SLOT + 0x6B62, thrust as u8);
            source.bus.write16(0x1D16, 0);
            let before = *fixture.records();
            source.run_with_y(0x069E25, Some(0x069E36), 0, OWNER, true, Some(SLOT as u16));
            player_motion::capture_position(
                &fixture.native.objects,
                &mut fixture.native.world,
                fixture.native.owner,
            )
            .unwrap();
            let entry = Entry {
                x: OWNER,
                p: 0x20,
                dbr: 0x7E,
                ..Default::default()
            };
            assert!(call_near(&mut source.bus, 0x06ECB0, &entry).returned);
            sf2_game::player_pose::compose(
                &mut fixture.native.objects,
                &mut fixture.native.world,
                &mut fixture.runtime.resources,
                fixture.native.owner,
            )
            .unwrap();
            assert!(call_near(&mut source.bus, 0x06EE0A, &entry).returned);
            player_motion::advance(
                &mut fixture.native.objects,
                &mut fixture.native.world,
                &fixture.runtime.resources,
                fixture.native.owner,
                MotionContext::default(),
            )
            .unwrap();
            source.run(0x06E273, Some(0x06E2D0), 0, OWNER, true);
            recoils += usize::from(fixture.records().motion.unwrap().lateral_impulse != 0);
            player_impact::advance_recoil(
                &mut fixture.native.objects,
                &mut fixture.native.world,
                fixture.native.owner,
            )
            .unwrap();
            source.run(0x07E18E, None, 0, OWNER, true);
            player_surface_damage::advance(
                &mut fixture.native.objects,
                &mut fixture.native.world,
                &mut fixture.runtime.resources,
                fixture.native.owner,
            )
            .unwrap();
            fixture.compare(&mut source, before);
            contacts += usize::from(
                fixture
                    .native
                    .objects
                    .get(fixture.native.owner)
                    .unwrap()
                    .extension
                    .surface_contact
                    .supporting_object
                    .is_some(),
            );
        }
    }
    assert!(
        contacts > 0 && recoils > 0,
        "retained contact-to-next-pose/movement handoff required"
    );
}
