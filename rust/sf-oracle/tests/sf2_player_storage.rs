//! Unmodified player auxiliary allocation/clear/publication prefix. This
//! deliberately stops at the distinct view-selection tail ($06:82B7).

use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_storage::{self, PlayerScore, PlayerStorage, PlayerStorageInputs};
use sf2_game::program_resources::AllocationFailure;
use sf2_game::program_state::ProgramData;
use sf2_game::scene_path_world::ScenePathWorld;
use sf2_game::{Angle, Behavior, Object, ObjectId, ObjectKind, ObjectStore, RandomState, ShapeId};
use sf_oracle::SnesBus;
use w65c816::{AddressType, Signals, System, CPU};

const WRAM: u32 = 0x7E0000;
const OWNER: u16 = 0x0500;
const OTHER: u16 = 0x0600;

struct Source {
    bus: SnesBus,
    reset: bool,
}

impl System for Source {
    fn read(&mut self, address: u32, kind: AddressType, signals: &Signals) -> u8 {
        self.bus.read(address, kind, signals)
    }
    fn write(&mut self, address: u32, value: u8, kind: AddressType, signals: &Signals) {
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
        Self { bus, reset: false }
    }

    fn run(&mut self, entry: u32, stop: Option<u32>, a: u16, owner: u16, byte_a: bool) -> u16 {
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
                assert_eq!(source.bus.read16(WRAM + 0x12C3), OWNER);
                assert_eq!(source.bus.read16(WRAM + 0x1E24), slot as u16);
                assert_eq!(world.primary_player, Some(owner));
                let records = world.player(&objects, owner).unwrap();
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
                assert_eq!(records.particles.unwrap().flags, byte(0x6BE4));
                assert_eq!(records.yaw_motion.unwrap(), word(0x6ACD));
                assert_eq!(records.consumable.unwrap().input_control, byte(0x6B61));
                assert_eq!(records.rapid_aim.unwrap().roll_step.units(), byte(0x6ADD));
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
