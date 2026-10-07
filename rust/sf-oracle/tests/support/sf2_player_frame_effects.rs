//! Continuous original post-movement effects tail, including real allocation,
//! shared formatting, damage, countdown, draw depth and request consumption.

use super::damage_effects_tests;
use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::path_program::ActionGate;
use sf2_game::path_protection::{DeflectionProtection, LinkedEffectActivity};
use sf2_game::player_frame_effects::{self, FrameEffectsError};
use sf2_game::player_hit_control::ShieldRecoveryRequest;
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::{authored_paths, ShapeId};

fn fixture(source: &mut Source, count: usize, value: u16) -> Native {
    let mut native = damage_effects_tests::fixture(source, count, value);
    let record = native
        .world
        .player_mut(&native.objects, native.owner)
        .unwrap();
    record.visit = Some(Default::default());
    record.appearance = Some(Default::default());
    record.protection = Some(DeflectionProtection::from_control(value as u8));
    native.world.action_gate = Some(ActionGate { code: 0 });
    native.world.shield_recovery = Some(ShieldRecoveryRequest { amount: 0 });
    native.world.active_shield_capacity = Some(80);
    native.world.linked_effect_activity = Some(LinkedEffectActivity {
        recent_spawn: value as u8,
    });
    native
}

fn seed(source: &mut Source, native: &mut Native, child_number: u8) {
    damage_effects_tests::seed(source, native);
    let record = native.world.player(&native.objects, native.owner).unwrap();
    for (offset, value) in [
        (0x6AA2, record.appearance.unwrap().depth_control),
        (0x6BFF, record.visit.unwrap().pilot_code),
        (0x6C02, record.protection.unwrap().control()),
        (
            0x6A72,
            if record.contact.unwrap().ignores_contacts {
                0x10
            } else {
                0
            },
        ),
    ] {
        source.bus.write8(WRAM + SLOT + offset, value);
    }
    for (offset, value) in [
        (0x1D72, native.world.action_gate.unwrap().code),
        (0x1E1B, native.world.shield_recovery.unwrap().amount),
        (0x1DD5, native.world.active_shield_capacity.unwrap()),
        (
            0x1DDF,
            native.world.linked_effect_activity.unwrap().recent_spawn,
        ),
    ] {
        source.bus.write8(offset, value);
    }
    source.bus.write8(0, child_number);
    let actor = native.objects.get(native.owner).unwrap();
    source.bus.write16(
        WRAM + u32::from(OWNER) + 0x1CCD,
        actor
            .extension
            .material_set
            .map_or(0, |m| m.catalog_token()),
    );
    source.bus.write16(
        WRAM + u32::from(OWNER) + 0x1CC8,
        actor.extension.depth_offset,
    );
    source.bus.write8(
        u32::from(OWNER) + 0x23,
        (source.bus.read8(u32::from(OWNER) + 0x23) & !2)
            | if actor.base.flags.visible { 0 } else { 2 },
    );
}

fn compare(source: &Source, native: &mut Native, mut before: PlayerPathRecords) {
    before.appearance.as_mut().unwrap().depth_control = source.bus.read8(WRAM + SLOT + 0x6AA2);
    before.protection = Some(DeflectionProtection::from_control(
        source.bus.read8(WRAM + SLOT + 0x6C02),
    ));
    damage_effects_tests::compare(source, native, before);
    let actor = native.objects.get(native.owner).unwrap();
    assert_eq!(
        source.bus.read16(WRAM + u32::from(OWNER) + 0x1CCD),
        actor
            .extension
            .material_set
            .map_or(0, |m| m.catalog_token())
    );
    assert_eq!(
        source.bus.read16(WRAM + u32::from(OWNER) + 0x1CC8),
        actor.extension.depth_offset
    );
    assert_eq!(
        source.bus.read8(0x1E1B),
        native.world.shield_recovery.unwrap().amount
    );
    assert_eq!(
        source.bus.read8(0x1DDF),
        native.world.linked_effect_activity.unwrap().recent_spawn
    );
}

fn advance(source: &mut Source, native: &mut Native, number: u8, fatal: Option<FrameEffectsError>) {
    seed(source, native, number);
    let before = *native.world.player(&native.objects, native.owner).unwrap();
    source.run_with_y(
        0x069EE8,
        Some(if fatal.is_some() { 0x008032 } else { 0x069F54 }),
        0,
        OWNER,
        true,
        Some(SLOT as u16),
    );
    assert_eq!(
        player_frame_effects::advance(&mut native.objects, &mut native.world, native.owner, number),
        fatal.map_or(Ok(()), Err)
    );
    compare(source, native, before);
}

#[test]
fn original_protection_installer_all_control_activity_bytes_gates_and_existing_children() {
    let mut source = Source::new(&rom(), 0xA5);
    for control in 0..=255u16 {
        for activity in 0..=255u16 {
            let mut native = fixture(&mut source, 1, control | activity << 8);
            native
                .world
                .linked_effect_activity
                .as_mut()
                .unwrap()
                .recent_spawn = activity as u8;
            native
                .objects
                .get_mut(native.owner)
                .unwrap()
                .base
                .hit_points = (activity as u8) & 1;
            // Test the authoritative objective low byte, not the stale fallback.
            native.world.objective_counts =
                Some(sf2_game::path_scene_state::EncounterObjectiveCounts {
                    remaining_word: if activity & 1 == 0 { 0xA500 } else { 0xA501 },
                    ..Default::default()
                });
            native.world.contacts_enabled = Some(activity & 1 == 0);
            seed(&mut source, &mut native, 79);
            let before = *native.world.player(&native.objects, native.owner).unwrap();
            source.run(0x07CD70, None, 0, OWNER, true);
            let child = player_frame_effects::install_protection(
                &mut native.objects,
                &native.world,
                native.owner,
            )
            .unwrap();
            assert_eq!(source.last_carry, child.is_some());
            compare(&source, &mut native, before);
            if let Some(child) = child {
                native
                    .objects
                    .get_mut(child)
                    .unwrap()
                    .base
                    .flags
                    .remove_after_tick = true;
                source.bus.write8(
                    u32::from(address(Some(child))) + 0x25,
                    source.bus.read8(u32::from(address(Some(child))) + 0x25) | 8,
                );
                let before = *native.world.player(&native.objects, native.owner).unwrap();
                source.run(0x07CD70, None, 0, OWNER, true);
                assert_eq!(
                    player_frame_effects::install_protection(
                        &mut native.objects,
                        &native.world,
                        native.owner
                    ),
                    Ok(None)
                );
                assert!(!source.last_carry);
                compare(&source, &mut native, before);
            }
        }
    }
}

#[test]
fn original_complete_effects_tail_covers_all_protection_and_clock_bytes_with_damage_and_healing() {
    let mut source = Source::new(&rom(), 0xA5);
    for control in 0..=255u16 {
        for clock in 0..=255u16 {
            let value = control << 8 | clock;
            let mut native = fixture(&mut source, 1, value);
            native.world.strategy_clock = clock;
            native.world.action_gate.as_mut().unwrap().code =
                if control & 8 != 0 { clock as u8 & 1 } else { 0 };
            native.world.contacts_enabled = Some(control & 16 == 0);
            native.world.shield_recovery.as_mut().unwrap().amount = control as u8;
            native.world.active_shield_capacity = Some(clock as u8);
            let record = native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap();
            record.protection = Some(DeflectionProtection::from_control(control as u8));
            record.visit.as_mut().unwrap().pilot_code = clock as u8;
            record.contact.as_mut().unwrap().hit.reserve_shield =
                (control as u8).wrapping_add(clock as u8);
            record.contact.as_mut().unwrap().hit.recovery = (control as u8) & 0xC0;
            record.particles.as_mut().unwrap().flags = (control as u8) ^ clock as u8;
            record.particles.as_mut().unwrap().age = 98 + (clock as u8 & 1);
            record.auxiliary.as_mut().unwrap().mode = if clock & 2 == 0 { 0x20 } else { 0x10 };
            record.motion.as_mut().unwrap().walker_contact_control = clock as u8;
            advance(&mut source, &mut native, 77, None);
        }
    }
}

#[test]
fn original_retained_effect_visits_keep_preheal_flash_live_countdown_and_numbered_child_reuse() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = fixture(&mut source, 1, 0);
    native.objects.get_mut(native.owner).unwrap().base.shape = ShapeId::EMPTY;
    native
        .world
        .spawn_defaults
        .as_mut()
        .unwrap()
        .run_when_paused = true;
    for visit in 0..4096u16 {
        native.world.strategy_clock = visit.wrapping_mul(257);
        native.world.action_gate.as_mut().unwrap().code = u8::from(visit & 7 == 3);
        native.world.contacts_enabled = Some(visit & 7 != 5);
        native.world.shield_recovery.as_mut().unwrap().amount = visit as u8;
        if visit & 31 == 0 {
            let record = native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap();
            record.protection = Some(DeflectionProtection::from_control(visit as u8 | 31));
            record.contact.as_mut().unwrap().hit.reserve_shield = visit as u8;
        }
        advance(&mut source, &mut native, visit as u8, None);
    }
    assert_eq!(native.objects.len(), 3);
}

#[test]
fn original_composed_pool_exhaustion_preserves_each_distinct_completed_prefix() {
    use sf2_game::player_appearance::AppearanceError;
    use sf2_game::player_damage_effects::DamageEffectsError;
    use sf2_game::player_recovery::RecoveryError;
    let mut source = Source::new(&rom(), 0xA5);
    for stage in 0..4 {
        let mut native = fixture(&mut source, 60, 0);
        native
            .objects
            .get_mut(native.owner)
            .unwrap()
            .extension
            .depth_offset = 0xACED;
        let record = native
            .world
            .player_mut(&native.objects, native.owner)
            .unwrap();
        record.protection = Some(DeflectionProtection::from_control(0xE1));
        record.contact.as_mut().unwrap().hit.reserve_shield = if stage == 0 { 12 } else { 13 };
        record.particles.as_mut().unwrap().flags = if stage == 1 { 0x20 } else { 0 };
        native.world.action_gate.as_mut().unwrap().code = u8::from(stage == 3);
        native.world.shield_recovery.as_mut().unwrap().amount = 10;
        advance(
            &mut source,
            &mut native,
            71,
            Some(match stage {
                0 => FrameEffectsError::Appearance(AppearanceError::ObjectPoolExhausted),
                1 => FrameEffectsError::Damage(DamageEffectsError::ObjectPoolExhausted),
                2 => FrameEffectsError::ObjectPoolExhausted,
                3 => FrameEffectsError::Recovery(RecoveryError::ObjectPoolExhausted),
                _ => unreachable!(),
            }),
        );
    }
}

struct Callbacks;
impl sf2_game::scene_strategy::SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: sf2_game::ObjectId,
    ) -> Result<sf2_game::strategy_schedule::StrategyCompletion, Self::Error> {
        panic!("protection must execute its authored path")
    }
    fn death_override(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: sf2_game::ObjectId,
    ) -> Result<Option<sf2_game::strategy_schedule::StrategyCompletion>, Self::Error> {
        panic!("unexpected death override")
    }
    fn resume_map_on_death(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: sf2_game::ObjectId,
    ) -> Result<(), Self::Error> {
        panic!("unexpected map continuation")
    }
}

#[test]
fn original_protection_birth_player_countdown_real_path_spin_flicker_and_retirement_compose() {
    use sf2_game::collision_surface::SurfaceMode;
    use sf2_game::path_control::PlayerTarget;
    use sf2_game::path_sound::{AuthoredCue, CueListener};
    use sf2_game::player_action::PlayerServiceFlags;
    use sf2_game::scene_path_world::AudioRouting;
    use sf2_game::scene_strategy::{SceneActors, SceneExecution};
    use sf2_game::strategy_schedule::StrategyHost;
    use sf2_game::SoundEvent;
    use sf_oracle::{call_near, Entry};
    let rom = rom();
    let catalog = authored_paths::catalog();
    for activity in [0, 1, 2, 255] {
        for control in [1, 2, 31, 0xE1, 0xFF] {
            for mode in 0..4 {
                let mut source = Source::new(&rom, 0);
                source.bus.enable_gsu();
                for (index, &byte) in rom[0x50000..0x54E00].iter().enumerate() {
                    source.bus.write8(0x7F7E00 + index as u32, byte);
                }
                source.run(0x7F1737, None, 0, OWNER, true);
                let mut native = fixture(&mut source, 1, 0xFEDC);
                native.objects.get_mut(native.owner).unwrap().base.shape = ShapeId::EMPTY;
                native
                    .world
                    .linked_effect_activity
                    .as_mut()
                    .unwrap()
                    .recent_spawn = activity;
                native.world.scene.player_configuration = Some(if mode == 1 { 9 } else { 0 });
                native.world.surface_mode = Some(SurfaceMode {
                    flags: if mode == 2 { 0 } else { 1 },
                });
                native.world.player_service_flags = Some(PlayerServiceFlags::from_bits(0));
                native.world.audio_routing = Some(AudioRouting {
                    listeners: [CueListener::PrimaryPlayer; 2],
                    markers: None,
                });
                let record = native
                    .world
                    .player_mut(&native.objects, native.owner)
                    .unwrap();
                record.protection = Some(DeflectionProtection::from_control(control));
                record.particles = Some(Default::default());
                record.contact.as_mut().unwrap().hit.reserve_shield = 255;
                seed(&mut source, &mut native, 71);
                let before = *native.world.player(&native.objects, native.owner).unwrap();
                source.run(0x07CD70, None, 0, OWNER, true);
                let child = player_frame_effects::install_protection(
                    &mut native.objects,
                    &native.world,
                    native.owner,
                )
                .unwrap()
                .unwrap();
                compare(&source, &mut native, before);
                let mut execution = SceneExecution::default();
                let mut callbacks = Callbacks;
                for visit in 1..=300u16 {
                    native.world.strategy_clock = visit;
                    native.world.action_gate.as_mut().unwrap().code =
                        u8::from((mode == 1 || mode == 2) && visit >= 12);
                    let service = if mode == 3 && visit >= 12 { 1 } else { 0 };
                    native.world.player_service_flags =
                        Some(PlayerServiceFlags::from_bits(service));
                    // Execute the ENTIRE player effects tail before the existing
                    // child gets its next strategy visit. No child/phase patching.
                    seed(&mut source, &mut native, 71);
                    source
                        .bus
                        .write8(0x1DE2, native.world.scene.player_configuration.unwrap());
                    source
                        .bus
                        .write8(0x1B4D, native.world.surface_mode.unwrap().flags);
                    source.bus.write8(0x1E0D, service);
                    source.run_with_y(0x069EE8, Some(0x069F54), 0, OWNER, true, Some(SLOT as u16));
                    player_frame_effects::advance(
                        &mut native.objects,
                        &mut native.world,
                        native.owner,
                        71,
                    )
                    .unwrap();
                    let base = u32::from(address(Some(child)));
                    let strategy = u32::from(source.bus.read16(base + 0x19))
                        | u32::from(source.bus.read8(base + 0x1B)) << 16;
                    source.run(strategy, None, 0, base as u16, true);
                    let mut host = SceneActors {
                        objects: &mut native.objects,
                        world: &mut native.world,
                        execution: &mut execution,
                        catalog: &catalog,
                        callbacks: &mut callbacks,
                        statement_budget: 256,
                    };
                    host.run_strategy(child, visit).unwrap();
                    let actor = host.objects.get(child).unwrap();
                    for (offset, actual) in [
                        (12, actor.base.position.x),
                        (14, actor.base.position.y),
                        (16, actor.base.position.z),
                    ] {
                        assert_eq!(source.bus.read16(base + offset) as i16, actual, "activity {activity} control {control} mode {mode} visit {visit} position {offset:X}");
                    }
                    for (offset, actual) in [
                        (0x1CD5, actor.extension.relative_rotation.pitch.units()),
                        (0x1CD7, actor.extension.relative_rotation.roll.units()),
                        (0x1CCB, actor.extension.path_state.animation.shape.packed()),
                        (0x1CDA, actor.extension.texture_scroll_x),
                    ] {
                        assert_eq!(source.bus.read8(WRAM + base + offset), actual, "activity {activity} control {control} mode {mode} visit {visit} field {offset:X}");
                    }
                    assert_eq!(
                        source.bus.read16(WRAM + base + 0x1CE2),
                        actor.extension.path_state.motion_phase
                    );
                    assert_eq!(
                        source.bus.read8(WRAM + SLOT + 0x6C02),
                        host.world
                            .player(host.objects, native.owner)
                            .unwrap()
                            .protection
                            .unwrap()
                            .control()
                    );
                    assert_eq!(
                        source.bus.read8(0x1DDF),
                        host.world.linked_effect_activity.unwrap().recent_spawn
                    );
                    assert_eq!(
                        source.bus.read8(base + 0x25) & 8 != 0,
                        actor.base.flags.remove_after_tick
                    );
                    let removed = actor.base.flags.remove_after_tick;
                    let events: Vec<_> = host
                        .world
                        .audio
                        .take_events()
                        .into_iter()
                        .flatten()
                        .collect();
                    assert_eq!(source.bus.read16(0x1D16) as usize, events.len() * 2);
                    if activity == 0 && visit == 1 {
                        assert_eq!(
                            events,
                            vec![SoundEvent::Authored(AuthoredCue::new(
                                20,
                                0,
                                PlayerTarget::Primary
                            ))]
                        );
                        assert_eq!(source.bus.read16(0x1CF6), 20);
                    } else {
                        assert!(events.is_empty());
                    }
                    if removed {
                        assert!(
                            call_near(
                                &mut source.bus,
                                0x7F335A,
                                &Entry {
                                    x: base as u16,
                                    dbr: 0x7E,
                                    p: 0x20,
                                    ..Default::default()
                                }
                            )
                            .returned
                        );
                        host.retire_object(child).unwrap();
                        assert_eq!(host.execution.paths.runtime.resources.owner_count(child), 0);
                        break;
                    }
                }
                assert_eq!(
                    native.objects.len(),
                    1,
                    "protection must finish activity {activity} control {control} mode {mode}"
                );
                native.compare_pool(&source);
            }
        }
    }
}
