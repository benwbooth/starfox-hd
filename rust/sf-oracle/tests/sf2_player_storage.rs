//! Unmodified player input preparation, auxiliary allocation/clear/publication
//! prefix and target initialization/selection. Storage stops before the distinct
//! view-selection tail ($06:82B7); targeting runs its separate full routines.

#[path = "support/sf2_player_input.rs"]
mod input_tests;

#[path = "support/sf2_player_action.rs"]
mod action_tests;

#[path = "support/sf2_player_palette.rs"]
mod palette_tests;

#[path = "support/sf2_player_mission.rs"]
mod mission_tests;
#[path = "support/sf2_player_node_exit.rs"]
mod node_exit_admission_tests;

#[path = "support/sf2_scene_clear.rs"]
mod scene_clear_tests;

#[path = "support/sf2_scene_world_reset.rs"]
mod scene_world_reset_tests;

#[path = "support/sf2_player_motion_reset.rs"]
mod motion_reset_tests;

#[path = "support/sf2_player_entry_reset.rs"]
mod entry_reset_tests;

#[path = "support/sf2_player_format.rs"]
mod format_tests;

#[path = "support/sf2_player_protection.rs"]
mod protection_tests;

#[path = "support/sf2_player_target_lock.rs"]
mod target_lock_tests;

#[path = "support/sf2_player_reticle.rs"]
mod reticle_tests;

#[path = "support/sf2_player_reticle_producer.rs"]
mod reticle_producer_tests;

#[path = "support/sf2_player_scene_reset.rs"]
mod scene_reset_tests;

#[path = "support/sf2_player_roll.rs"]
mod roll_tests;

#[path = "support/sf2_player_pose.rs"]
mod pose_tests;

#[path = "support/sf2_player_steering.rs"]
mod steering_tests;

#[path = "support/sf2_player_vertical.rs"]
mod vertical_tests;

#[path = "support/sf2_player_throttle.rs"]
mod throttle_tests;

#[path = "support/sf2_player_pool_failure.rs"]
mod pool_failure_tests;

#[path = "support/sf2_player_ambient.rs"]
mod ambient_tests;

#[path = "support/sf2_player_surface_particle.rs"]
mod surface_particle_tests;

#[path = "support/sf2_player_surface_splash.rs"]
mod surface_splash_tests;

#[path = "support/sf2_player_surface_render.rs"]
mod surface_render_tests;

#[path = "support/sf2_player_appearance.rs"]
mod appearance_tests;

#[path = "support/sf2_player_damage_effects.rs"]
mod damage_effects_tests;

#[path = "support/sf2_player_frame_effects.rs"]
mod frame_effects_tests;

#[path = "support/sf2_player_post_motion.rs"]
mod post_motion_tests;
#[path = "support/sf2_player_action_wait.rs"]
mod action_wait_tests;
#[path = "support/sf2_player_scene_actions.rs"]
mod scene_action_tests;

#[path = "support/sf2_scene_nine.rs"]
mod scene_nine_tests;
#[path = "support/sf2_scene_three.rs"]
mod scene_three_tests;

#[path = "support/sf2_scene_five.rs"]
mod scene_five_tests;
#[path = "support/sf2_scene_six.rs"]
mod scene_six_tests;
#[path = "support/sf2_attract_scene.rs"]
mod attract_scene_tests;
#[path = "support/sf2_star_wolf_scene.rs"]
mod star_wolf_scene_tests;

#[path = "support/sf2_scene_twentyfive.rs"]
mod scene_twentyfive_tests;

#[path = "support/sf2_scene_four.rs"]
mod scene_four_tests;

#[path = "support/sf2_map_streaming.rs"]
mod map_streaming_tests;

#[path = "support/sf2_scene_install.rs"]
mod scene_install_tests;

#[path = "support/sf2_attachment_pose.rs"]
mod attachment_poses;

#[path = "support/sf2_player_attachments.rs"]
mod attachment_tests;

#[path = "support/sf2_player_surface.rs"]
mod surface_tests;

#[path = "support/sf2_player_speed.rs"]
mod speed_tests;

#[path = "support/sf2_player_impact.rs"]
mod impact_tests;

#[path = "support/sf2_player_boundary.rs"]
mod boundary_tests;

#[path = "support/sf2_corridor_exit.rs"]
mod corridor_exit_tests;

#[path = "support/sf2_exit_view.rs"]
mod exit_view_tests;

#[path = "support/sf2_exit_shield.rs"]
mod exit_shield_tests;

#[path = "support/sf2_special_exit.rs"]
mod special_exit_tests;

#[path = "support/sf2_ordinary_exit.rs"]
mod ordinary_exit_tests;

#[path = "support/sf2_ordinary_exit_geometry.rs"]
mod ordinary_exit_geometry_tests;

#[path = "support/sf2_ordinary_exit_scratch.rs"]
mod ordinary_exit_scratch_tests;

#[path = "support/sf2_attachment_lifecycle.rs"]
mod attachment_lifecycle_tests;

#[path = "support/sf2_player_occupancy.rs"]
mod occupancy_tests;

#[path = "support/sf2_player_flight.rs"]
mod flight_tests;

#[path = "support/sf2_player_surface_prepare.rs"]
mod surface_prepare_tests;

#[path = "support/sf2_player_mode_selection.rs"]
mod mode_selection_tests;

#[path = "support/sf2_player_surface_effect.rs"]
mod surface_effect_tests;

#[path = "support/sf2_view_blend.rs"]
mod view_blend_tests;

#[path = "support/sf2_player_view_distance.rs"]
mod view_distance_tests;

#[path = "support/sf2_player_status.rs"]
mod status_tests;

#[path = "support/sf2_player_engine_sound.rs"]
mod engine_sound_tests;

use sf2_game::path_runtime::PathRuntime;
use sf2_game::path_target::TargetSelection;
use sf2_game::player_storage::{self, PlayerScore, PlayerStorage, PlayerStorageInputs};
use sf2_game::program_resources::AllocationFailure;
use sf2_game::program_state::ProgramData;
use sf2_game::scene_path_world::ScenePathWorld;
use sf2_game::view_transition::FixedViewAngles;
use sf2_game::{
    Angle, Behavior, Object, ObjectId, ObjectKind, ObjectStore, RandomState, ShapeId, Vector3,
};
use sf_oracle::SnesBus;
use w65c816::{AddressType, Signals, System, CPU};

const WRAM: u32 = 0x7E0000;
const OWNER: u16 = 0x0500;
const OTHER: u16 = 0x0600;

struct Source {
    bus: SnesBus,
    reset: bool,
    writes: Option<Vec<(u32, u8)>>,
    /// Valid data accesses only. Discarded indexed-address bus cycles do not
    /// constitute a live read of a scratch value by the original program.
    byte_accesses: Option<(u16, Vec<(bool, u8)>)>,
    last_carry: bool,
}

impl System for Source {
    fn read(&mut self, address: u32, kind: AddressType, signals: &Signals) -> u8 {
        let value = self.bus.read(address, kind, signals);
        if let Some((watched, accesses)) = &mut self.byte_accesses {
            if kind != AddressType::Invalid && (address == u32::from(*watched) || address == WRAM + u32::from(*watched)) {
                accesses.push((false, value));
            }
        }
        value
    }
    fn write(&mut self, address: u32, value: u8, kind: AddressType, signals: &Signals) {
        if let Some(writes) = &mut self.writes {
            writes.push((address, value));
        }
        if let Some((watched, accesses)) = &mut self.byte_accesses {
            if address == u32::from(*watched) || address == WRAM + u32::from(*watched) {
                accesses.push((true, value));
            }
        }
        self.bus.write(address, value, kind, signals);
    }
    fn res(&mut self) -> bool {
        std::mem::take(&mut self.reset)
    }
}

impl Source {
    fn new(rom: &[u8], fill: u8) -> Self {
        let mut bus = SnesBus::new(rom.to_vec());
        for (index, &byte) in rom[0x10000..0x17E00].iter().enumerate() {
            bus.write8(0x7F0000 + index as u32, byte);
        }
        for address in 0x6A61..0xB261 {
            bus.write8(WRAM + address, fill);
        }
        Self {
            bus,
            reset: false,
            writes: None,
            byte_accesses: None,
            last_carry: false,
        }
    }

    fn run(&mut self, entry: u32, stop: Option<u32>, a: u16, owner: u16, byte_a: bool) -> u16 {
        self.run_with_y(entry, stop, a, owner, byte_a, None)
    }

    fn run_with_y(
        &mut self,
        entry: u32,
        stop: Option<u32>,
        a: u16,
        owner: u16,
        byte_a: bool,
        y: Option<u16>,
    ) -> u16 {
        if let Some(writes) = &mut self.writes {
            writes.clear();
        }
        // Only the bootstrap is synthetic. Original allocation, coalescing,
        // zeroing, input reads and publications all execute unchanged.
        let mut boot = vec![
            0x18,
            0xFB,
            0xC2,
            0x30,
            0xE2,
            0x20,
            0xA9,
            0x7E,
            0x48,
            0xAB,
            0xC2,
            0x20,
            0xA9,
            a as u8,
            (a >> 8) as u8,
            0xA2,
            owner as u8,
            (owner >> 8) as u8,
        ];
        if byte_a {
            boot.extend_from_slice(&[0xE2, 0x20]);
        }
        if let Some(y) = y {
            boot.extend_from_slice(&[0xA0, y as u8, (y >> 8) as u8]);
        }
        boot.extend_from_slice(&[
            0x22,
            entry as u8,
            (entry >> 8) as u8,
            (entry >> 16) as u8,
            0xDB,
        ]);
        let boundary = stop.unwrap_or(0x200 + boot.len() as u32 - 1);
        for (offset, byte) in boot.into_iter().enumerate() {
            self.bus.write8(0x200 + offset as u32, byte);
        }
        self.reset = true;
        let mut cpu = CPU::new();
        for _ in 0..200_000 {
            cpu.cycle(self);
            if cpu.tcu() == 0
                && (u32::from(cpu.pbr()) << 16 | u32::from(cpu.pc().wrapping_sub(1))) == boundary
            {
                self.last_carry = cpu.p() & 1 != 0;
                return cpu.c();
            }
        }
        panic!(
            "source {entry:06X} did not reach {boundary:06X}: {:02X}:{:04X}",
            cpu.pbr(),
            cpu.pc()
        );
    }

    fn available(&self) -> u16 {
        let mut cursor = self.bus.read16(WRAM + 0x6A61);
        let mut total = 0;
        let mut count = 0;
        while cursor != 0 {
            total += self.bus.read16(WRAM + 0x6A65 + u32::from(cursor));
            cursor = self.bus.read16(WRAM + 0x6A61 + u32::from(cursor));
            count += 1;
            assert!(count < 100, "source resource list must terminate");
        }
        total
    }
}

fn actor(objects: &mut ObjectStore) -> ObjectId {
    objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap()
}

fn rom() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 ROM required")
}

#[test]
fn replacement_matches_original_zeroing_inputs_publication_and_shared_allocation_pressure() {
    let rom = rom();
    // Empty, old small allocation, fragmented ownership, and a full-sized
    // player block later replaced in place all use the original allocator.
    let allocations: &[&[(bool, u16)]] = &[
        &[],
        &[(true, 5)],
        &[
            (true, 5),
            (false, 39),
            (true, 472),
            (false, 127),
            (true, 69),
        ],
        &[(false, 100), (true, 472), (false, 250)],
    ];
    for pattern in allocations {
        for seed in 0..=u8::MAX {
            let mut source = Source::new(&rom, seed ^ 0xA7);
            source.run(0x7F1737, None, 0, OWNER, true);
            let mut objects = ObjectStore::new();
            let owner = actor(&mut objects);
            let other = actor(&mut objects);
            let mut world = ScenePathWorld::new(RandomState::new([17, 23, 49, 81]));
            let mut runtime = PathRuntime::default();
            for &(is_owner, cost) in *pattern {
                source.run(
                    0x7F194E,
                    None,
                    cost,
                    if is_owner { OWNER } else { OTHER },
                    false,
                );
                runtime
                    .resources
                    .allocate_owned(
                        if is_owner { owner } else { other },
                        cost,
                        ProgramData::PathStack(Default::default()),
                    )
                    .unwrap();
                assert_eq!(runtime.resources.available_capacity(), source.available());
            }
            let inputs = PlayerStorageInputs {
                pilot_code: seed,
                reserve_shield: seed.rotate_left(1),
                score: PlayerScore::from_parts(u16::from(seed) * 257, seed ^ 0xD9),
            };
            let rotation = [seed.wrapping_add(19), seed.rotate_left(3), !seed];
            for (field, value) in [0x12, 0x14, 0x16].into_iter().zip(rotation) {
                source.bus.write8(WRAM + u32::from(OWNER) + field, value);
            }
            let native = objects.get_mut(owner).unwrap();
            native.base.pitch = Angle::from_units(rotation[0]);
            native.base.yaw = Angle::from_units(rotation[1]);
            native.base.roll = Angle::from_units(rotation[2]);
            native.base.hit_points = seed;
            native.extension.spawn_group = seed;
            source.bus.write8(WRAM + u32::from(OWNER) + 0x2D, seed);
            source.bus.write8(WRAM + u32::from(OWNER) + 0x1CF0, seed);
            source.bus.write8(WRAM + 0x1E14, inputs.pilot_code);
            source.bus.write8(WRAM + 0x1DD1, inputs.reserve_shield);
            for (index, byte) in inputs.score.points().to_le_bytes()[..3].iter().enumerate() {
                source.bus.write8(WRAM + 0xD816 + index as u32, *byte);
            }
            source.bus.write16(WRAM + 0x12C3, OTHER);
            world.primary_player = Some(other);
            for repeat in 0..3 {
                source.run(0x068260, Some(0x0682B7), 0, OWNER, true);
                player_storage::replace(&mut objects, &mut world, &mut runtime, owner, inputs)
                    .unwrap();
                let slot = u32::from(source.bus.read16(WRAM + u32::from(OWNER) + 0x2B));
                assert_ne!(slot, 0);
                assert_source_target(
                    &source,
                    slot,
                    world
                        .player(&objects, owner)
                        .unwrap()
                        .target_selection
                        .unwrap(),
                    |_| panic!("new player target must not retain an actor"),
                    "storage replacement",
                );
                assert_eq!(source.bus.read16(WRAM + 0x12C3), OWNER);
                assert_eq!(source.bus.read16(WRAM + 0x1E24), slot as u16);
                assert_eq!(world.primary_player, Some(owner));
                let records = world.player(&objects, owner).unwrap();
                let lock = records.target_lock.unwrap();
                assert_eq!(lock.previous_candidate, None);
                assert_eq!(source.bus.read16(WRAM + slot + 0x6BC8), 0);
                assert_eq!(
                    lock.acquisition_clock,
                    source.bus.read8(WRAM + slot + 0x6BC6)
                );
                assert_eq!(lock.grace_remaining, source.bus.read8(WRAM + slot + 0x6BC7));
                assert_eq!(lock.marker_style, source.bus.read8(WRAM + slot + 0x6BB7));
                assert_eq!(
                    records.injected_input,
                    Some(sf2_game::InputState::default())
                );
                assert_eq!(
                    records.visit.unwrap().pilot_code,
                    source.bus.read8(WRAM + slot + 0x6BFF)
                );
                assert_eq!(
                    records.contact.unwrap().hit.reserve_shield,
                    source.bus.read8(WRAM + slot + 0x6C00)
                );
                assert_eq!(
                    records.score.unwrap().points(),
                    u32::from(source.bus.read16(WRAM + slot + 0x6C33))
                        | (u32::from(source.bus.read8(WRAM + slot + 0x6C35)) << 16)
                );
                let byte = |field| source.bus.read8(WRAM + slot + field);
                assert_eq!(records.palette_effects.unwrap().bits(), byte(0x6BE9) & 0xE0);
                assert_eq!(records.appearance.unwrap().depth_control, byte(0x6AA2));
                let word = |field| source.bus.read16(WRAM + slot + field);
                let charge = records.charge.unwrap();
                assert_eq!(charge.progress, word(0x6C07));
                assert_eq!(charge.control, byte(0x6C09));
                assert_eq!(charge.rapid_control, byte(0x6B60));
                assert_eq!(charge.speed_impulse, word(0x6B56) as i16);
                assert_eq!(charge.speed_impulse_ticks, byte(0x6B58));
                assert_eq!(charge.linked_mode, byte(0x6B63) & 0x80 != 0);
                assert_eq!(charge.linked_muzzle_disabled, byte(0x6B63) & 0x40 != 0);
                let equipment = records.equipment.unwrap();
                assert_eq!(equipment.packed_consumables, byte(0x6C04));
                assert_eq!(equipment.consumable_type, byte(0x6C05));
                assert_eq!(equipment.weapon_level, byte(0x6C06));
                assert_eq!(records.protection.unwrap().control(), byte(0x6C02));
                assert_eq!(world.shots(&objects, owner).unwrap().count(), byte(0x6C03));
                let action = records.action.unwrap();
                assert_eq!(action.action.is_none(), word(0x6C13) == 0);
                assert_eq!(action.elapsed, word(0x6C16));
                assert_eq!(action.auxiliary_counter, word(0x6C18));
                assert_eq!(action.total_updates, word(0x6C1A));
                let target = records.target_control.unwrap();
                assert_eq!(target.mode, word(0x6C1C));
                assert_eq!(target.transition_delay, byte(0x6BEA));
                assert_eq!(target.owner.is_none(), word(0x6A98) == 0);
                assert_eq!(target.configuration_locked, byte(0x6A8C) & 0x80 != 0);
                assert_eq!(target.offset_enabled, byte(0x6A8C) & 0x40 != 0);
                assert_eq!(target.range, word(0x6A90) as i16);
                assert_eq!(target.positive_range, word(0x6C26));
                assert_eq!(target.limit, word(0x6C24));
                assert_eq!(target.axis_mode, byte(0x6C29));
                assert_eq!(target.control, byte(0x6C28));
                assert_eq!(
                    target.axis_rates,
                    [byte(0x6A8D), byte(0x6A8E), byte(0x6A8F)]
                );
                assert_eq!(
                    target.axis_limits,
                    [byte(0x6C2A), byte(0x6C2B), byte(0x6C2C)]
                );
                assert_eq!(records.visit.unwrap().shield_warning_clock, byte(0x6BE8));
                assert_eq!(records.mission.unwrap().flags, byte(0x6A71));
                assert_eq!(records.particles.unwrap().flags, byte(0x6BE4));
                assert_eq!(records.particles.unwrap().age, byte(0x6BE5));
                assert_eq!(records.yaw_motion.unwrap(), word(0x6ACD));
                assert_eq!(records.consumable.unwrap().input_control, byte(0x6B61));
                assert_eq!(records.rapid_aim.unwrap().roll_step.units(), byte(0x6ADD));
                assert_eq!(records.rapid_rejection_consumes_queue, Some(slot & 255 != 0));
                let ambient = records.ambient.unwrap();
                assert_eq!(ambient.bank_phase, byte(0x6AD6));
                assert_eq!(ambient.offset_phase, byte(0x6ADB));
                assert_eq!(ambient.retained_offset, word(0x6AE2) as i16);
                assert_eq!(records.flight_displacement.unwrap(), Vector3 {
                    x: word(0x6B0B) as i16,
                    y: word(0x6B0D) as i16,
                    z: word(0x6B0F) as i16,
                });
                let surface = records.surface.unwrap();
                let motion = records.motion.unwrap();
                assert_eq!(motion.walker_attachment_yaw, byte(0x6AEE));
                assert_eq!(motion.lateral_impulse, byte(0x6AAD) as i8);
                assert_eq!(motion.previous_position, Vector3 {
                    x: word(0x6AC7) as i16,
                    y: word(0x6AC9) as i16,
                    z: word(0x6ACB) as i16,
                });
                assert_eq!(motion.surface_velocity, [word(0x6B11) as i16, word(0x6B13) as i16]);
                assert_eq!(motion.contact_flags, byte(0x6BE6));
                let boundary = records.boundary.unwrap();
                let occupancy = records.occupancy.unwrap();
                assert_eq!(occupancy.current_cell, [byte(0x6B29) as i8, byte(0x6B2A) as i8]);
                assert_eq!(occupancy.previous_cell, [byte(0x6B2C) as i8, byte(0x6B2D) as i8]);
                assert_eq!(occupancy.displacement, [word(0x6AF9) as i16, word(0x6AFB) as i16]);
                assert_eq!(boundary.center, Vector3 {
                    x: word(0x6AAF) as i16, y: word(0x6AB1) as i16, z: word(0x6AB3) as i16,
                });
                assert_eq!(boundary.half_width, word(0x6AB5) as i16);
                assert_eq!(boundary.half_height, word(0x6AB7) as i16);
                assert_eq!(boundary.return_position, Vector3 {
                    x: word(0x6BED) as i16, y: word(0x6BEF) as i16, z: word(0x6BF1) as i16,
                });
                assert_eq!(surface.plane_height, word(0x6A7D) as i16);
                assert_eq!(surface.material, byte(0x6A82));
                assert_eq!(
                    player_storage::get(&objects, &runtime.resources, owner).unwrap(),
                    &PlayerStorage {
                        fine_pitch: source.bus.read16(WRAM + slot + 0x6AB9),
                        fine_yaw: source.bus.read16(WRAM + slot + 0x6ABB),
                        bank: Angle::from_units(source.bus.read8(WRAM + slot + 0x6ABD)),
                        retained_shield: source.bus.read8(WRAM + slot + 0x6C38),
                    }
                );
                assert_eq!(
                    objects.get(owner).unwrap().base.hit_points,
                    source.bus.read8(WRAM + u32::from(OWNER) + 0x2D)
                );
                assert_eq!(
                    objects.get(owner).unwrap().extension.spawn_group,
                    source.bus.read8(WRAM + u32::from(OWNER) + 0x1CF0)
                );
                assert_eq!(runtime.resources.available_capacity(), source.available());
                let score = inputs.score.points().to_le_bytes();
                for offset in 0..472 {
                    let expected = match offset {
                        0x59 => rotation[0],
                        0x5B => rotation[1],
                        0x5C => rotation[2],
                        0x19E => inputs.pilot_code,
                        0x19F | 0x1D7 => inputs.reserve_shield,
                        0x1D2 => score[0],
                        0x1D3 => score[1],
                        0x1D4 => score[2],
                        _ => 0,
                    };
                    assert_eq!(
                        source.bus.read8(WRAM + slot + 0x6A61 + offset),
                        expected,
                        "seed={seed} pattern={pattern:?} repeat={repeat} offset={offset:X}"
                    );
                }
                // The next entry must clear an already used record, not just
                // encounter blank memory after a fresh allocation.
                for offset in 0..472 {
                    source
                        .bus
                        .write8(WRAM + slot + 0x6A61 + offset, seed ^ 0x5A);
                }
                world
                    .player_mut(&objects, owner)
                    .unwrap()
                    .charge
                    .as_mut()
                    .unwrap()
                    .progress = 65535;
                world
                    .player_mut(&objects, owner)
                    .unwrap()
                    .equipment
                    .as_mut()
                    .unwrap()
                    .weapon_level = 99;
                world.player_mut(&objects, owner).unwrap().target_selection =
                    Some(TargetSelection {
                        candidate: Some(other),
                        forced_owner: Some(other),
                        distance: u16::MAX,
                        control_flags: u8::MAX,
                        ..Default::default()
                    });
            }
        }
    }
}

#[test]
fn unavailable_storage_diagnoses_the_original_allocator_boundary_before_invalid_record_writes() {
    let rom = rom();
    for old_costs in [vec![], vec![5], vec![250, 250]] {
        let mut source = Source::new(&rom, 0xA7);
        source.run(0x7F1737, None, 0, OWNER, true);
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let other = actor(&mut objects);
        let mut world = ScenePathWorld::new(RandomState::default());
        world.primary_player = Some(other);
        let mut runtime = PathRuntime::default();
        for &cost in &old_costs {
            for (native_owner, source_owner, amount) in [(owner, OWNER, cost), (other, OTHER, 10)] {
                source.run(0x7F194E, None, amount, source_owner, false);
                runtime
                    .resources
                    .allocate_owned(
                        native_owner,
                        amount,
                        ProgramData::PathStack(Default::default()),
                    )
                    .unwrap();
            }
        }
        let remainder = runtime.resources.available_capacity();
        source.run(0x7F194E, None, remainder - 4, OTHER, false);
        runtime
            .resources
            .allocate_owned(
                other,
                remainder - 4,
                ProgramData::PathStack(Default::default()),
            )
            .unwrap();
        assert_eq!(source.available(), 0);
        let original_result = source.run(0x068260, Some(0x06826D), 0, OWNER, true);
        let native = player_storage::replace(
            &mut objects,
            &mut world,
            &mut runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 5,
                reserve_shield: 17,
                score: PlayerScore::from_parts(129, 53),
            },
        );
        let failure = if old_costs.is_empty() {
            // The original empty-list bug returns its request cost, then the
            // owned allocator adds its link displacement. It is NOT storage.
            assert_eq!(original_result, 478);
            AllocationFailure::EmptyFreeList { returned_cost: 476 }
        } else {
            assert_eq!(original_result, 0);
            AllocationFailure::NoContiguousFit
        };
        assert_eq!(
            native,
            Err(player_storage::PlayerStorageError::Allocation(failure))
        );
        assert_eq!(runtime.resources.available_capacity(), source.available());
        assert_eq!(runtime.resources.owner_count(owner), 0);
        assert_eq!(objects.get(owner).unwrap().base.player_storage, None);
        assert_eq!(world.primary_player, Some(other));
        assert!(world.player(&objects, owner).is_err());
    }
}

fn source_target(
    source: &mut Source,
    slot: u32,
    value: TargetSelection,
    pointer: impl Fn(ObjectId) -> u16,
) {
    for (field, value) in [
        (0x6BB8, value.candidate.map(&pointer).unwrap_or(0)),
        (0x6BBA, value.auxiliary_distance),
        (0x6BBC, value.distance),
        (0x6BBE, value.yaw),
        (0x6BC0, value.pitch),
        (0x6BCA, value.forced_owner.map(pointer).unwrap_or(0)),
        (0x6BCC, value.position.x as u16),
        (0x6BCE, value.position.y as u16),
        (0x6BD0, value.position.z as u16),
    ] {
        source.bus.write16(WRAM + slot + field, value);
    }
    for (field, value) in [
        (0x6BB6, value.display_status),
        (0x6BC2, value.control_flags),
        (0x6BC5, value.clipped_yaw),
        (0x6BAD, value.screen[0]),
        (0x6BAF, value.screen[1]),
    ] {
        source.bus.write8(WRAM + slot + field, value);
    }
}

fn assert_source_target(
    source: &Source,
    slot: u32,
    value: TargetSelection,
    pointer: impl Fn(ObjectId) -> u16,
    case: &str,
) {
    for (field, native) in [
        (0x6BB8, value.candidate.map(&pointer).unwrap_or(0)),
        (0x6BBA, value.auxiliary_distance),
        (0x6BBC, value.distance),
        (0x6BBE, value.yaw),
        (0x6BC0, value.pitch),
        (0x6BCA, value.forced_owner.map(pointer).unwrap_or(0)),
        (0x6BCC, value.position.x as u16),
        (0x6BCE, value.position.y as u16),
        (0x6BD0, value.position.z as u16),
    ] {
        assert_eq!(
            native,
            source.bus.read16(WRAM + slot + field),
            "{case}: {field:04X}"
        );
    }
    for (field, native) in [
        (0x6BB6, value.display_status),
        (0x6BC2, value.control_flags),
        (0x6BC5, value.clipped_yaw),
        (0x6BAD, value.screen[0]),
        (0x6BAF, value.screen[1]),
    ] {
        assert_eq!(
            native,
            source.bus.read8(WRAM + slot + field),
            "{case}: {field:04X}"
        );
    }
}

#[test]
fn target_initializer_matches_original_for_every_existing_control_and_shared_mode_byte() {
    let mut source = Source::new(&rom(), 0xA7);
    source.run(0x7F1737, None, 0, OWNER, true);
    source.run(0x068260, Some(0x0682B7), 0, OWNER, true);
    let slot = u32::from(source.bus.read16(WRAM + u32::from(OWNER) + 0x2B));
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let other = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    let mut runtime = PathRuntime::default();
    player_storage::replace(
        &mut objects,
        &mut world,
        &mut runtime,
        owner,
        PlayerStorageInputs {
            pilot_code: 0,
            reserve_shield: 0,
            score: PlayerScore::default(),
        },
    )
    .unwrap();
    // The reset is caller-owned, not implicitly the currently selected primary.
    world.primary_player = Some(other);
    source.bus.write16(WRAM + 0x12C3, OTHER);
    for flags in 0..=u8::MAX {
        for mode in 0..=u8::MAX {
            let initial = TargetSelection {
                display_status: !flags,
                control_flags: flags,
                forced_owner: Some(other),
                candidate: Some(other),
                distance: u16::from(mode) * 257,
                auxiliary_distance: 47131,
                position: Vector3 {
                    x: -30201,
                    y: 1779,
                    z: 28131,
                },
                pitch: 41709,
                yaw: 49159,
                screen: [213, 179],
                clipped_yaw: flags ^ mode,
            };
            source_target(&mut source, slot, initial, |_| OTHER);
            source.bus.write8(WRAM + 0x1AA6, mode);
            world.reflect_all_contacts = Some(mode & 2 != 0);
            world.player_mut(&objects, owner).unwrap().target_selection = Some(initial);
            source.run(0x07B0CE, None, 0, OWNER, true);
            sf2_game::path_target::initialize_player(&objects, &mut world, owner).unwrap();
            let result = world
                .player(&objects, owner)
                .unwrap()
                .target_selection
                .unwrap();
            assert_source_target(
                &source,
                slot,
                result,
                |_| OTHER,
                &format!("flags={flags} mode={mode}"),
            );
            assert_eq!(source.bus.read16(WRAM + 0x12C3), OTHER);
            assert_eq!(world.primary_player, Some(other));
        }
    }
}

#[test]
fn scene_primary_target_selection_matches_both_original_entries_with_live_view_and_player_switches()
{
    use sf2_game::path_commands::{ControlCommand, ControlStep};
    use sf2_game::path_control::PlayerTarget;
    use sf2_game::path_invocation::InvocationWorld;
    use sf2_game::path_program::{PathCatalog, Statement};
    use sf2_game::{PathCursor, PathId};
    const CANDIDATE: u16 = 0x0700;
    const VIEW: u32 = 0x033F;
    let mut source = Source::new(&rom(), 0xA7);
    source.run(0x7F1737, None, 0, OWNER, true);
    let mut slots = [0; 2];
    for (index, owner) in [OWNER, OTHER].into_iter().enumerate() {
        source.run(0x068260, Some(0x0682B7), 0, owner, true);
        slots[index] = u32::from(source.bus.read16(WRAM + u32::from(owner) + 0x2B));
    }
    let mut objects = ObjectStore::new();
    let players = [actor(&mut objects), actor(&mut objects)];
    let owner = actor(&mut objects);
    let view = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::new([1, 9, 17, 81]));
    let mut runtime = PathRuntime::default();
    for player in players {
        player_storage::replace(
            &mut objects,
            &mut world,
            &mut runtime,
            player,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 0,
                score: PlayerScore::default(),
            },
        )
        .unwrap();
    }
    world.fixed_players[0] = Some(view);
    let cursor = |path, command_index| PathCursor {
        path: PathId::from_catalog_index(path),
        command_index,
    };
    let catalog = PathCatalog::new(vec![
        vec![
            Statement::ConsiderPrimaryTarget { next: cursor(0, 1) },
            Statement::Control(ControlCommand::Hold),
        ],
        vec![
            Statement::ConsiderPrimaryTargetAndMarkSceneProxy { next: cursor(1, 1) },
            Statement::Control(ControlCommand::Hold),
        ],
    ])
    .unwrap();
    let pointer = |id| {
        if id == owner {
            CANDIDATE
        } else if id == players[0] {
            OWNER
        } else if id == players[1] {
            OTHER
        } else {
            panic!("unexpected target identity")
        }
    };
    for seed in 0..=u8::MAX {
        let selected_index = usize::from(seed & 1);
        let primary = players[selected_index];
        let secondary = players[1 - selected_index];
        let slot = slots[selected_index];
        world.primary_player = Some(primary);
        world.secondary_player = Some(secondary);
        source.bus.write16(WRAM + 0x12C3, pointer(primary));
        for (index, (anchor_position, candidate_position)) in [
            (Vector3::default(), Vector3 { x: 0, y: 0, z: 100 }),
            (
                Vector3 {
                    x: 100,
                    y: -101,
                    z: -999,
                },
                Vector3 {
                    x: -777,
                    y: 205,
                    z: 539,
                },
            ),
            (
                Vector3 {
                    x: -32768,
                    y: 32767,
                    z: 0,
                },
                Vector3 {
                    x: 32767,
                    y: -32768,
                    z: -32768,
                },
            ),
            (
                Vector3 {
                    x: -301,
                    y: -203,
                    z: -107,
                },
                Vector3 {
                    x: -300,
                    y: -202,
                    z: -106,
                },
            ),
            (
                Vector3 {
                    x: 31979,
                    y: 12917,
                    z: -25713,
                },
                Vector3 {
                    x: -27101,
                    y: -5371,
                    z: 17533,
                },
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let angles = FixedViewAngles {
                pitch: u16::from(seed).wrapping_mul(157),
                yaw: u16::from(seed).wrapping_mul(353),
                roll: 0,
            };
            angles.write_to(objects.get_mut(view).unwrap());
            objects.get_mut(view).unwrap().base.position = anchor_position;
            objects.get_mut(owner).unwrap().base.position = candidate_position;
            for (field, value) in [
                (0x0C, anchor_position.x as u16),
                (0x0E, anchor_position.y as u16),
                (0x10, anchor_position.z as u16),
                (0x12, angles.pitch),
                (0x14, angles.yaw),
            ] {
                source.bus.write16(WRAM + VIEW + field, value);
            }
            for (field, value) in [
                (0x0C, candidate_position.x),
                (0x0E, candidate_position.y),
                (0x10, candidate_position.z),
            ] {
                source
                    .bus
                    .write16(WRAM + u32::from(CANDIDATE) + field, value as u16);
            }
            for distance in [0, 56, 32768, 65535] {
                for (path, entry) in [(0, 0x07B1EA), (1, 0x07B1FD)] {
                    let initial = TargetSelection {
                        display_status: seed.rotate_left(3),
                        control_flags: seed,
                        forced_owner: match seed % 3 {
                            0 => Some(owner),
                            1 => Some(secondary),
                            _ => None,
                        },
                        candidate: Some(secondary),
                        distance,
                        auxiliary_distance: 49631,
                        position: Vector3 {
                            x: -17003,
                            y: 737,
                            z: -9701,
                        },
                        pitch: 61071,
                        yaw: 40839,
                        screen: [173, 237],
                        clipped_yaw: !seed,
                    };
                    source_target(&mut source, slot, initial, pointer);
                    world
                        .player_mut(&objects, primary)
                        .unwrap()
                        .target_selection = Some(initial);
                    let secondary_before = *world.player(&objects, secondary).unwrap();
                    objects.get_mut(owner).unwrap().base.path = Some(cursor(path, 0));
                    source.run(entry, None, 0, CANDIDATE, true);
                    let mut input = world
                        .path_world(&objects, owner, PlayerTarget::Secondary)
                        .unwrap();
                    let result = runtime
                        .step_program(&catalog, &mut objects, owner, &mut input)
                        .unwrap();
                    assert_eq!(result.step, ControlStep::Continue);
                    drop(input);
                    let result = world
                        .player(&objects, primary)
                        .unwrap()
                        .target_selection
                        .unwrap();
                    assert_source_target(
                        &source,
                        slot,
                        result,
                        pointer,
                        &format!("seed={seed} geometry={index} distance={distance} path={path}"),
                    );
                    assert_eq!(
                        world.player(&objects, secondary).unwrap(),
                        &secondary_before
                    );
                }
            }
        }
    }
    assert_eq!(world.random.bytes(), [1, 9, 17, 81]);
}
