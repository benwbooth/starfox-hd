//! Original action-only player strategy and its one-time movement reset.
//! Independent retained source/native visits cover both controller sides.
use super::{action_tests::Fixture, entry_reset_tests, rom, Source, OWNER, WRAM};
use sf2_game::hit_response::HitSide;
use sf2_game::player_action::PlayerAction;
use sf2_game::player_action_wait::{self, ActionWaitPhase};
use sf2_game::positional_audio::PositionalAudio;
use sf2_game::program_resources::ProgramResources;
use sf2_game::{Behavior, Buttons, InputState};

fn input(word: u16) -> InputState {
    InputState {
        held: Buttons::from_bits(word.rotate_left(7)),
        pressed: Buttons::from_bits(word),
    }
}

fn seed_input(source: &mut Source, words: [InputState; 2], side: HitSide) {
    source.bus.write8(
        u32::from(OWNER) + 0x23,
        0xBF | if side == HitSide::Secondary { 0x40 } else { 0 },
    );
    for (index, value) in words.into_iter().enumerate() {
        source
            .bus
            .write16(0x1292 + index as u32 * 2, value.held.bits());
        source
            .bus
            .write16(0x1296 + index as u32 * 2, value.pressed.bits());
    }
}

#[test]
fn original_action_wait_all_controller_and_elapsed_words_with_both_sides_and_live_streams() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    let mut resources = ProgramResources::default();
    let mut positional = PositionalAudio::default();
    for action in [
        None,
        Some(PlayerAction::TriggeredProjectile),
        Some(PlayerAction::ForcedRetreat),
    ] {
        for word in 0..=u16::MAX {
            for secondary in [false, true] {
                f.prepare(action, word, word.rotate_left(5));
                f.world.view_transition_mode.as_mut().unwrap().flags =
                    0xFFFD | if word & 2 != 0 { 2 } else { 0 };
                f.seed(&mut source, false);
                let side = if secondary {
                    HitSide::Secondary
                } else {
                    HitSide::Primary
                };
                let words = [input(word), input(!word)];
                let selected = usize::from(secondary);
                f.world.controller_inputs = [None; 2];
                f.world.controller_inputs[selected] = Some(words[selected]);
                f.world.unmasked_player_input = Some(input(0xBD63));
                f.records().injected_input = Some(input(0x83C0));
                let actor = f.objects.get_mut(f.owner).unwrap();
                actor.base.contacts.hit_side = side;
                actor.base.behavior = Behavior::PlayerActionWait(ActionWaitPhase::Active);
                source.bus.write8(u32::from(OWNER) + 0x21, 0xA4);
                seed_input(&mut source, words, side);
                for _ in 0..2 {
                    source.run(0x0683C0, None, 0, OWNER, true);
                    player_action_wait::step(
                        &mut f.objects,
                        &mut f.world,
                        &mut resources,
                        &mut positional,
                        f.owner,
                    )
                    .unwrap();
                    f.compare(&source);
                    let actual = f.world.processed_player_input.unwrap();
                    assert_eq!(source.bus.read16(0x1936), actual.pressed.bits());
                    assert_eq!(source.bus.read16(0x1938), actual.held.bits());
                    assert_eq!(actual, words[selected]);
                    assert_eq!(source.bus.read8(u32::from(OWNER) + 0x21), 0xA5);
                    assert!(
                        f.objects
                            .get(f.owner)
                            .unwrap()
                            .base
                            .flags
                            .collision_disabled
                    );
                    assert_eq!(f.world.unmasked_player_input, Some(input(0xBD63)));
                    assert_eq!(f.records().injected_input, Some(input(0x83C0)));
                }
            }
        }
    }
}

#[test]
fn original_action_wait_initialization_resets_motion_once_and_retains_action_companion_and_age() {
    let image = rom();
    for seed in 0..=u8::MAX {
        let mut f = entry_reset_tests::Fixture::new(&image, seed);
        // Entry fixture publishes scripted view through spawn defaults;
        // bind the original gate explicitly before the action dispatcher.
        f.source.bus.write16(0x1B84, 2);
        let words = [input(u16::from(seed) * 257), input(u16::from(!seed) * 257)];
        let side = if seed & 1 == 0 {
            HitSide::Primary
        } else {
            HitSide::Secondary
        };
        f.world.controller_inputs = words.map(Some);
        f.objects.get_mut(f.owner).unwrap().base.contacts.hit_side = side;
        f.objects.get_mut(f.owner).unwrap().base.behavior =
            Behavior::PlayerActionWait(ActionWaitPhase::Initialize);
        seed_input(&mut f.source, words, side);
        let action = f.world.player(&f.objects, f.owner).unwrap().action.unwrap();
        let resource = f.objects.get(f.owner).unwrap().base.player_storage;
        let capacity = f.runtime.resources.available_capacity();
        let selected = f.select(100);
        f.source.run(0x03815A, Some(0x038192), 0, OWNER, true);
        f.audio.publish(false);
        let pending = f.select(3000);
        for visit in 0..3 {
            f.source.run(
                if visit == 0 { 0x06837F } else { 0x0683C0 },
                None,
                0,
                OWNER,
                true,
            );
            player_action_wait::step(
                &mut f.objects,
                &mut f.world,
                &mut f.runtime.resources,
                &mut f.audio,
                f.owner,
            )
            .unwrap();
            f.compare();
            assert_eq!(
                f.objects.get(f.owner).unwrap().base.player_storage,
                resource
            );
            assert_eq!(f.runtime.resources.available_capacity(), capacity);
            let current = f.world.player(&f.objects, f.owner).unwrap().action.unwrap();
            assert_eq!(current.action, action.action);
            assert_eq!(current.elapsed, 0);
            assert_eq!(current.auxiliary_counter, action.auxiliary_counter);
            assert_eq!(current.total_updates, action.total_updates);
            assert_eq!(
                f.objects.get(f.owner).unwrap().base.behavior,
                Behavior::PlayerActionWait(ActionWaitPhase::Active)
            );
            assert_eq!(f.source.bus.read16(u32::from(OWNER) + 0x19), 0x83C0);
            assert_eq!(f.source.bus.read8(u32::from(OWNER) + 0x1B), 6);
            assert_eq!(f.audio.published(), None);
            assert_eq!(f.audio.retained_selection(), Some(selected));
            assert_eq!(f.audio.pending(), Some(pending));
            let input = f.world.processed_player_input.unwrap();
            assert_eq!(f.source.bus.read16(0x1936), input.pressed.bits());
            assert_eq!(f.source.bus.read16(0x1938), input.held.bits());
            assert_eq!(input, words[usize::from(side == HitSide::Secondary)]);
            // A subsequent active visit must leave these dirty publications
            // intact; neither implementation receives the other's outputs.
            f.world.handoff.as_mut().unwrap().player_flags = 173;
            f.source.bus.write8(0x1D74, 173);
            f.world
                .player_mut(&f.objects, f.owner)
                .unwrap()
                .pose
                .as_mut()
                .unwrap()
                .yaw_trim = 7;
            f.source.bus.write8(WRAM + f.slot + 0x6AD4, 7);
        }
    }
}

#[test]
fn original_resetting_action_wait_reprepares_motion_and_runs_live_action_then_primary_palette() {
    use sf2_game::path_program::ProjectileTrigger;
    use sf2_game::player_action::{PlayerServiceFlags, ScenePalette};
    use sf2_game::view_transition::ViewTransitionMode;

    let image = rom();
    for seed in 0..=u8::MAX {
        for paused in [false, true] {
            for elapsed in [0, 2, 7, 12, 14, 39, 40, u16::MAX] {
                let mut f = entry_reset_tests::Fixture::new(&image, seed);
                f.world.view_transition_mode = Some(ViewTransitionMode {
                    flags: 0xFFFD | if paused { 2 } else { 0 },
                });
                f.source
                    .bus
                    .write16(0x1B84, f.world.view_transition_mode.unwrap().flags);
                f.world.scene.player_configuration = Some(seed % 10);
                f.source.bus.write8(0x1DE2, seed % 10);
                f.world.projectile_trigger = Some(ProjectileTrigger {
                    activation: seed & 3,
                    ..Default::default()
                });
                f.source.bus.write8(0x1E59, seed & 3);
                f.world.player_service_flags = Some(PlayerServiceFlags::from_bits(seed));
                f.source.bus.write8(0x1E0D, seed);
                f.world.primary_player = Some(f.owner);
                f.source.bus.write16(0x12C3, OWNER);
                f.world.strategy_clock = u16::from(seed);
                f.source.bus.write8(0xC4, seed);
                f.world.palette_refresh_requested = Some(false);
                f.source.bus.write8(0x1E58, 0x37);
                f.world.palette = Some(ScenePalette {
                    colors: std::array::from_fn(|index| (index as u16 * 251) ^ u16::from(seed)),
                    saved_colors: std::array::from_fn(|index| {
                        (index as u16 * 397) ^ u16::from(!seed)
                    }),
                });
                for (index, (&color, &saved)) in f
                    .world
                    .palette
                    .as_ref()
                    .unwrap()
                    .colors
                    .iter()
                    .zip(&f.world.palette.as_ref().unwrap().saved_colors)
                    .enumerate()
                {
                    f.source
                        .bus
                        .write16(WRAM + 0xEFE5 + index as u32 * 2, color);
                    f.source
                        .bus
                        .write16(WRAM + 0xF2E5 + index as u32 * 2, saved);
                }
                f.world
                    .player_mut(&f.objects, f.owner)
                    .unwrap()
                    .action
                    .as_mut()
                    .unwrap()
                    .elapsed = elapsed;
                f.source.bus.write16(WRAM + f.slot + 0x6C16, elapsed);
                f.source.bus.write16(u32::from(OWNER) + 4, 0xBC9C);
                f.source.bus.write16(u32::from(OWNER) + 0x19, 0x8362);
                f.source.bus.write8(u32::from(OWNER) + 0x1B, 6);
                f.objects.get_mut(f.owner).unwrap().base.behavior =
                    Behavior::PlayerActionWait(ActionWaitPhase::Resetting);
                let words = [input(u16::from(seed) * 257), input(u16::from(!seed) * 257)];
                let side = if seed & 1 == 0 {
                    HitSide::Primary
                } else {
                    HitSide::Secondary
                };
                f.objects.get_mut(f.owner).unwrap().base.contacts.hit_side = side;
                f.world.controller_inputs = words.map(Some);
                seed_input(&mut f.source, words, side);
                let selected = f.select(100);
                // Publish a real selection before enabling the retained
                // scene freeze bit in this strategy fixture.
                f.source.bus.write16(0x1B84, 0);
                f.source.run(0x03815A, Some(0x038192), 0, OWNER, true);
                f.audio.publish(false);
                f.source
                    .bus
                    .write16(0x1B84, f.world.view_transition_mode.unwrap().flags);
                let pending = f.select(3000);
                for _ in 0..3 {
                    f.source.run(0x068362, None, 0, OWNER, true);
                    player_action_wait::step(
                        &mut f.objects,
                        &mut f.world,
                        &mut f.runtime.resources,
                        &mut f.audio,
                        f.owner,
                    )
                    .unwrap();
                    f.compare();
                    assert_eq!(f.audio.published(), Some(selected));
                    assert_eq!(f.audio.pending(), Some(pending));
                    assert_eq!(f.source.bus.read16(u32::from(OWNER) + 0x19), 0x8362);
                    assert_eq!(f.source.bus.read8(u32::from(OWNER) + 0x1B), 6);
                    assert_eq!(
                        f.objects.get(f.owner).unwrap().base.behavior,
                        Behavior::PlayerActionWait(ActionWaitPhase::Resetting)
                    );
                    let actual = f.world.processed_player_input.unwrap();
                    assert_eq!(f.source.bus.read16(0x1936), actual.pressed.bits());
                    assert_eq!(f.source.bus.read16(0x1938), actual.held.bits());
                    assert_eq!(actual, words[usize::from(side == HitSide::Secondary)]);
                    assert_eq!(
                        f.source.bus.read8(0x1E59),
                        f.world.projectile_trigger.unwrap().activation
                    );
                    assert_eq!(
                        f.source.bus.read8(0x1E0D),
                        f.world.player_service_flags.unwrap().bits()
                    );
                    assert_eq!(
                        f.source.bus.read8(0x1E58),
                        0x37 | u8::from(f.world.palette_refresh_requested.unwrap()) * 0x80
                    );
                    for (index, (&color, &saved)) in f
                        .world
                        .palette
                        .as_ref()
                        .unwrap()
                        .colors
                        .iter()
                        .zip(&f.world.palette.as_ref().unwrap().saved_colors)
                        .enumerate()
                    {
                        assert_eq!(f.source.bus.read16(WRAM + 0xEFE5 + index as u32 * 2), color);
                        assert_eq!(f.source.bus.read16(WRAM + 0xF2E5 + index as u32 * 2), saved);
                    }
                    // External writes before the next visit, never copied
                    // from either implementation's result.
                    f.world.handoff.as_mut().unwrap().player_flags = 255;
                    f.source.bus.write8(0x1D74, 255);
                    f.world
                        .player_mut(&f.objects, f.owner)
                        .unwrap()
                        .pose
                        .as_mut()
                        .unwrap()
                        .yaw_trim = 7;
                    f.source.bus.write8(WRAM + f.slot + 0x6AD4, 7);
                }
            }
        }
    }
}

/// Scene wait ($06:84A2..84BD) into the shared tail: projection gates, the
/// per-visit mode-bit clear, the closed-gate exit and the live action.
#[test]
fn original_scene_wait_clears_entry_mode_and_runs_action_tail_or_faults_closed_gate() {
    use sf2_game::path_program::{ActionGate, ProjectileTrigger};
    use sf2_game::player_action::{PlayerServiceFlags, ScenePalette};
    use sf2_game::player_scene_entry::{self, SceneEntryError, SceneEntryPhase};
    use sf2_game::view_transition::ViewTransitionMode;

    let image = rom();
    for seed in (0..=u8::MAX).step_by(5) {
        for paused in [false, true] {
            for gate in [0, seed | 1] {
                let mut f = entry_reset_tests::Fixture::new(&image, seed);
                let mode = 0xFFFD | if paused { 2 } else { 0 };
                f.world.view_transition_mode = Some(ViewTransitionMode { flags: mode });
                f.source.bus.write16(0x1B84, mode);
                // The projection itself has its own differential coverage.
                f.world
                    .player_mut(&f.objects, f.owner)
                    .unwrap()
                    .camera_dispatch
                    .as_mut()
                    .unwrap()
                    .projection_correction_disabled = true;
                let flags = f.source.bus.read8(WRAM + f.slot + 0x6B65);
                f.source.bus.write8(WRAM + f.slot + 0x6B65, flags | 0x40);
                f.world.action_gate = Some(ActionGate { code: gate });
                f.source.bus.write8(0x1D72, gate);
                f.world.scene.player_configuration = Some(seed % 10);
                f.source.bus.write8(0x1DE2, seed % 10);
                f.world.projectile_trigger = Some(ProjectileTrigger {
                    activation: seed & 3,
                    ..Default::default()
                });
                f.source.bus.write8(0x1E59, seed & 3);
                f.world.player_service_flags = Some(PlayerServiceFlags::from_bits(seed));
                f.source.bus.write8(0x1E0D, seed);
                f.world.primary_player = Some(f.owner);
                f.source.bus.write16(0x12C3, OWNER);
                f.world.strategy_clock = u16::from(seed);
                f.source.bus.write8(0xC4, seed);
                f.world.palette_refresh_requested = Some(false);
                f.source.bus.write8(0x1E58, 0x37);
                f.world.palette = Some(ScenePalette {
                    colors: std::array::from_fn(|index| (index as u16 * 251) ^ u16::from(seed)),
                    saved_colors: std::array::from_fn(|index| {
                        (index as u16 * 397) ^ u16::from(!seed)
                    }),
                });
                let palette = f.world.palette.clone().unwrap();
                for (index, (&color, &saved)) in
                    palette.colors.iter().zip(&palette.saved_colors).enumerate()
                {
                    f.source.bus.write16(WRAM + 0xEFE5 + index as u32 * 2, color);
                    f.source.bus.write16(WRAM + 0xF2E5 + index as u32 * 2, saved);
                }
                f.source.bus.write16(u32::from(OWNER) + 4, 0xBC9C);
                f.world.engine_sound_control = Some(
                    sf2_game::player_engine_sound::EngineSoundControl::from_bits(seed ^ 0x3C),
                );
                f.source.bus.write8(0x1CE5, seed ^ 0x3C);
                f.source.bus.write16(u32::from(OWNER) + 0x19, 0x84A2);
                f.source.bus.write8(u32::from(OWNER) + 0x1B, 6);
                f.objects.get_mut(f.owner).unwrap().base.behavior =
                    Behavior::PlayerSceneEntry(SceneEntryPhase::Wait);
                let words = [input(u16::from(seed) * 257), input(u16::from(!seed) * 257)];
                let side = if seed & 1 == 0 {
                    HitSide::Primary
                } else {
                    HitSide::Secondary
                };
                f.objects.get_mut(f.owner).unwrap().base.contacts.hit_side = side;
                f.world.controller_inputs = words.map(Some);
                seed_input(&mut f.source, words, side);
                for visit in 0..3 {
                    let result =
                        player_scene_entry::wait(&mut f.objects, &mut f.world, f.owner);
                    if gate == 0 {
                        f.source.run(0x0684A2, Some(0x0684EF), 0, OWNER, true);
                        assert_eq!(result, Err(SceneEntryError::UnportedWaitExit));
                    } else {
                        f.source.run(0x0684A2, None, 0, OWNER, true);
                        result.unwrap();
                        f.compare();
                        let actual = f.world.processed_player_input.unwrap();
                        assert_eq!(f.source.bus.read16(0x1936), actual.pressed.bits());
                        assert_eq!(f.source.bus.read16(0x1938), actual.held.bits());
                        assert_eq!(actual, words[usize::from(side == HitSide::Secondary)]);
                        assert_eq!(
                            f.source.bus.read8(0x1E58),
                            0x37 | u8::from(f.world.palette_refresh_requested.unwrap()) * 0x80
                        );
                        let palette = f.world.palette.as_ref().unwrap();
                        for (index, (&color, &saved)) in
                            palette.colors.iter().zip(&palette.saved_colors).enumerate()
                        {
                            let offset = index as u32 * 2;
                            assert_eq!(f.source.bus.read16(WRAM + 0xEFE5 + offset), color);
                            assert_eq!(f.source.bus.read16(WRAM + 0xF2E5 + offset), saved);
                        }
                    }
                    assert_eq!(
                        f.source.bus.read16(0x1B84),
                        f.world.view_transition_mode.unwrap().flags,
                        "seed {seed} paused {paused} gate {gate} visit {visit}"
                    );
                    assert_eq!(f.source.bus.read16(u32::from(OWNER) + 0x19), 0x84A2);
                    // External mode publication before the next visit.
                    f.world.view_transition_mode.as_mut().unwrap().flags |= 0x0010;
                    f.source.bus.write16(0x1B84, f.source.bus.read16(0x1B84) | 0x0010);
                }
            }
        }
    }
}
