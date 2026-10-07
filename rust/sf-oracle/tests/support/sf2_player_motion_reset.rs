//! Decode both sides of the unmodified partial record clear. Expected state
//! comes from original execution, not from the native reset implementation.
use super::{actor, rom, Source, OTHER, OWNER, WRAM};
use sf2_game::path_equipment::SelectedEquipment;
use sf2_game::path_player_control::PlayerTargetControl;
use sf2_game::path_program::{SelectedAuxiliaryState, SelectedParticleEffects};
use sf2_game::path_protection::DeflectionProtection;
use sf2_game::path_runtime::PathRuntime;
use sf2_game::path_target::TargetSelection;
use sf2_game::path_trigger_conditions::ControlledAuxFlags;
use sf2_game::platform_carry::CarriedPlayer;
use sf2_game::player_action::{PlayerAction, PlayerActionState};
use sf2_game::player_ambient::PlayerAmbient;
use sf2_game::player_appearance::PlayerAppearance;
use sf2_game::player_boundary::PlayerBoundary;
use sf2_game::player_camera_angles::{
    CameraAngleFractions, CameraPitchProfile, PlayerCameraAngles,
};
use sf2_game::player_camera_auxiliary::{AuxiliaryCameraTask, PlayerCameraAuxiliary};
use sf2_game::player_camera_dispatch::PlayerCameraDispatch;
use sf2_game::player_camera_ground::PlayerCameraGround;
use sf2_game::player_camera_position::PlayerCameraPosition;
use sf2_game::player_camera_surface::PlayerCameraSurface;
use sf2_game::player_camera_tracking::{PlayerCameraTracking, TrackingStyle};
use sf2_game::player_charge::PlayerCharge;
use sf2_game::player_consumable::{PlayerConsumableControl, TriggeredUseBlockers};
use sf2_game::player_hit_control::PlayerHitControl;
use sf2_game::player_mission::PlayerMissionControl;
use sf2_game::player_mode_selection::PlayerModeSelection;
use sf2_game::player_motion::PlayerMotion;
use sf2_game::player_occupancy::PlayerOccupancy;
use sf2_game::player_pose::PlayerPose;
use sf2_game::player_rapid::RapidAim;
use sf2_game::player_reticle::ReticleDisplay;
use sf2_game::player_roll::{PlayerRoll, ShoulderControl};
use sf2_game::player_speed::PlayerSpeed;
use sf2_game::player_status::PlayerStatus;
use sf2_game::player_steering::PlayerSteering;
use sf2_game::player_storage::{self, PlayerScore, PlayerStorage, PlayerStorageInputs};
use sf2_game::player_surface::PlayerSurface;
use sf2_game::player_target_lock::TargetLock;
use sf2_game::player_throttle::PlayerThrottle;
use sf2_game::player_vertical::{PlayerVerticalControl, VerticalProfile};
use sf2_game::player_view_distance::PlayerViewDistance;
use sf2_game::player_visit::PlayerVisitControl;
use sf2_game::program_state::ProgramData;
use sf2_game::scene_contact::PlayerContactControl;
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::{Angle, Buttons, InputState, ObjectId, ObjectStore, Rotation, Vector3};

struct Reader<'a> {
    source: &'a Source,
    slot: u32,
    other: ObjectId,
}

impl Reader<'_> {
    fn byte(&self, field: u32) -> u8 {
        self.source.bus.read8(WRAM + self.slot + field)
    }
    fn word(&self, field: u32) -> u16 {
        self.source.bus.read16(WRAM + self.slot + field)
    }
    fn vector(&self, field: u32) -> Vector3 {
        Vector3 {
            x: self.word(field) as i16,
            y: self.word(field + 2) as i16,
            z: self.word(field + 4) as i16,
        }
    }
    fn object(&self, field: u32) -> Option<ObjectId> {
        match self.word(field) {
            0 => None,
            OTHER => Some(self.other),
            value => panic!("unexpected original actor identity {value:04X} at {field:04X}"),
        }
    }
    fn storage(&self) -> PlayerStorage {
        PlayerStorage {
            fine_pitch: self.word(0x6AB9),
            fine_yaw: self.word(0x6ABB),
            bank: Angle::from_units(self.byte(0x6ABD)),
            retained_shield: self.byte(0x6C38),
        }
    }
    fn records(&self) -> PlayerPathRecords {
        PlayerPathRecords {
            contact: Some(PlayerContactControl {
                hit: PlayerHitControl {
                    recovery: self.byte(0x6BE3),
                    secondary_protection: self.byte(0x6BE2),
                    hold_secondary_protection: self.byte(0x6B7D) & 0x80 != 0,
                    feedback_duration: self.byte(0x6C11),
                    feedback_flags: self.byte(0x6C12),
                    camera_pitch_recoil: self.word(0x6B3B) as i16,
                    reserve_shield: self.byte(0x6C00),
                    deflection_sound_cooldown: self.byte(0x6BE7),
                },
                ignores_contacts: self.byte(0x6A72) & 0x10 != 0,
            }),
            protection: Some(DeflectionProtection::from_control(self.byte(0x6C02))),
            auxiliary: Some(SelectedAuxiliaryState {
                mode: self.byte(0x6AA0),
                action_flags: self.byte(0x6B77),
                stored_world_position: self.vector(0x6AC1),
                stored_rotation: Rotation {
                    pitch: Angle::from_units(self.byte(0x6B32)),
                    yaw: Angle::from_units(self.byte(0x6B34)),
                    roll: Angle::from_units(self.byte(0x6B36)),
                },
            }),
            charge: Some(PlayerCharge {
                progress: self.word(0x6C07),
                control: self.byte(0x6C09),
                linked_mode: self.byte(0x6B63) & 0x80 != 0,
                linked_muzzle_disabled: self.byte(0x6B63) & 0x40 != 0,
                rapid_control: self.byte(0x6B60),
                speed_impulse: self.word(0x6B56) as i16,
                speed_impulse_ticks: self.byte(0x6B58),
            }),
            rapid_aim: Some(RapidAim {
                roll_step: Angle::from_units(self.byte(0x6ADD)),
                retained_aim: self.vector(0x6B9C),
            }),
            rapid_rejection_consumes_queue: Some(self.slot & 255 != 0),
            action: Some(PlayerActionState {
                action: match (self.word(0x6C13), self.byte(0x6C15)) {
                    (0, 0) => None,
                    (0xBDDA, 0x0D) => Some(PlayerAction::TriggeredProjectile),
                    (0xBF63, 0x0D) => Some(PlayerAction::ForcedRetreat),
                    value => panic!("unexpected retained action {value:?}"),
                },
                elapsed: self.word(0x6C16),
                auxiliary_counter: self.word(0x6C18),
                total_updates: self.word(0x6C1A),
            }),
            mission: Some(PlayerMissionControl {
                flags: self.byte(0x6A71),
            }),
            consumable: Some(PlayerConsumableControl {
                input_control: self.byte(0x6B61),
                projectile_blockers: TriggeredUseBlockers::from_control(self.byte(0x6BE9)),
                recovery_blocked: self.byte(0x6B7D) & 0x40 != 0,
            }),
            target_control: Some(PlayerTargetControl {
                transition_delay: self.byte(0x6BEA),
                configuration_locked: self.byte(0x6A8C) & 0x80 != 0,
                offset_enabled: self.byte(0x6A8C) & 0x40 != 0,
                mode: self.word(0x6C1C),
                owner: self.object(0x6A98),
                origin: self.vector(0x6A92),
                range: self.word(0x6A90) as i16,
                positive_range: self.word(0x6C26),
                limit: self.word(0x6C24),
                axis_mode: self.byte(0x6C29),
                control: self.byte(0x6C28),
                axis_rates: [self.byte(0x6A8D), self.byte(0x6A8E), self.byte(0x6A8F)],
                axis_limits: [self.byte(0x6C2A), self.byte(0x6C2B), self.byte(0x6C2C)],
            }),
            target_selection: Some(TargetSelection {
                display_status: self.byte(0x6BB6),
                control_flags: self.byte(0x6BC2),
                forced_owner: self.object(0x6BCA),
                candidate: self.object(0x6BB8),
                distance: self.word(0x6BBC),
                auxiliary_distance: self.word(0x6BBA),
                position: self.vector(0x6BCC),
                pitch: self.word(0x6BC0),
                yaw: self.word(0x6BBE),
                screen: [self.byte(0x6BAD), self.byte(0x6BAF)],
                clipped_yaw: self.byte(0x6BC5),
            }),
            target_lock: Some(TargetLock {
                previous_candidate: self.object(0x6BC8),
                acquisition_clock: self.byte(0x6BC6),
                grace_remaining: self.byte(0x6BC7),
                marker_style: self.byte(0x6BB7),
            }),
            reticle_display: Some(ReticleDisplay {
                roll: Angle::from_units(self.byte(0x6BA2)),
                mode_flags: self.byte(0x6BA3),
                display_flags: self.byte(0x6BA4),
            }),
            visit: Some(PlayerVisitControl {
                pilot_code: self.byte(0x6BFF),
                shield_warning_clock: self.byte(0x6BE8),
            }),
            status: Some(PlayerStatus {
                shield_warning_history: self.byte(0x6C0A),
            }),
            appearance: Some(PlayerAppearance {
                depth_control: self.byte(0x6AA2),
            }),
            injected_input: Some(InputState {
                held: Buttons::from_bits(self.word(0x6A8A)),
                pressed: Buttons::from_bits(self.word(0x6A88)),
            }),
            roll: Some(PlayerRoll {
                shoulders: ShoulderControl::from_bits(self.byte(0x6B7E)),
                tap_window: self.byte(0x6ADC),
                impulse: self.byte(0x6ADD) as i8,
            }),
            pose: Some(PlayerPose {
                pitch_lean: self.byte(0x6ACF) as i8,
                turning_lean: self.word(0x6AD0),
                yaw_trim: self.byte(0x6AD4) as i8,
                ambient_bank: self.byte(0x6AD5) as i8,
                steering_bank: self.byte(0x6AD7) as i8,
                shoulder_bank: self.word(0x6AD8) as i16,
                heading_return_bank: self.byte(0x6ADA) as i8,
                yaw_offset: self.byte(0x6ADE) as i8,
            }),
            steering: Some(PlayerSteering {
                direction_age: self.byte(0x6B17),
                turn_response: self.word(0x6AD2),
                camera_bank_target: self.byte(0x6AC0) as i8,
                locked_heading: Angle::from_units(self.byte(0x6AAE)),
                lateral_offset: self.word(0x6B15) as i16,
            }),
            vertical: Some(PlayerVerticalControl {
                profile: VerticalProfile {
                    upper_height_offset: self.word(0x6BF5) as i16,
                    lower_height_offset: self.word(0x6BF7) as i16,
                    up_pitch: self.byte(0x6BF9),
                    down_pitch: self.byte(0x6BFA),
                },
                limit_flags: self.byte(0x6B81),
                control_flags: self.byte(0x6B82),
                motion_axes: self.byte(0x6B84),
                pitch_adjustment: self.byte(0x6ABF),
                latched_input: self.word(0x6B87),
                previous_held: self.word(0x6B89),
            }),
            throttle: Some(PlayerThrottle {
                brake_preference: self.byte(0x6B78),
                effect_flags: self.byte(0x6B79),
                effect_level: self.byte(0x6B7A),
                entry_marker: self.byte(0x6B7F),
            }),
            ambient: Some(PlayerAmbient {
                bank_phase: self.byte(0x6AD6),
                offset_phase: self.byte(0x6ADB),
                retained_offset: self.word(0x6AE2) as i16,
            }),
            flight_displacement: Some(self.vector(0x6B0B)),
            surface: Some(PlayerSurface {
                plane_height: self.word(0x6A7D) as i16,
                material: self.byte(0x6A82),
            }),
            speed: Some(PlayerSpeed {
                thrust: self.byte(0x6B62) as i8,
            }),
            motion: Some(PlayerMotion {
                walker_turn_control: self.byte(0x6AE9),
                walker_stride_control: self.byte(0x6AEA),
                walker_attachment_yaw: self.byte(0x6AEE),
                walker_contact_control: self.byte(0x6B94),
                walker_motion_control: self.byte(0x6AA4),
                surface_height: self.word(0x6AF7) as i16,
                previous_position: self.vector(0x6AC7),
                lateral_impulse: self.byte(0x6AAD) as i8,
                surface_velocity: [self.word(0x6B11) as i16, self.word(0x6B13) as i16],
                contact_flags: self.byte(0x6BE6),
            }),
            boundary: Some(PlayerBoundary {
                reset_control: self.byte(0x6A61),
                center: self.vector(0x6AAF),
                half_width: self.word(0x6AB5) as i16,
                half_height: self.word(0x6AB7) as i16,
                return_position: self.vector(0x6BED),
            }),
            occupancy: Some(PlayerOccupancy {
                current_cell: [self.byte(0x6B29) as i8, self.byte(0x6B2A) as i8],
                previous_cell: [self.byte(0x6B2C) as i8, self.byte(0x6B2D) as i8],
                displacement: [self.word(0x6AF9) as i16, self.word(0x6AFB) as i16],
            }),
            mode_selection: Some(PlayerModeSelection {
                requested: self.byte(0x6AA1),
                transition_control: self.byte(0x6BEC),
                surface_control: self.byte(0x6B64),
                cue_control: self.byte(0x6B9B),
            }),
            view_distance: Some(PlayerViewDistance {
                distance: self.word(0x6B67) as i16,
                boost_response: self.word(0x6B69) as i16,
                brake_response: self.word(0x6B6B) as i16,
                linked_target: self.word(0x6B6D) as i16,
                external_target: self.word(0x6B6F) as i16,
                reserved_profile: [self.word(0x6B71) as i16, self.word(0x6B73) as i16],
                pitch_height_offset: self.word(0x6B75) as i16,
                capture_pending: self.byte(0x6B63) & 0x20 != 0,
                switch_requested: self.byte(0x6B63) & 4 != 0,
            }),
            camera_angles: Some(PlayerCameraAngles {
                fractions: CameraAngleFractions {
                    pitch: self.byte(0x6B31),
                    yaw: self.byte(0x6B33),
                    roll: self.byte(0x6B35),
                },
                height_control: self.byte(0x6B30),
                pitch_increment: self.word(0x6B3F) as i16,
                yaw_offset: self.word(0x6B3D) as i16,
                yaw_difference: self.word(0x6B39) as i16,
                profile: CameraPitchProfile {
                    up: self.byte(0x6BFD) as i8,
                    down: self.byte(0x6BFE) as i8,
                },
            }),
            camera_tracking: Some(PlayerCameraTracking {
                anchor_height: self.word(0x6B45) as i16,
                height_difference: self.word(0x6B47) as i16,
                vertical_offset: self.word(0x6B4E) as i16,
            }),
            camera_position: Some(PlayerCameraPosition {
                lateral_offset: self.word(0x6B4A) as i16,
                secondary_lateral_offset: self.word(0x6B4C) as i16,
                lateral_accumulator: self.word(0x6AE7) as i16,
                longitudinal_offset: self.word(0x6B52) as i16,
            }),
            camera_ground: Some(PlayerCameraGround {
                hold_pitch: self.byte(0x6B7D) & 0x10 != 0,
                follow_environment_plane: self.byte(0x6B7D) & 8 != 0,
                height_offset: self.word(0x6B5A) as i16,
                height_target_adjustment: self.word(0x6B5C) as i16,
                carried_target_height: self.word(0x6BF3) as i16,
                animation_pitch: self.word(0x6B37) as i16,
            }),
            camera_surface: Some(PlayerCameraSurface {
                returning_below_plane: self.byte(0x6B7D) & 0x20 != 0,
            }),
            camera_auxiliary: Some(PlayerCameraAuxiliary {
                task: match (self.word(0x6A9D), self.byte(0x6A9F)) {
                    (0, 0) => AuxiliaryCameraTask::None,
                    (0x9DF6, 7) => AuxiliaryCameraTask::Handoff,
                    value => panic!("unexpected auxiliary camera {value:?}"),
                },
                retreat_distance: self.word(0x6B54) as i16,
            }),
            camera_dispatch: Some(PlayerCameraDispatch {
                style: match (self.word(0x6A9A), self.byte(0x6A9C)) {
                    (0, 0) => None,
                    (0x8048, 7) => Some(TrackingStyle::Normal),
                    value => panic!("unexpected primary camera {value:?}"),
                },
                projection_correction_disabled: self.byte(0x6B65) & 0x40 != 0,
                outside_occupied_world: self.byte(0x6B63) & 2 != 0,
            }),
            yaw_motion: Some(self.word(0x6ACD)),
            occupancy_exempt: Some(self.byte(0x6BEB) & 0x80 != 0),
            equipment: Some(SelectedEquipment {
                packed_consumables: self.byte(0x6C04),
                consumable_type: self.byte(0x6C05),
                weapon_level: self.byte(0x6C06),
            }),
            score: Some(PlayerScore::from_parts(
                self.word(0x6C33),
                self.byte(0x6C35),
            )),
            particles: Some(SelectedParticleEffects {
                flags: self.byte(0x6BE4),
                age: self.byte(0x6BE5),
            }),
            controlled_flags: Some(ControlledAuxFlags {
                high: self.byte(0x6B65) & 0x10 != 0,
                low: self.byte(0x6B65) & 8 != 0,
            }),
            carried: Some(CarriedPlayer {
                enabled: true,
                carrier: Some(self.other),
                origin: self.vector(0x6BED),
                fine_yaw: self.word(0x6ABB),
            }),
        }
    }
}

#[test]
fn partial_motion_reset_and_complete_entry_preparation_match_original_every_field() {
    let rom = rom();
    for prefix_cost in [0, 1, 247, 248, 249, 503, 1000] {
        for seed in 0..=u8::MAX {
            let mut source = Source::new(&rom, 0);
            source.run(0x7F1737, None, 0, OWNER, true);
            let mut objects = ObjectStore::new();
            let owner = actor(&mut objects);
            let other = actor(&mut objects);
            let mut runtime = PathRuntime::default();
            let mut world = ScenePathWorld::new(Default::default());
            if prefix_cost != 0 {
                source.run(0x7F194E, None, prefix_cost, OTHER, false);
                runtime
                    .resources
                    .allocate_owned(
                        other,
                        prefix_cost,
                        ProgramData::PathStack(Default::default()),
                    )
                    .unwrap();
            }
            source.run(0x068260, Some(0x0682B7), 0, OWNER, true);
            player_storage::replace(
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
            let slot = u32::from(source.bus.read16(u32::from(OWNER) + 0x2B));
            for index in 0..472 {
                source.bus.write8(
                    WRAM + slot + 0x6A61 + index,
                    seed.wrapping_add((index as u8).wrapping_mul(73)),
                );
            }
            for field in [0x6A98, 0x6BB8, 0x6BC8, 0x6BCA] {
                source
                    .bus
                    .write16(WRAM + slot + field, if seed & 1 == 0 { OTHER } else { 0 });
            }
            for (field, pointer, bank) in [
                (0x6A9A, 0x8048, 7),
                (0x6A9D, 0x9DF6, 7),
                (0x6C13, if seed & 1 == 0 { 0xBDDA } else { 0xBF63 }, 0x0D),
            ] {
                source.bus.write16(WRAM + slot + field, pointer);
                source.bus.write8(WRAM + slot + field + 2, bank);
            }
            let reader = Reader {
                source: &source,
                slot,
                other,
            };
            *player_storage::get_mut(&objects, &mut runtime.resources, owner).unwrap() =
                reader.storage();
            *world.player_mut(&objects, owner).unwrap() = reader.records();
            world
                .bind_shots(
                    &objects,
                    owner,
                    sf2_game::path_shots::ActiveShots::from_count(reader.byte(0x6C03)),
                )
                .unwrap();
            let original: Vec<_> = (0..472)
                .map(|index| source.bus.read8(WRAM + slot + 0x6A61 + index))
                .collect();
            let objects_before = objects.clone();
            let available = runtime.resources.available_capacity();
            let resource_count = runtime.resources.owner_count(owner);
            for repeat in 0..2 {
                // The original caller performs the near call; its next
                // equipment-restore helper is outside this exact boundary.
                source.run(0x06DEBD, Some(0x06DEC2), 0, OWNER, true);
                sf2_game::player_motion_reset::clear(
                    player_storage::get_mut(&objects, &mut runtime.resources, owner).unwrap(),
                    world.player_mut(&objects, owner).unwrap(),
                )
                .unwrap();
                let reader = Reader {
                    source: &source,
                    slot,
                    other,
                };
                assert_eq!(
                    world.player(&objects, owner).unwrap(),
                    &reader.records(),
                    "cost={prefix_cost} seed={seed} repeat={repeat}"
                );
                assert_eq!(
                    player_storage::get(&objects, &runtime.resources, owner).unwrap(),
                    &reader.storage()
                );
                assert_eq!(objects, objects_before);
                assert_eq!(runtime.resources.available_capacity(), available);
                assert_eq!(runtime.resources.owner_count(owner), resource_count);
                assert_eq!(
                    world.shots(&objects, owner).unwrap().count(),
                    reader.byte(0x6C03)
                );
                for (index, &prior) in original.iter().enumerate() {
                    assert_eq!(
                        source.bus.read8(WRAM + slot + 0x6A61 + index as u32),
                        if index < 391 { 0 } else { prior }
                    );
                }
            }

            // Exercise the complete caller too, starting from dirty player
            // records again. It adds ordered ambient/equipment/actor writes.
            for (index, &value) in original.iter().enumerate() {
                source
                    .bus
                    .write8(WRAM + slot + 0x6A61 + index as u32, value);
            }
            let reader = Reader {
                source: &source,
                slot,
                other,
            };
            *player_storage::get_mut(&objects, &mut runtime.resources, owner).unwrap() =
                reader.storage();
            *world.player_mut(&objects, owner).unwrap() = reader.records();
            let ambient = u16::from_le_bytes([seed, seed ^ 0xA7]);
            world.render_environment.ambient_control =
                Some(sf2_game::player_surface_render::AmbientParticleControl::from_bits(ambient));
            world.render_environment.plane_height = Some(-17391);
            source.bus.write16(0x7001BC, ambient);
            world.scene.active_weapon_level = Some(seed ^ 0x71);
            world.active_consumables = Some(sf2_game::player_visit::PublishedConsumables {
                packed_count: seed.wrapping_mul(79),
                kind: seed.rotate_left(3),
            });
            source.bus.write8(0x1DD4, seed ^ 0x71);
            source.bus.write8(0x1DD3, seed.rotate_left(3));
            source.bus.write8(0x1DD2, seed.wrapping_mul(79));
            world.surface_mode = Some(sf2_game::collision_surface::SurfaceMode { flags: seed });
            source.bus.write16(0x1B4D, 0xA500 | u16::from(seed));
            for field in [0x1E36, 0x1E38, 0x1E3A] {
                source.bus.write16(field, ambient);
            }
            world.player_pitch_target = Some(ambient);
            world.player_yaw_increment = Some(ambient);
            world.player_roll_increment = Some(ambient);
            let actor = objects.get_mut(owner).unwrap();
            actor.extension.path_state.motion_delta = Vector3 {
                x: -1291,
                y: 30000,
                z: -32768,
            };
            actor.extension.path_state.platform_carry.saved_position = Vector3 {
                x: 317,
                y: -713,
                z: 971,
            };
            actor.extension.path_state.motion.carry_selected_player = true;
            for (field, value) in [
                (0x1CC1, -1291_i16),
                (0x1CC3, 30000),
                (0x1CC5, -32768),
                (0x39, 317),
                (0x3B, -713),
                (0x3D, 971),
            ] {
                source
                    .bus
                    .write16(WRAM + u32::from(OWNER) + field, value as u16);
            }
            let old_flags = source.bus.read8(u32::from(OWNER) + 0x21) | 0x20;
            source.bus.write8(u32::from(OWNER) + 0x21, old_flags);
            let mut expected_objects = objects.clone();
            let expected_actor = expected_objects.get_mut(owner).unwrap();
            expected_actor.extension.path_state.motion_delta = Default::default();
            expected_actor
                .extension
                .path_state
                .platform_carry
                .saved_position = Default::default();
            expected_actor
                .extension
                .path_state
                .motion
                .carry_selected_player = false;
            for _ in 0..2 {
                source.run(0x068410, Some(0x068413), 0, OWNER, true);
                sf2_game::player_motion_reset::prepare_scene_entry(
                    &mut objects,
                    &mut world,
                    &mut runtime.resources,
                    owner,
                )
                .unwrap();
                let reader = Reader {
                    source: &source,
                    slot,
                    other,
                };
                assert_eq!(world.player(&objects, owner).unwrap(), &reader.records());
                assert_eq!(
                    player_storage::get(&objects, &runtime.resources, owner).unwrap(),
                    &reader.storage()
                );
                assert_eq!(objects, expected_objects);
                assert_eq!(
                    world.render_environment.ambient_control.unwrap().bits(),
                    source.bus.read16(0x7001BC)
                );
                assert_eq!(source.bus.read16(0x7001BC), ambient & 0xFF00);
                assert_eq!(world.render_environment.plane_height, Some(-17391));
                for (field, value) in [
                    (0x1E36, world.player_pitch_target),
                    (0x1E38, world.player_yaw_increment),
                    (0x1E3A, world.player_roll_increment),
                ] {
                    assert_eq!(value, Some(source.bus.read16(field)));
                }
                assert_eq!(
                    source.bus.read16(0x1B4D),
                    0xA500 | u16::from(world.surface_mode.unwrap().flags)
                );
                assert_eq!(source.bus.read8(u32::from(OWNER) + 0x21), old_flags & !0x20);
                for field in [0x1CC1, 0x1CC3, 0x1CC5, 0x39, 0x3B, 0x3D] {
                    assert_eq!(source.bus.read16(WRAM + u32::from(OWNER) + field), 0);
                }
                assert_eq!(runtime.resources.available_capacity(), available);
                assert_eq!(runtime.resources.owner_count(owner), resource_count);
                assert_eq!(
                    world.shots(&objects, owner).unwrap().count(),
                    reader.byte(0x6C03)
                );
            }
        }
    }
}
