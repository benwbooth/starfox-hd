//! Sustained damage, countdown puffs and flame attachment against the
//! unmodified original routines, including their shared authored path VM.

use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_program::{SelectedAuxiliaryState, SelectedParticleEffects};
use sf2_game::path_sound::AuthoredCue;
use sf2_game::player_damage_effects::{self, DamageEffectsError};
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::{authored_paths, Behavior, ObjectId, ShapeId, SoundEvent};

pub(super) fn fixture(source: &mut Source, count: usize, value: u16) -> Native {
    let mut native = Native::new(source, count, value as u8, (value >> 8) as u8, value);
    let actor = native.objects.get_mut(native.owner).unwrap();
    actor.base.shape = ShapeId::from_catalog_index(2);
    actor.base.hit_points = 1;
    actor.base.speed = value as u8;
    let record = native
        .world
        .player_mut(&native.objects, native.owner)
        .unwrap();
    record.particles = Some(SelectedParticleEffects {
        flags: value as u8,
        age: (value >> 8) as u8,
    });
    record.auxiliary = Some(SelectedAuxiliaryState {
        mode: 0,
        action_flags: 0,
        stored_world_position: Default::default(),
        stored_rotation: Default::default(),
    });
    record.roll = Some(Default::default());
    record.motion = Some(Default::default());
    record.contact = Some(Default::default());
    record.charge = Some(Default::default());
    record.pose = Some(Default::default());
    native.world.contacts_enabled = Some(true);
    native.world.primary_player = Some(native.owner);
    native.world.strategy_clock = value;
    native.world.random = sf2_game::RandomState::new([
        value as u8,
        (value >> 8) as u8,
        (value as u8).wrapping_add(127),
        (value >> 8) as u8 ^ 0xD3,
    ]);
    native
}

pub(super) fn seed(source: &mut Source, native: &mut Native) {
    let record = native.world.player(&native.objects, native.owner).unwrap();
    let hit = record.contact.unwrap().hit;
    for (offset, value) in [
        (0x6BE4, record.particles.unwrap().flags),
        (0x6BE5, record.particles.unwrap().age),
        (0x6AA0, record.auxiliary.unwrap().mode),
        (0x6B77, record.auxiliary.unwrap().action_flags),
        (0x6ADD, record.roll.unwrap().impulse as u8),
        (0x6B94, record.motion.unwrap().walker_contact_control),
        (0x6BE3, hit.recovery),
        (0x6C00, hit.reserve_shield),
        (0x6C11, hit.feedback_duration),
        (0x6C12, hit.feedback_flags),
        (0x6C09, record.charge.unwrap().control),
        (0x6ADA, record.pose.unwrap().heading_return_bank as u8),
        (
            0x6B63,
            if record.charge.unwrap().linked_mode {
                0x80
            } else {
                0
            } | if record.charge.unwrap().linked_muzzle_disabled {
                0x40
            } else {
                0
            },
        ),
    ] {
        source.bus.write8(WRAM + SLOT + offset, value);
    }
    source
        .bus
        .write16(WRAM + SLOT + 0x6B3B, hit.camera_pitch_recoil as u16);
    let actor = native.objects.get(native.owner).unwrap();
    source.bus.write8(u32::from(OWNER) + 0x18, actor.base.speed);
    source
        .bus
        .write8(u32::from(OWNER) + 0x2D, actor.base.hit_points);
    source.bus.write16(
        u32::from(OWNER) + 4,
        0xBC9C + actor.base.shape.catalog_index() as u16 * 28,
    );
    source.bus.write16(0xC4, native.world.strategy_clock);
    source.bus.write16(
        0x1B84,
        if native.world.scripted_view_active().unwrap() {
            2
        } else {
            0
        },
    );
    source.bus.write8(
        WRAM + 0xD7F4,
        if native.world.contacts_enabled().unwrap() {
            1
        } else {
            0
        },
    );
    source.bus.write16(0x12C3, OWNER);
    for (index, byte) in native.world.random.bytes().into_iter().enumerate() {
        source.bus.write8(0xE0 + index as u32, byte);
    }
    native.world.audio = Default::default();
    source.bus.write16(0x1D16, 0);
    for slot in 0..16 {
        source.bus.write16(0x1CF6 + slot * 2, 0xACE0 + slot as u16);
    }
}

pub(super) fn compare(source: &Source, native: &mut Native, mut before: PlayerPathRecords) {
    let byte = |offset| source.bus.read8(WRAM + SLOT + offset);
    before.particles.as_mut().unwrap().flags = byte(0x6BE4);
    before.particles.as_mut().unwrap().age = byte(0x6BE5);
    let hit = &mut before.contact.as_mut().unwrap().hit;
    hit.recovery = byte(0x6BE3);
    hit.reserve_shield = byte(0x6C00);
    hit.feedback_duration = byte(0x6C11);
    hit.feedback_flags = byte(0x6C12);
    hit.camera_pitch_recoil = source.bus.read16(WRAM + SLOT + 0x6B3B) as i16;
    before.charge.as_mut().unwrap().control = byte(0x6C09);
    before.pose.as_mut().unwrap().heading_return_bank = byte(0x6ADA) as i8;
    assert_eq!(
        *native.world.player(&native.objects, native.owner).unwrap(),
        before
    );
    assert_eq!(
        source.bus.read8(u32::from(OWNER) + 0x2D),
        native.objects.get(native.owner).unwrap().base.hit_points
    );
    for (index, byte) in native.world.random.bytes().into_iter().enumerate() {
        assert_eq!(source.bus.read8(0xE0 + index as u32), byte);
    }
    let events: Vec<_> = native
        .world
        .audio
        .take_events()
        .into_iter()
        .flatten()
        .collect();
    assert!(events.len() <= 1);
    assert_eq!(source.bus.read16(0x1D16) as usize, events.len() * 2);
    for slot in 0..16 {
        assert_eq!(
            source.bus.read16(0x1CF6 + slot * 2),
            if slot == 0 && !events.is_empty() {
                109
            } else {
                0xACE0 + slot as u16
            }
        );
    }
    for event in events {
        assert_eq!(
            event,
            SoundEvent::Authored(AuthoredCue::new(109, 0, PlayerTarget::Primary))
        );
    }
    native.compare_pool(source);
    for (id, actor) in native.objects.active_objects() {
        if actor.base.behavior != Behavior::FollowPath {
            continue;
        }
        let base = u32::from(address(Some(id)));
        assert_eq!(
            source.bus.read16(base + 4),
            0xBC9C + actor.base.shape.catalog_index() as u16 * 28
        );
        assert_eq!(source.bus.read16(base + 0x19), 0x7E1E);
        assert_eq!(source.bus.read8(base + 0x1B), 0x7F);
        assert!(actor.extension.path_state.needs_path_initialization);
        assert_eq!(
            source.bus.read16(base + 0x2B),
            match actor.base.path.unwrap() {
                authored_paths::RANDOMIZED_COLOR_PARTICLE => 0xF294,
                authored_paths::CHILD_DETACHING_SPRITE => 0xF540,
                authored_paths::LOCAL_JITTER_SPRITE => 0xF521,
                authored_paths::LINKED_PROTECTION_EFFECT => 0xF2B9,
                authored_paths::PRIMARY_TARGET_FOLLOWER => 0xF38A,
                other => panic!("unexpected effect path {other:?}"),
            }
        );
        assert_eq!(
            source.bus.read16(WRAM + base + 0x1CD8),
            address(actor.extension.parent)
        );
        for (offset, actual) in [
            (12, actor.base.position.x),
            (14, actor.base.position.y),
            (16, actor.base.position.z),
            (0x32, actor.base.velocity.x),
            (0x34, actor.base.velocity.y),
            (0x36, actor.base.velocity.z),
            (0x1CCF, actor.extension.relative_position.x),
            (0x1CD1, actor.extension.relative_position.y),
            (0x1CD3, actor.extension.relative_position.z),
        ] {
            assert_eq!(
                source.bus.read16(WRAM + base + offset) as i16,
                actual,
                "child {} field {offset:X}",
                id.index()
            );
        }
        for (offset, actual) in [
            (0x12, actor.base.pitch.units()),
            (0x14, actor.base.yaw.units()),
            (0x16, actor.base.roll.units()),
            (0x18, actor.base.speed),
            (0x13, actor.base.child_number),
            (0x2D, actor.base.hit_points),
            (0x2E, actor.base.attack_power),
            (0x1CF0, actor.extension.spawn_group),
        ] {
            assert_eq!(
                source.bus.read8(WRAM + base + offset),
                actual,
                "child {} byte {offset:X}",
                id.index()
            );
        }
        for (offset, mask, flag) in [
            (0x20, 8, actor.base.flags.casts_shadow),
            (0x21, 1, actor.base.flags.collision_disabled),
            (0x22, 4, actor.base.flags.general_search_eligible),
            (
                0x23,
                4,
                actor.extension.path_state.motion.attached_coordinates,
            ),
            (0x25, 1, actor.base.flags.remove_with_parent),
            (0x26, 8, actor.base.contacts.run_when_paused),
        ] {
            assert_eq!(
                source.bus.read8(base + offset) & mask != 0,
                flag,
                "child {} flag {offset:X}:{mask:X}",
                id.index()
            );
        }
    }
}

fn advance(source: &mut Source, native: &mut Native, fatal: bool) {
    let before = *native.world.player(&native.objects, native.owner).unwrap();
    source.run(0x07D1A8, fatal.then_some(0x008032), 0, OWNER, true);
    let result =
        player_damage_effects::advance(&mut native.objects, &mut native.world, native.owner);
    assert_eq!(
        result,
        if fatal {
            Err(DamageEffectsError::ObjectPoolExhausted)
        } else {
            Ok(())
        }
    );
    compare(source, native, before);
}

#[test]
fn original_sustained_damage_all_flag_and_clock_bytes_with_real_emitters() {
    let mut source = Source::new(&rom(), 0xA5);
    for flags in 0..=255u16 {
        for clock in 0..=255u16 {
            let mut native = fixture(&mut source, 1, flags << 8 | clock);
            native.world.strategy_clock = clock;
            let record = native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap();
            record.particles = Some(SelectedParticleEffects {
                flags: flags as u8,
                age: clock as u8,
            });
            record.contact.as_mut().unwrap().hit.reserve_shield = clock as u8;
            record.contact.as_mut().unwrap().hit.recovery = flags as u8 & 0xC0;
            record.charge.as_mut().unwrap().linked_mode = clock & 2 != 0;
            record.charge.as_mut().unwrap().linked_muzzle_disabled = clock & 4 != 0;
            seed(&mut source, &mut native);
            advance(&mut source, &mut native, false);
        }
    }
}

#[test]
fn original_sustained_age_and_roll_exhaustive_and_gate_order() {
    let mut source = Source::new(&rom(), 0xA5);
    for age in 0..=255u16 {
        for roll in 0..=255u16 {
            let mut native = fixture(&mut source, 1, age << 8 | roll);
            native.world.strategy_clock = 31;
            let record = native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap();
            record.particles = Some(SelectedParticleEffects {
                flags: 0xB1,
                age: age as u8,
            });
            record.roll.as_mut().unwrap().impulse = roll as i8;
            seed(&mut source, &mut native);
            advance(&mut source, &mut native, false);
        }
    }
    for gates in 0..8 {
        let mut native = fixture(&mut source, 1, 0x21E1);
        native
            .world
            .spawn_defaults
            .as_mut()
            .unwrap()
            .run_when_paused = gates & 1 != 0;
        native.world.contacts_enabled = Some(gates & 2 != 0);
        native
            .objects
            .get_mut(native.owner)
            .unwrap()
            .base
            .hit_points = (gates & 4) as u8;
        seed(&mut source, &mut native);
        advance(&mut source, &mut native, false);
    }
}

#[test]
fn original_sustained_mode_and_surface_controls_are_independent_byte_tests() {
    let mut source = Source::new(&rom(), 0xA5);
    for mode in 0..=255u16 {
        for surface in 0..=255u16 {
            let mut native = fixture(&mut source, 1, mode << 8 | surface);
            native.world.strategy_clock = if surface & 1 == 0 { 2 } else { 3 };
            let record = native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap();
            record.particles = Some(SelectedParticleEffects {
                flags: 0xE1,
                age: 23,
            });
            record.auxiliary.as_mut().unwrap().mode = mode as u8;
            record.motion.as_mut().unwrap().walker_contact_control = surface as u8;
            record.charge.as_mut().unwrap().linked_mode = surface & 2 != 0;
            seed(&mut source, &mut native);
            advance(&mut source, &mut native, false);
        }
    }
}

#[test]
fn original_sustained_damage_all_shield_and_recovery_bytes_preserve_other_hit_state() {
    let mut source = Source::new(&rom(), 0xA5);
    for shield in 0..=255u16 {
        for recovery in 0..=255u16 {
            let mut native = fixture(&mut source, 1, shield << 8 | recovery);
            native.world.strategy_clock = 32;
            let record = native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap();
            record.particles = Some(SelectedParticleEffects {
                flags: 0x40,
                age: 3,
            });
            let hit = &mut record.contact.as_mut().unwrap().hit;
            hit.reserve_shield = shield as u8;
            hit.recovery = recovery as u8;
            hit.feedback_flags = shield as u8;
            hit.feedback_duration = recovery as u8;
            hit.camera_pitch_recoil = if shield & 1 == 0 {
                0
            } else {
                (shield << 8 | recovery) as i16
            };
            record.charge.as_mut().unwrap().control = shield as u8;
            record.pose.as_mut().unwrap().heading_return_bank = recovery as i8;
            seed(&mut source, &mut native);
            advance(&mut source, &mut native, false);
        }
    }
}

#[test]
fn original_countdown_puff_all_speeds_all_surface_bytes_and_five_child_limit() {
    let mut source = Source::new(&rom(), 0xA5);
    for speed in 0..=255u16 {
        for surface in 0..=255u16 {
            let mut native = fixture(&mut source, 1, speed << 8 | surface);
            native.objects.get_mut(native.owner).unwrap().base.speed = speed as u8;
            native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap()
                .motion
                .as_mut()
                .unwrap()
                .walker_contact_control = surface as u8;
            seed(&mut source, &mut native);
            let before = *native.world.player(&native.objects, native.owner).unwrap();
            source.run(0x07D048, None, 0, OWNER, true);
            player_damage_effects::emit_puff(&mut native.objects, &mut native.world, native.owner)
                .unwrap();
            compare(&source, &mut native, before);
        }
    }
    let mut native = fixture(&mut source, 1, 0x89FE);
    native
        .world
        .player_mut(&native.objects, native.owner)
        .unwrap()
        .motion
        .as_mut()
        .unwrap()
        .walker_contact_control = 0;
    for _ in 0..8 {
        seed(&mut source, &mut native);
        let before = *native.world.player(&native.objects, native.owner).unwrap();
        source.run(0x07D048, None, 0, OWNER, true);
        player_damage_effects::emit_puff(&mut native.objects, &mut native.world, native.owner)
            .unwrap();
        compare(&source, &mut native, before);
    }
    assert_eq!(native.objects.len(), 6);
}

#[test]
fn original_sustained_fatal_allocations_preserve_earlier_damage_countdown_and_rng_order() {
    let mut source = Source::new(&rom(), 0xA5);
    for (count, flags, clock) in [(60, 0x40, 1), (59, 0xA0, 0), (59, 0x4F, 1), (60, 0x0F, 1)] {
        let mut native = fixture(&mut source, count, 0x9C20);
        native.world.strategy_clock = clock;
        let record = native
            .world
            .player_mut(&native.objects, native.owner)
            .unwrap();
        record.particles = Some(SelectedParticleEffects { flags, age: 0 });
        record.contact.as_mut().unwrap().hit.reserve_shield = 17;
        seed(&mut source, &mut native);
        advance(&mut source, &mut native, true);
    }
}

struct Callbacks;
impl sf2_game::scene_strategy::SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<sf2_game::strategy_schedule::StrategyCompletion, Self::Error> {
        panic!("particle must execute its authored path")
    }
    fn death_override(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<sf2_game::strategy_schedule::StrategyCompletion>, Self::Error> {
        panic!("unexpected death override")
    }
    fn resume_map_on_death(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<(), Self::Error> {
        panic!("unexpected map continuation")
    }
}

#[test]
fn original_puff_and_flame_birth_real_path_motion_animation_and_retirement_compose() {
    use sf2_game::scene_strategy::{SceneActors, SceneExecution};
    use sf2_game::strategy_schedule::StrategyHost;
    use sf_oracle::{call_near, Entry};
    let rom = rom();
    let catalog = authored_paths::catalog();
    for value in [0u16, 1, 127, 128, 255, 256, 32767, 32768, 65535] {
        for action in [0, 0x40] {
            let mut source = Source::new(&rom, 0);
            source.bus.enable_gsu();
            for (index, &byte) in rom[0x50000..0x54E00].iter().enumerate() {
                source.bus.write8(0x7F7E00 + index as u32, byte);
            }
            source.run(0x7F1737, None, 0, OWNER, true);
            let mut native = fixture(&mut source, 1, value);
            native.world.strategy_clock = 1;
            let record = native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap();
            record.particles = Some(SelectedParticleEffects {
                flags: 0x4F,
                age: 0,
            });
            record.auxiliary.as_mut().unwrap().action_flags = action;
            seed(&mut source, &mut native);
            advance(&mut source, &mut native, false);
            assert_eq!(native.objects.len(), 3);
            let mut execution = SceneExecution::default();
            let mut callbacks = Callbacks;
            let mut host = SceneActors {
                objects: &mut native.objects,
                world: &mut native.world,
                execution: &mut execution,
                catalog: &catalog,
                callbacks: &mut callbacks,
                statement_budget: 256,
            };
            for visit in 0..40u16 {
                let children = host.objects.active_ids().to_vec();
                for child in children {
                    if child == native.owner {
                        continue;
                    }
                    let clock = value.wrapping_add(visit);
                    source.bus.write16(0xC4, clock);
                    let base = u32::from(address(Some(child)));
                    let strategy = u32::from(source.bus.read16(base + 0x19))
                        | u32::from(source.bus.read8(base + 0x1B)) << 16;
                    source.run(strategy, None, 0, base as u16, true);
                    host.run_strategy(child, clock).unwrap();
                    let actor = host.objects.get(child).unwrap();
                    for (offset, actual) in [
                        (12, actor.base.position.x),
                        (14, actor.base.position.y),
                        (16, actor.base.position.z),
                        (0x32, actor.base.velocity.x),
                        (0x34, actor.base.velocity.y),
                        (0x36, actor.base.velocity.z),
                        (0x1CCF, actor.extension.relative_position.x),
                        (0x1CD1, actor.extension.relative_position.y),
                        (0x1CD3, actor.extension.relative_position.z),
                    ] {
                        assert_eq!(
                            source.bus.read16(WRAM + base + offset) as i16,
                            actual,
                            "seed {value} action {action} visit {visit} child {} field {offset:X}",
                            child.index()
                        );
                    }
                    assert_eq!(source.bus.read8(base + 0x18), actor.base.speed);
                    assert_eq!(
                        source.bus.read16(WRAM + base + 0x1CE2),
                        actor.extension.path_state.motion_phase
                    );
                    assert_eq!(
                        source.bus.read8(WRAM + base + 0x1CDA),
                        actor.extension.texture_scroll_x
                    );
                    assert_eq!(
                        source.bus.read8(WRAM + base + 0x1CCA),
                        actor.extension.path_state.animation.color.packed()
                    );
                    assert_eq!(
                        source.bus.read8(base + 0x25) & 8 != 0,
                        actor.base.flags.remove_after_tick
                    );
                    for (index, byte) in host.world.random.bytes().into_iter().enumerate() {
                        assert_eq!(source.bus.read8(0xE0 + index as u32), byte);
                    }
                    if actor.base.flags.remove_after_tick {
                        let result = call_near(
                            &mut source.bus,
                            0x7F335A,
                            &Entry {
                                x: base as u16,
                                dbr: 0x7E,
                                p: 0x20,
                                ..Default::default()
                            },
                        );
                        assert!(result.returned);
                        host.retire_object(child).unwrap();
                        assert!(host.objects.get(child).is_none());
                        assert_eq!(host.execution.paths.runtime.resources.owner_count(child), 0);
                    }
                }
                if host.objects.len() == 1 {
                    break;
                }
            }
            assert_eq!(host.objects.len(), 1, "both real scripts must finish");
            assert!(host
                .objects
                .get(native.owner)
                .unwrap()
                .base
                .attachment_next
                .is_none());
            native.compare_pool(&source);
        }
    }
}
