//! Exhaustion reaches the real, non-returning $00:8032 handler. Never patch
//! its call or execute the unreachable carry-clear return after it. Native
//! diagnostic errors preserve exactly the gameplay writes before that call.

use super::{rom, Source, WRAM};
use sf2_game::collision_contacts::ContactStore;
use sf2_game::path_program::{ActionGate, ProjectileTrigger, SelectedAuxiliaryState};
use sf2_game::path_protection::DeflectionProtection;
use sf2_game::path_relationships;
use sf2_game::path_shots::ActiveShots;
use sf2_game::player_action::PlayerActionState;
use sf2_game::player_charge::{self, ChargeError, PlayerCharge};
use sf2_game::player_consumable::{self, ConsumableError, PlayerConsumableControl};
use sf2_game::player_recovery::{self, RecoveryError};
use sf2_game::program_resources::ProgramResources;
use sf2_game::program_state::ProgramData;
use sf2_game::scene_contact::PlayerContactControl;
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::weapon_creation::CreationError;
use sf2_game::weapon_dispatch::{
    self, LaunchError, LaunchRequest, LaunchWorld, PathWeapon, WeaponState,
};
use sf2_game::weapon_launch::{LaunchParameters, MuzzleOffset};
use sf2_game::weapon_rapid::CallerWeaponInputs;
use sf2_game::weapon_reflection::{self, ReflectionError, ReflectionRules, ReflectionWorld};
use sf2_game::{
    Angle, Behavior, Buttons, InputState, Object, ObjectId, ObjectKind, ObjectSpawnDefaults,
    ObjectStore, RandomState, ShapeId, SoundEvent,
};

const OWNER: u16 = 0x03BD;
const SLOT: u16 = 64;
const FATAL: u32 = 0x008032;

fn address(id: ObjectId) -> u16 {
    OWNER + id.index() as u16 * 0x3F
}

struct FullPool {
    objects: ObjectStore,
    world: ScenePathWorld,
    resources: ProgramResources<ProgramData>,
    owner: ObjectId,
}

impl FullPool {
    fn new(source: &mut Source) -> Self {
        let mut objects = ObjectStore::new();
        for _ in 0..60 {
            objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap();
        }
        let owner = *objects.active_ids().last().unwrap();
        assert_eq!(address(owner), OWNER);
        for (id, actor) in objects.active_objects() {
            let base = u32::from(address(id));
            for offset in 0..0x3F {
                source.bus.write8(WRAM + base + offset, 0);
                source.bus.write8(WRAM + 0x1CC1 + base + offset, 0);
            }
            source.bus.write16(base, actor.base.next.map_or(0, address));
            source
                .bus
                .write16(base + 2, actor.base.previous.map_or(0, address));
            source.bus.write16(base + 4, 0xBC9C);
            source.bus.write8(base + 0x2D, actor.base.hit_points);
        }
        source.bus.write16(0x12A8, address(objects.active_ids()[0]));
        source.bus.write16(0x12AA, 0);
        source.bus.write16(0x12C3, OWNER);
        source.bus.write16(0x12C5, 0);
        source.bus.write16(u32::from(OWNER) + 0x2B, SLOT);
        for field in 0..472 {
            source
                .bus
                .write8(WRAM + 0x6A61 + u32::from(SLOT) + field, 0);
        }
        for field in [0x1B84, 0x1D16, 0x1936, 0x1938] {
            source.bus.write16(field, 0);
        }
        for field in [0x1D72, 0x1E1B, 0x1E59] {
            source.bus.write8(field, 0);
        }
        source.bus.write8(0x1DD5, 100);
        source.bus.write8(0x1DD6, 25);
        let random = RandomState::new([17, 63, 149, 211]);
        for (index, value) in random.bytes().into_iter().enumerate() {
            source.bus.write8(0xE0 + index as u32, value);
        }
        let mut world = ScenePathWorld::new(random);
        world.primary_player = Some(owner);
        world.spawn_defaults = Some(ObjectSpawnDefaults::default());
        world.action_gate = Some(ActionGate::default());
        world.active_charge_threshold = Some(25);
        world.active_shield_capacity = Some(100);
        world.shield_recovery = Some(Default::default());
        world.projectile_trigger = Some(ProjectileTrigger::default());
        world.weapons = Some(WeaponState::default());
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    charge: Some(PlayerCharge::default()),
                    contact: Some(PlayerContactControl::default()),
                    action: Some(PlayerActionState::default()),
                    consumable: Some(PlayerConsumableControl::default()),
                    equipment: Some(Default::default()),
                    protection: Some(DeflectionProtection::default()),
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 0,
                        action_flags: 0,
                        stored_world_position: Default::default(),
                        stored_rotation: Default::default(),
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        let mut pool = Self {
            objects,
            world,
            resources: Default::default(),
            owner,
        };
        pool.seed_parameters(source);
        pool
    }

    fn record(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }

    fn byte(source: &mut Source, field: u32, value: u8) {
        source.bus.write8(WRAM + u32::from(SLOT) + field, value);
    }
    fn word(source: &mut Source, field: u32, value: u16) {
        source.bus.write16(WRAM + u32::from(SLOT) + field, value);
    }

    fn seed_parameters(&mut self, source: &mut Source) {
        self.world.weapons.as_mut().unwrap().parameters = LaunchParameters {
            muzzle: MuzzleOffset {
                x: -17,
                y: 33,
                z: -99,
            },
            pitch_offset: -37,
            yaw_offset: 113,
            target: Some(Default::default()),
        };
        for (field, value) in [
            (0x14B0, 239),
            (0x14B2, 33),
            (0x14B4, 157),
            (0x14B6, 113),
            (0x14B7, 219),
        ] {
            source.bus.write8(field, value);
        }
        source.bus.write16(0x14B8, OWNER);
    }

    fn child(&mut self, source: &mut Source, number: u8) -> ObjectId {
        let child = self.objects.active_ids()[1];
        path_relationships::attach_fresh_child(&mut self.objects, self.owner, child, number)
            .unwrap();
        source.run_with_y(
            0x7F2A3D,
            None,
            u16::from(number),
            OWNER,
            true,
            Some(address(child)),
        );
        child
    }

    fn auxiliary(source: &Source) -> Vec<u8> {
        (0..472)
            .map(|i| source.bus.read8(WRAM + 0x6A61 + u32::from(SLOT) + i))
            .collect()
    }

    fn check(&mut self, source: &Source, mut auxiliary: Vec<u8>) {
        let record = *self.record();
        let charge = record.charge.unwrap();
        let equipment = record.equipment.unwrap();
        let mut put = |field: usize, bytes: &[u8]| {
            auxiliary[field - 0x6A61..field - 0x6A61 + bytes.len()].copy_from_slice(bytes);
        };
        put(0x6C07, &charge.progress.to_le_bytes());
        put(0x6C09, &[charge.control]);
        put(0x6B60, &[charge.rapid_control]);
        put(0x6B56, &charge.speed_impulse.to_le_bytes());
        put(0x6B58, &[charge.speed_impulse_ticks]);
        put(0x6C00, &[record.contact.unwrap().hit.reserve_shield]);
        put(0x6C04, &[equipment.packed_consumables]);
        put(0x6C02, &[record.protection.unwrap().control()]);
        let action = record.action.unwrap();
        if action.action.is_some() {
            put(0x6C13, &[0xDA, 0xBD, 0x0D]);
        }
        put(0x6C16, &action.elapsed.to_le_bytes());
        put(0x6C18, &action.auxiliary_counter.to_le_bytes());
        for (offset, (actual, expected)) in Self::auxiliary(source)
            .into_iter()
            .zip(auxiliary)
            .enumerate()
        {
            assert_eq!(actual, expected, "player field {:04X}", 0x6A61 + offset);
        }
        let parameters = self.world.weapons.unwrap().parameters;
        for (field, value) in [
            (0x14B0, parameters.muzzle.x),
            (0x14B2, parameters.muzzle.y),
            (0x14B4, parameters.muzzle.z),
            (0x14B6, parameters.yaw_offset),
            (0x14B7, parameters.pitch_offset),
        ] {
            assert_eq!(source.bus.read8(field), value as u8, "parameter {field:X}");
        }
        assert_eq!(
            source.bus.read16(0x14B8),
            if parameters.target.is_some() {
                OWNER
            } else {
                0
            }
        );
        assert_eq!(
            source.bus.read8(0x1E1B),
            self.world.shield_recovery.unwrap().amount
        );
        assert_eq!(
            source.bus.read8(0x1E59),
            self.world.projectile_trigger.unwrap().activation
        );
        let events: Vec<_> = self
            .world
            .audio
            .take_events()
            .into_iter()
            .flatten()
            .map(|event| {
                let SoundEvent::Authored(cue) = event else {
                    panic!("unexpected cue")
                };
                u16::from(cue.id) | u16::from(cue.parameter()) << 8
            })
            .collect();
        assert_eq!(source.bus.read16(0x1D16) as usize, events.len() * 2);
        for (index, value) in events.into_iter().enumerate() {
            assert_eq!(source.bus.read16(0x1CF6 + index as u32 * 2), value);
        }
        assert_eq!(
            self.world.random.bytes(),
            std::array::from_fn(|i| source.bus.read8(0xE0 + i as u32))
        );
        assert_eq!(self.objects.len(), 60);
        assert_eq!(source.bus.read16(0x12AA), 0);
        // The failed weapon allocator can leave its temporary source-list
        // head installed before the fatal display. Native linked-list identity
        // remains valid; no caller resumes, and actor links themselves match.
        for (id, actor) in self.objects.active_objects() {
            let base = u32::from(address(id));
            assert_eq!(source.bus.read16(base), actor.base.next.map_or(0, address));
            assert_eq!(
                source.bus.read16(base + 2),
                actor.base.previous.map_or(0, address)
            );
            assert_eq!(
                source.bus.read8(base + 0x21) & 1 != 0,
                actor.base.flags.collision_disabled
            );
            assert_eq!(
                source.bus.read8(base + 0x25) & 8 != 0,
                actor.base.flags.remove_after_tick
            );
            assert_eq!(source.bus.read8(base + 0x18), actor.base.speed);
            assert_eq!(source.bus.read16(base + 12) as i16, actor.base.position.x);
            assert_eq!(source.bus.read16(base + 14) as i16, actor.base.position.y);
            assert_eq!(source.bus.read16(base + 16) as i16, actor.base.position.z);
        }
    }
}

#[test]
fn full_pool_charge_and_release_match_original_fatal_prefix_or_existing_child_return() {
    let mut source = Source::new(&rom(), 0);
    let mut faults = 0;
    for flags in 0..=u8::MAX {
        for release in [false, true] {
            for existing in [false, true] {
                let mut native = FullPool::new(&mut source);
                if existing {
                    native.child(&mut source, 39);
                }
                let progress = (if release { 25 } else { 24 }) << 8 | u16::from(flags);
                let charge = native.record().charge.as_mut().unwrap();
                charge.progress = progress;
                charge.control = flags;
                charge.rapid_control = 0x71;
                charge.speed_impulse = -299;
                charge.speed_impulse_ticks = 231;
                FullPool::word(&mut source, 0x6C07, progress);
                FullPool::byte(&mut source, 0x6C09, flags);
                FullPool::byte(&mut source, 0x6B60, 0x71);
                FullPool::word(&mut source, 0x6B56, (-299_i16) as u16);
                FullPool::byte(&mut source, 0x6B58, 231);
                source.bus.write16(0x1938, if release { 0 } else { 0x8000 });
                let before = FullPool::auxiliary(&source);
                let fatal = release || (!existing && flags & 0x40 == 0);
                source.run(0x07DAB2, fatal.then_some(FATAL), 0, OWNER, true);
                let result = player_charge::advance(
                    &mut native.objects,
                    &mut native.world,
                    &mut native.resources,
                    native.owner,
                    InputState {
                        held: Buttons::from_bits(if release { 0 } else { 0x8000 }),
                        pressed: Buttons::default(),
                    },
                );
                assert_eq!(
                    result,
                    if fatal {
                        Err(ChargeError::ObjectPoolExhausted)
                    } else {
                        Ok(())
                    },
                    "flags {flags:02X}, release {release}, existing {existing}"
                );
                faults += usize::from(fatal);
                native.check(&source, before);
            }
        }
    }
    assert_eq!(faults, 640);
}

#[test]
fn all_consumable_types_and_count_bytes_match_original_full_pool_stop_or_rejection() {
    let mut source = Source::new(&rom(), 0);
    for packed in 0..=u8::MAX {
        for kind in 0..=u8::MAX {
            let mut native = FullPool::new(&mut source);
            let equipment = native.record().equipment.as_mut().unwrap();
            equipment.packed_consumables = packed;
            equipment.consumable_type = kind;
            native.world.projectile_trigger.as_mut().unwrap().activation = 231;
            source.bus.write8(0x1E59, 231);
            FullPool::byte(&mut source, 0x6C04, packed);
            FullPool::byte(&mut source, 0x6C05, kind);
            let before = FullPool::auxiliary(&source);
            let admitted = packed & 15 != 0;
            let fatal = admitted && kind & 127 < 2;
            source.run(0x07DC8B, fatal.then_some(FATAL), 0, OWNER, true);
            let result =
                player_consumable::use_item(&mut native.objects, &mut native.world, native.owner);
            assert_eq!(
                result,
                if fatal {
                    Err(ConsumableError::ObjectPoolExhausted)
                } else {
                    Ok(admitted)
                },
                "packed {packed:02X}, kind {kind:02X}"
            );
            native.check(&source, before);
        }
    }
}

#[test]
fn consumable_early_gates_and_existing_recovery_child_do_not_fault_even_with_a_full_pool() {
    use sf2_game::player_action::PlayerAction;
    use sf2_game::player_consumable::TriggeredUseBlockers;
    let mut source = Source::new(&rom(), 0);
    for kind in [0, 1] {
        for flags in 0..=u8::MAX {
            for gate in 0..4 {
                let mut native = FullPool::new(&mut source);
                let equipment = native.record().equipment.as_mut().unwrap();
                equipment.packed_consumables = 3;
                equipment.consumable_type = kind;
                FullPool::byte(&mut source, 0x6C04, 3);
                FullPool::byte(&mut source, 0x6C05, kind);
                if kind == 0 {
                    native
                        .record()
                        .consumable
                        .as_mut()
                        .unwrap()
                        .recovery_blocked = flags & 0x40 != 0;
                    FullPool::byte(&mut source, 0x6B7D, flags);
                } else {
                    native
                        .record()
                        .consumable
                        .as_mut()
                        .unwrap()
                        .projectile_blockers = TriggeredUseBlockers::from_control(flags);
                    FullPool::byte(&mut source, 0x6BE9, flags);
                }
                match gate {
                    0 => {}
                    1 => {
                        native
                            .world
                            .spawn_defaults
                            .as_mut()
                            .unwrap()
                            .run_when_paused = true;
                        source.bus.write16(0x1B84, 2);
                    }
                    2 if kind == 0 => {
                        native.record().contact.as_mut().unwrap().hit.reserve_shield = 100;
                        FullPool::byte(&mut source, 0x6C00, 100);
                    }
                    3 if kind == 0 => {
                        native.child(&mut source, 22);
                    }
                    _ => {
                        native.record().action.as_mut().unwrap().action =
                            Some(PlayerAction::TriggeredProjectile);
                        FullPool::word(&mut source, 0x6C13, 0xBDDA);
                        FullPool::byte(&mut source, 0x6C15, 0x0D);
                    }
                }
                let fatal = gate == 0
                    && if kind == 0 {
                        flags & 0x40 == 0
                    } else {
                        flags & 0x18 == 0
                    };
                let before = FullPool::auxiliary(&source);
                source.run(0x07DC8B, fatal.then_some(FATAL), 0, OWNER, true);
                let result = player_consumable::use_item(
                    &mut native.objects,
                    &mut native.world,
                    native.owner,
                );
                assert_eq!(
                    result,
                    if fatal {
                        Err(ConsumableError::ObjectPoolExhausted)
                    } else {
                        Ok(false)
                    }
                );
                native.check(&source, before);
            }
        }
    }
}

#[test]
fn full_pool_recovery_keeps_cleared_request_and_wrapped_shield_before_fatal_or_existing_child() {
    let mut source = Source::new(&rom(), 0);
    for amount in 0..=u8::MAX {
        for shield in [0, 1, 99, 100, 127, 128, 254, 255] {
            for existing in [false, true] {
                let mut native = FullPool::new(&mut source);
                if existing {
                    native.child(&mut source, 24);
                }
                native.record().contact.as_mut().unwrap().hit.reserve_shield = shield;
                native.world.shield_recovery.as_mut().unwrap().amount = amount;
                source.bus.write8(0x1E1B, amount);
                FullPool::byte(&mut source, 0x6C00, shield);
                let before = FullPool::auxiliary(&source);
                let fatal = amount != 0 && !existing;
                source.run_with_y(
                    0x069F36,
                    Some(if fatal { FATAL } else { 0x069F54 }),
                    0,
                    OWNER,
                    true,
                    Some(SLOT),
                );
                let result =
                    player_recovery::consume(&mut native.objects, &mut native.world, native.owner);
                assert_eq!(
                    result,
                    if fatal {
                        Err(RecoveryError::ObjectPoolExhausted)
                    } else {
                        Ok(amount != 0)
                    }
                );
                native.check(&source, before);
            }
        }
    }
}

#[test]
fn every_path_weapon_selector_preserves_full_pool_rapid_admission_before_fatal_allocation() {
    let mut source = Source::new(&rom(), 0);
    for selection in (2..=32).step_by(2) {
        for count in 0..=u8::MAX {
            let mut native = FullPool::new(&mut source);
            let level = count.rotate_left(3);
            FullPool::byte(&mut source, 0x6C03, count);
            FullPool::byte(&mut source, 0x6C06, level);
            let before = FullPool::auxiliary(&source);
            let rapid = [4, 6, 8, 10].contains(&selection);
            let admitted =
                !rapid || ((count < 8 || count >= 136) && (selection != 4 || level & 3 != 0));
            source.run(
                0x03A89C,
                admitted.then_some(FATAL),
                u16::from(selection),
                OWNER,
                true,
            );
            let mut world = LaunchWorld {
                caller_inputs: Some(CallerWeaponInputs {
                    owner: native.owner,
                    active_shots: Some(ActiveShots::from_count(count)),
                    weapon_level: Some(level),
                    roll_step: None,
                    retained_aim: None,
                }),
                fallback: None,
                published_pitch: None,
                primary: None,
                secondary: None,
                primary_auxiliary_mode: None,
                hostile_counts: None,
                random: &mut native.world.random,
            };
            let result = weapon_dispatch::launch(
                &mut native.objects,
                &mut native.resources,
                native.owner,
                LaunchRequest {
                    weapon: PathWeapon::from_selection(selection).unwrap(),
                    parameters: native.world.weapons.unwrap().parameters,
                    defaults: Default::default(),
                },
                &mut world,
            );
            assert_eq!(
                result,
                if admitted {
                    Err(LaunchError::Creation(CreationError::ObjectPoolExhausted))
                } else {
                    Ok(None)
                },
                "selector {selection}, count {count}, level {level}"
            );
            native.check(&source, before);
        }
    }
}

#[test]
fn full_pool_reflection_preserves_original_disable_scatter_and_parameters_without_fallback_edits() {
    let mut source = Source::new(&rom(), 0);
    for seed in 0..=u8::MAX {
        for player in [false, true] {
            for scatter in [false, true] {
                let mut native = FullPool::new(&mut source);
                let incoming = native.objects.active_ids()[1];
                native
                    .objects
                    .get_mut(native.owner)
                    .unwrap()
                    .base
                    .contacts
                    .skip_contacts = true;
                native.objects.get_mut(native.owner).unwrap().base.yaw = Angle::from_units(seed);
                let shot = native.objects.get_mut(incoming).unwrap();
                shot.base.contacts.credits_hit_side = true;
                shot.base.pitch = Angle::from_units(seed.rotate_left(3));
                shot.base.yaw = Angle::from_units(!seed);
                source.bus.write8(u32::from(OWNER) + 0x25, 0x10);
                source.bus.write8(u32::from(OWNER) + 0x14, seed);
                source.bus.write8(u32::from(address(incoming)) + 0x31, 8);
                source
                    .bus
                    .write8(u32::from(address(incoming)) + 0x12, seed.rotate_left(3));
                source
                    .bus
                    .write8(u32::from(address(incoming)) + 0x14, !seed);
                // A real contact-list node, not a patched reflection callee.
                source.bus.write16(u32::from(OWNER) + 0x1E, 0x5000);
                source.bus.write16(WRAM + 0x5000, 0);
                source.bus.write16(WRAM + 0x5004, address(incoming));
                source.bus.write16(0x12C3, if player { OWNER } else { 0 });
                FullPool::byte(&mut source, 0x6C02, if scatter { 0x40 } else { 0 });
                native.record().protection = Some(DeflectionProtection::from_control(if scatter {
                    0x40
                } else {
                    0
                }));
                let before = FullPool::auxiliary(&source);
                let mut contacts = ContactStore::default();
                contacts
                    .record_pair(native.owner, incoming, [None, None])
                    .unwrap();
                source.run(0x07F1AE, Some(FATAL), 0, OWNER, true);
                let result = weapon_reflection::reflect_contacts(
                    &mut native.objects,
                    &mut native.resources,
                    native.owner,
                    &mut ReflectionWorld {
                        contacts: Some(&contacts),
                        rules: Some(ReflectionRules {
                            owner: native.owner,
                            process_all: true,
                            player_scatter: Some(scatter),
                        }),
                        weapons: native.world.weapons.as_mut(),
                        defaults: native.world.spawn_defaults,
                        primary: player.then_some(native.owner),
                        secondary: None,
                        random: &mut native.world.random,
                    },
                );
                assert_eq!(
                    result,
                    Err(ReflectionError::Launch(LaunchError::Creation(
                        CreationError::ObjectPoolExhausted
                    )))
                );
                native.check(&source, before);
            }
        }
    }
}

fn source_path_runtime() -> Source {
    let rom = rom();
    let mut source = Source::new(&rom, 0);
    for (index, &byte) in rom[0x50000..0x54E00].iter().enumerate() {
        source.bus.write8(0x7F7E00 + index as u32, byte);
    }
    source
}

#[test]
fn all_path_spawn_forms_reach_original_fatal_before_last_spawn_publication() {
    use sf2_game::path_spawn::{ChildSpawn, IndependentSpawn, OffsetSpawn, SpawnError, SpawnState};
    let mut source = source_path_runtime();
    for entry in [0x7F9042, 0x7F91A3, 0x7F9235] {
        for prior in [false, true] {
            let mut native = FullPool::new(&mut source);
            let before = FullPool::auxiliary(&source);
            let actors = native.objects.clone();
            let last_spawn = prior.then_some(native.owner);
            source
                .bus
                .write16(WRAM + 0xD771, last_spawn.map_or(0, address));
            // Input data for the unmodified operand readers. The pool is
            // already full before these complete creation routines enter.
            source.bus.write16(0xF9, 0x6000);
            source.bus.write8(0xFB, 0x7E);
            for offset in 0..32 {
                source.bus.write8(WRAM + 0x6000 + offset, 0);
            }
            source.bus.write16(WRAM + 0x6001, 0xBC9C);
            source.bus.write8(0x1911, 0x32);
            source.run(entry, Some(FATAL), 0, OWNER, true);
            let mut spawns = SpawnState {
                last_spawn,
                ..Default::default()
            };
            let independent = IndependentSpawn {
                shape: ShapeId::EMPTY,
                path: None,
                hit_points: 0,
                attack_power: 0,
            };
            let result = match entry {
                0x7F9042 => spawns
                    .child(
                        &mut native.objects,
                        native.owner,
                        ObjectKind::Effect,
                        ChildSpawn {
                            shape: ShapeId::EMPTY,
                            path: None,
                            hit_points: 0,
                            attack_power: 0,
                            number: 0,
                            position: Default::default(),
                            rotation: Default::default(),
                        },
                        Default::default(),
                    )
                    .map(Some),
                0x7F91A3 => spawns.independent(
                    &mut native.objects,
                    native.owner,
                    ObjectKind::Effect,
                    independent,
                    Default::default(),
                ),
                _ => spawns.offset(
                    &mut native.objects,
                    native.owner,
                    ObjectKind::Effect,
                    OffsetSpawn {
                        actor: independent,
                        rotation: Default::default(),
                        offset: Default::default(),
                    },
                    Default::default(),
                ),
            };
            assert_eq!(result, Err(SpawnError::PoolExhausted));
            assert_eq!(spawns.last_spawn, last_spawn);
            assert_eq!(
                source.bus.read16(WRAM + 0xD771),
                last_spawn.map_or(0, address)
            );
            assert_eq!(native.objects, actors);
            native.check(&source, before);
        }
    }
}

#[test]
fn path_fire_fatal_prefix_clears_parameters_but_preserves_cursor_and_reserved_selection() {
    use sf2_game::path_program::{PathWorld, ProgramError, Statement};
    use sf2_game::path_runtime::PathRuntime;
    use sf2_game::{PathCursor, PathId};
    let mut source = source_path_runtime();
    let catalog = sf2_game::authored_paths::catalog();
    let entry = (0..=u16::MAX)
        .map(|command_index| PathCursor {
            path: PathId::from_catalog_index(0),
            command_index,
        })
        .find(|&cursor| matches!(catalog.statement(cursor), Ok(Statement::FireWeapon { .. })))
        .expect("generated authored weapon command");
    for selection in (2..=32).step_by(2) {
        let mut native = FullPool::new(&mut source);
        let actor = native.objects.get_mut(native.owner).unwrap();
        actor.base.path = Some(entry);
        actor.extension.path_state.weapon_selection = selection;
        let actors = native.objects.clone();
        FullPool::byte(&mut source, 0x6C06, 1);
        source.bus.write8(u32::from(OWNER) + 0x2F, selection);
        source.bus.write16(WRAM + 0xD771, OWNER);
        let before = FullPool::auxiliary(&source);
        source.run(0x7F885E, Some(FATAL), 0, OWNER, true);
        let mut runtime = PathRuntime::default();
        runtime.spawns.last_spawn = Some(native.owner);
        let mut inputs = PathWorld::unbound(&mut native.world.random, 0);
        inputs.weapons = native.world.weapons.as_mut();
        inputs.spawn_defaults = native.world.spawn_defaults;
        inputs.caller_weapon_inputs = Some(CallerWeaponInputs {
            owner: native.owner,
            active_shots: Some(ActiveShots::from_count(0)),
            weapon_level: Some(1),
            roll_step: None,
            retained_aim: None,
        });
        assert_eq!(
            runtime.enter_program(&catalog, &mut native.objects, native.owner, &mut inputs, 1),
            Err(ProgramError::WeaponLaunch(LaunchError::Creation(
                CreationError::ObjectPoolExhausted
            )))
        );
        assert_eq!(runtime.spawns.last_spawn, Some(native.owner));
        assert_eq!(source.bus.read16(WRAM + 0xD771), OWNER);
        assert_eq!(native.objects, actors);
        native.check(&source, before);
    }
}

#[test]
fn full_pool_player_rapid_preserves_linked_origin_on_fatal_but_restores_it_on_admission_rejection()
{
    use sf2_game::player_rapid::{self, RapidError};
    use sf2_game::Vector3;
    let mut source = Source::new(&rom(), 0);
    for level in 0..=u8::MAX {
        for count in [0, 8, 136] {
            for linked in [0, 0x40, 0x80, 0xC0] {
                let mut native = FullPool::new(&mut source);
                native.record().equipment.as_mut().unwrap().weapon_level = level;
                native.record().auxiliary.as_mut().unwrap().mode = 0x10;
                let charge = native.record().charge.as_mut().unwrap();
                charge.rapid_control = 0x10;
                charge.linked_mode = linked & 0x80 != 0;
                charge.linked_muzzle_disabled = linked & 0x40 != 0;
                native
                    .world
                    .bind_shots(
                        &native.objects,
                        native.owner,
                        ActiveShots::from_count(count),
                    )
                    .unwrap();
                let view = native.objects.active_ids()[2];
                native.world.fixed_players[0] = Some(view);
                let position = Vector3 {
                    x: -9000,
                    y: 91,
                    z: 7000,
                };
                native.objects.get_mut(view).unwrap().base.position = position;
                for base in [0x033F, u32::from(address(view))] {
                    source.bus.write16(base + 12, position.x as u16);
                    source.bus.write16(base + 14, position.y as u16);
                    source.bus.write16(base + 16, position.z as u16);
                }
                for (field, value) in [
                    (0x6AA0, 0x10),
                    (0x6B60, 0x10),
                    (0x6B63, linked),
                    (0x6C03, count),
                    (0x6C06, level),
                ] {
                    FullPool::byte(&mut source, field, value);
                }
                let before = FullPool::auxiliary(&source);
                let fatal = (1..=128).contains(&level) && count != 8;
                // This is the tail of a larger routine: stop before its
                // enclosing epilogue, not after popping an absent prologue.
                source.run(
                    0x07D7E4,
                    Some(if fatal { FATAL } else { 0x07D89F }),
                    0,
                    OWNER,
                    true,
                );
                let result = player_rapid::advance(
                    &mut native.objects,
                    &mut native.world,
                    &mut native.resources,
                    native.owner,
                    InputState::default(),
                );
                assert_eq!(
                    result,
                    if fatal {
                        Err(RapidError::Launch(LaunchError::Creation(
                            CreationError::ObjectPoolExhausted,
                        )))
                    } else {
                        Ok(())
                    },
                    "level {level}, count {count}, linked {linked:02X}"
                );
                assert_eq!(
                    native.world.scene.active_weapon_level,
                    Some(source.bus.read8(0x1DB6))
                );
                native.check(&source, before);
            }
        }
    }
}
