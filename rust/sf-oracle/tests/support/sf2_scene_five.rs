//! Entire indexed scene-nine path, including its newborn actors, recurring
//! camera callbacks, sound publications and externally released action gate.
use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_program::{ActionGate, SelectedAuxiliaryState};
use sf2_game::path_protection::{DeflectionProtection, LinkedEffectActivity};
use sf2_game::path_scene_state::CameraTrackingTarget;
use sf2_game::path_sound::{AuthoredCue, CueListener, CueMarker, MarkerInputs};
use sf2_game::player_engine_sound::EngineSoundControl;
use sf2_game::scene_path_world::AudioRouting;
use sf2_game::scene_strategy::{SceneActors, SceneCallbacks, SceneExecution};
use sf2_game::strategy_schedule::{StrategyCompletion, StrategyHost};
use sf2_game::view_transition::FixedViewAngles;
use sf2_game::{authored_paths, Behavior, ObjectId, RandomState};
use sf_oracle::{call_near, Entry};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("unimplemented scene strategy")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected scene death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map callback")
    }
}

fn encoded(cue: AuthoredCue) -> u16 {
    u16::from(cue.id)
        | u16::from(cue.parameter()) << 8
        | if cue.target == PlayerTarget::Secondary {
            0x8000
        } else {
            0
        }
}

fn load_paths(source: &mut Source, bytes: &[u8]) {
    for (i, &byte) in bytes[0x50000..0x54E00].iter().enumerate() {
        source.bus.write8(0x7F7E00 + i as u32, byte);
    }
}

#[test]
fn complete_scene_five_matches_original_all_pilots_camera_callbacks_children_and_gate_release() {
    let bytes = rom();
    let catalog = authored_paths::catalog();
    for pilot in 0..6u8 {
        for protection in [0, 0x8F] {
            for (variant, release) in [1, 170, 410].into_iter().enumerate() {
                let mut source = Source::new(&bytes, 0);
                source.bus.enable_gsu();
                load_paths(&mut source, &bytes);
                source.run(0x7F1737, None, 0, OWNER, true);
                let mut native = Native::new(&mut source, 3, 87, 193, 0xEF73);
                let player = native.owner;
                let owner = native.objects.active_ids()[1];
                let view = native.objects.active_ids()[0];
                let base = u32::from(address(Some(owner)));
                let root = native.objects.get_mut(owner).unwrap();
                root.base.behavior = Behavior::FollowPath;
                root.base.path = Some(authored_paths::SCENE_FIVE);
                root.base.hit_points = 1;
                root.base.attack_power = 1;
                root.base.flags.reclaim_on_pool_pressure = false;
                root.extension.path_state.needs_path_initialization = true;
                for (offset, value) in [(0x2B, 0xB65B), (0x19, 0x7E1E)] {
                    source.bus.write16(base + offset, value);
                }
                for (offset, value) in [(0x1B, 0x7F), (0x20, 8), (0x2D, 1), (0x2E, 1)] {
                    source.bus.write8(base + offset, value);
                }
                native.world.primary_player = Some(player);
                native.world.fixed_players[0] = Some(view);
                native.world.action_gate = Some(ActionGate { code: 77 });
                native.world.camera_tracking = Some(CameraTrackingTarget::default());
                native.world.engine_sound_control = Some(EngineSoundControl::from_bits(0xED));
                native.world.linked_effect_activity =
                    Some(LinkedEffectActivity { recent_spawn: 0xAD });
                native.world.scene.active_pilot = Some(pilot);
                let wingmate = (pilot + 3 + variant as u8 * 2) % 6;
                native.world.scene.wingmate_pilot = Some(wingmate);
                source.bus.write8(0x1E70, wingmate);
                let phase = [0u8, 1, 2, 128, 129, 255][(usize::from(pilot) + variant) % 6];
                native.world.campaign_phase = Some(phase);
                source.bus.write8(0x1BE0, phase);
                let selection = 5u8;
                native.world.scene_selection = Some(selection);
                source.bus.write8(0x1D73, selection);
                let projection = (u16::from(pilot) * 0x1357 + variant as u16 * 0x2468) as i16;
                native.world.published_camera_projection = Some(projection);
                source.bus.write16(0x1E3C, projection as u16);
                native.world.scene.active_shield = Some(if pilot & 1 == 0 { 80 } else { 7 });
                source
                    .bus
                    .write8(0x1DD1, native.world.scene.active_shield.unwrap());
                native.world.scene.player_configuration = Some(if pilot & 1 == 0 { 9 } else { 3 });
                native.world.scene.encounter_location = Some(if pilot & 2 == 0 { 2 } else { 5 });
                native.world.random = RandomState::new([pilot + 1, protection + 31, 0x30, 0xAB]);
                let records = native.world.player_mut(&native.objects, player).unwrap();
                records.protection = Some(DeflectionProtection::from_control(protection));
                records.auxiliary = Some(SelectedAuxiliaryState {
                    mode: 0x30,
                    action_flags: 0,
                    stored_world_position: Default::default(),
                    stored_rotation: Default::default(),
                });
                records.target_control = Some(Default::default());
                records.target_control.as_mut().unwrap().mode = if pilot & 1 == 0 { 8 } else { 7 };
                records.contact = Some(Default::default());
                records.contact.as_mut().unwrap().hit.reserve_shield =
                    if pilot & 2 == 0 { 80 } else { 0 };
                source.bus.write16(0x12C3, address(Some(player)));
                source.bus.write8(WRAM + SLOT + 0x6C02, protection);
                source.bus.write8(WRAM + SLOT + 0x6AA0, 0x30);
                source
                    .bus
                    .write16(WRAM + SLOT + 0x6C1C, if pilot & 1 == 0 { 8 } else { 7 });
                source
                    .bus
                    .write8(WRAM + SLOT + 0x6C00, if pilot & 2 == 0 { 80 } else { 0 });
                for (offset, value) in [
                    (0x1D72, 77),
                    (0x1CE5, 0xED),
                    (0x1DDF, 0xAD),
                    (0x1E14, pilot),
                    (0x1DE2, native.world.scene.player_configuration.unwrap()),
                ] {
                    source.bus.write8(offset, value);
                }
                source
                    .bus
                    .write16(0x1BB5, native.world.scene.encounter_location.unwrap());
                for (i, byte) in native.world.random.bytes().into_iter().enumerate() {
                    source.bus.write8(0xE0 + i as u32, byte);
                }
                let mut execution = SceneExecution::default();
                let mut callbacks = Callbacks;
                let mut cue_read = 0u16;
                let mut peak = 3;
                let mut removed = 0;
                for visit in 0..450u16 {
                    native.world.strategy_clock = visit;
                    source.bus.write16(0xC4, visit);
                    if visit == release {
                        native.world.action_gate.as_mut().unwrap().code = 0;
                        source.bus.write8(0x1D72, 0);
                    }
                    let mut pending = native.objects.active_ids().first().copied();
                    while let Some(id) = pending {
                        if id == player || id == view {
                            pending = native.objects.get(id).unwrap().base.next;
                            continue;
                        }
                        let camera = native.objects.get(view).unwrap();
                        let marker = CueMarker {
                            identity: CueListener::PrimaryFallback,
                            position: camera.base.position,
                            bearing: FixedViewAngles::capture(camera).heading(),
                        };
                        native.world.audio_routing = Some(AudioRouting {
                            listeners: [CueListener::PrimaryPlayer, CueListener::Other],
                            markers: Some(MarkerInputs {
                                selected_sides: [PlayerTarget::Primary; 2],
                                markers: [marker; 2],
                            }),
                        });
                        execution.controls.loop_listener =
                            Some(sf2_game::positional_audio::LoopListener {
                                position: marker.position,
                                bearing: marker.bearing,
                            });
                        source.run(0x7F3565, Some(0x7F357B), 0, address(Some(id)), true);
                        let path_before = native.objects.get(id).unwrap().base.path;
                        let attachment = native.objects.get(id).unwrap().extension.parent;
                        let mut host = SceneActors {
                            objects: &mut native.objects,
                            world: &mut native.world,
                            execution: &mut execution,
                            catalog: &catalog,
                            callbacks: &mut callbacks,
                            statement_budget: 256,
                        };
                        host.run_strategy(id, visit).unwrap_or_else(|error| panic!(
                            "pilot {pilot} protection {protection} release {release} visit {visit} actor {} path {path_before:?} parent {attachment:?} phase {phase}: {error:?}", id.index()));
                        pending = host.objects.get(id).unwrap().base.next;
                        super::special_exit_tests::compare_actor(&source, &native, id, visit);
                        for (i, byte) in native.world.random.bytes().into_iter().enumerate() {
                            assert_eq!(
                                source.bus.read8(0xE0 + i as u32),
                                byte,
                                "random visit {visit} actor {} byte {i}",
                                id.index()
                            );
                        }
                    }
                    peak = peak.max(native.objects.len());
                    for id in native.objects.active_ids().to_vec() {
                        if native.objects.get(id).unwrap().base.flags.remove_after_tick {
                            assert!(
                                call_near(
                                    &mut source.bus,
                                    0x7F335A,
                                    &Entry {
                                        x: address(Some(id)),
                                        dbr: 0x7E,
                                        p: 0x20,
                                        ..Default::default()
                                    }
                                )
                                .returned
                            );
                            let mut host = SceneActors {
                                objects: &mut native.objects,
                                world: &mut native.world,
                                execution: &mut execution,
                                catalog: &catalog,
                                callbacks: &mut callbacks,
                                statement_budget: 256,
                            };
                            host.retire_object(id).unwrap();
                            removed += 1;
                        }
                    }
                    assert_eq!(
                        source.bus.read8(0x1E14),
                        native.world.scene.active_pilot.unwrap(),
                        "active pilot visit {visit}"
                    );
                    if let Some(horizontal) = execution.paths.runtime.background_horizontal {
                        assert_eq!(
                            source.bus.read16(0x1E4E),
                            horizontal as u16,
                            "background horizontal visit {visit}"
                        );
                    }
                    if let Some(shadow) = execution.paths.runtime.background_scroll_shadow {
                        assert_eq!(
                            source.bus.read16(0x193A),
                            shadow,
                            "scroll shadow visit {visit}"
                        );
                    }
                    native.compare_pool(&source);
                    assert_eq!(
                        execution.paths.runtime.resources.available_capacity(),
                        source.available()
                    );
                    assert_eq!(
                        source.bus.read8(0x1D72),
                        native.world.action_gate.unwrap().code
                    );
                    assert_eq!(
                        source.bus.read8(0x1CE5),
                        native.world.engine_sound_control.unwrap().bits()
                    );
                    assert_eq!(
                        source.bus.read8(0x1DDF),
                        native.world.linked_effect_activity.unwrap().recent_spawn
                    );
                    assert_eq!(
                        source.bus.read16(0x1DFF),
                        address(native.world.camera_tracking.unwrap().actor)
                    );
                    assert_eq!(
                        source.bus.read16(0x1CE1),
                        native.world.audio.retained_scene_cue().map_or(0, encoded)
                    );
                    let camera = native.objects.get(view).unwrap();
                    for (offset, value) in [
                        (12, camera.base.position.x),
                        (14, camera.base.position.y),
                        (16, camera.base.position.z),
                    ] {
                        assert_eq!(
                            source.bus.read16(0x033F + offset),
                            value as u16,
                            "camera position visit {visit}"
                        );
                    }
                    let angles = FixedViewAngles::capture(camera);
                    for (offset, value) in [(18, angles.pitch), (20, angles.yaw), (22, angles.roll)]
                    {
                        assert_eq!(
                            source.bus.read16(0x033F + offset),
                            value,
                            "camera angle visit {visit}"
                        );
                    }
                    for event in native.world.audio.take_events().into_iter().flatten() {
                        let sf2_game::SoundEvent::Authored(cue) = event else {
                            panic!("non-authored cue")
                        };
                        assert_eq!(
                            source.bus.read16(0x1CF6 + u32::from(cue_read)),
                            encoded(cue),
                            "cue visit {visit}"
                        );
                        cue_read = (cue_read + 2) & 31;
                    }
                    assert_eq!(source.bus.read16(0x1D16), cue_read, "queue visit {visit}");
                }
                assert!(peak >= 8);
                assert!(removed >= 2);
                assert_eq!(native.world.action_gate.unwrap().code, 0);
                assert_eq!(
                    native.world.engine_sound_control.unwrap().bits(),
                    source.bus.read8(0x1CE5)
                );
                // The source strategy word is authoritative: PATHHOLD switches the
                // owner from path following (7E1E) to movement only (9DDE).
                let strategy = source.bus.read16(base + 0x19);
                assert_eq!(source.bus.read8(base + 0x1B), 0x7F);
                assert_eq!(
                    native.objects.get(owner).unwrap().base.behavior,
                    match strategy {
                        0x7E1E => Behavior::FollowPath,
                        0x9DDE => Behavior::PathMovement,
                        other => panic!("unexpected owner strategy {other:04X}"),
                    }
                );
            }
        }
    }
}

#[test]
fn pilot_advance_and_phase_branches_match_original_for_every_byte() {
    use sf2_game::path_commands::ControlCommand;
    use sf2_game::path_control::PlayerTarget;
    use sf2_game::path_invocation::InvocationWorld;
    use sf2_game::path_program::{PathCatalog, PhaseTest, Statement};
    use sf2_game::path_runtime::PathRuntime;
    use sf2_game::scene_path_world::ScenePathWorld;
    use sf2_game::{ObjectStore, PathCursor, PathId};
    let bytes = rom();
    let mut source = Source::new(&bytes, 0);
    let at = |index| PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    };
    for value in 0..=u8::MAX {
        // $07:F3D1: advance, mask to three bits, wrap values of six or more.
        source.bus.write8(0x1E14, value);
        source.run(0x07F3D1, None, 0, 0x03BD, true);
        let expected = source.bus.read8(0x1E14);
        let mut objects = ObjectStore::new();
        let owner = super::actor(&mut objects);
        objects.get_mut(owner).unwrap().base.path = Some(at(0));
        let mut world = ScenePathWorld::new(sf2_game::RandomState::default());
        world.scene.active_pilot = Some(value);
        let catalog = PathCatalog::new(vec![vec![
            Statement::AdvanceActivePilot { next: at(1) },
            Statement::Control(ControlCommand::Hold),
        ]])
        .unwrap();
        let mut runtime = PathRuntime::default();
        let mut borrowed = world
            .path_world(&objects, owner, PlayerTarget::Primary)
            .unwrap();
        let _ = runtime.enter_program(&catalog, &mut objects, owner, &mut borrowed, 2);
        assert_eq!(world.scene.active_pilot, Some(expected), "pilot {value}");

        // $09:B796 (BEQ) and $09:B869 (BPL) on the phase byte.
        for (test, entry) in [
            (PhaseTest::Equal(1), 0x09B796u32),
            (PhaseTest::DifferenceNonNegative(1), 0x09B869u32),
        ] {
            let mut objects = ObjectStore::new();
            let owner = super::actor(&mut objects);
            objects.get_mut(owner).unwrap().base.path = Some(at(0));
            let mut world = ScenePathWorld::new(sf2_game::RandomState::default());
            world.campaign_phase = Some(value);
            let catalog = PathCatalog::new(vec![vec![Statement::BranchOnCampaignPhase {
                test,
                taken: at(7),
                not_taken: at(3),
            }]])
            .unwrap();
            let mut runtime = PathRuntime::default();
            let mut borrowed = world
                .path_world(&objects, owner, PlayerTarget::Primary)
                .unwrap();
            let _ = runtime.enter_program(&catalog, &mut objects, owner, &mut borrowed, 1);
            // Native outcome: the taken arm.
            let native_taken = objects.get(owner).unwrap().base.path == Some(at(7));
            source.bus.write8(0x1BE0, value);
            // The inline body begins one byte after the path opcode and
            // returns the continuation in A; run it as the original does.
            let result = source.run(entry + 1, None, 0, 0x03BD, true);
            // Arm continuations: BEQ block returns B7A4/B7AB, BPL block B877/B87E.
            let second = if entry == 0x09B796 { 0xB7AB } else { 0xB87E };
            assert_eq!(
                result == second,
                native_taken,
                "phase {value} entry {entry:06X}"
            );
        }
    }
}

#[test]
fn random_seed_inline_matches_original_from_every_prior_state() {
    use sf2_game::path_control::PlayerTarget;
    use sf2_game::path_invocation::InvocationWorld;
    use sf2_game::path_program::{PathCatalog, Statement};
    use sf2_game::path_runtime::PathRuntime;
    use sf2_game::scene_path_world::ScenePathWorld;
    use sf2_game::{ObjectStore, PathCursor, PathId, RandomState};
    let bytes = rom();
    let mut source = Source::new(&bytes, 0);
    let at = |index| PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    };
    for seed in 0..=u8::MAX {
        let prior = [seed, seed.wrapping_mul(3), !seed, seed ^ 0x6D];
        for (i, byte) in prior.into_iter().enumerate() {
            source.bus.write8(0xE0 + i as u32, byte);
        }
        source.run(0x09B13D, None, 0, 0x03BD, true);
        let mut objects = ObjectStore::new();
        let owner = super::actor(&mut objects);
        objects.get_mut(owner).unwrap().base.path = Some(at(0));
        let mut world = ScenePathWorld::new(RandomState::new(prior));
        let catalog = PathCatalog::new(vec![vec![Statement::SeedRandom {
            bytes: [0x3A, 0xA7, 0x55, 0x7F],
            next: at(1),
        }]])
        .unwrap();
        let mut runtime = PathRuntime::default();
        let mut borrowed = world
            .path_world(&objects, owner, PlayerTarget::Primary)
            .unwrap();
        let _ = runtime.enter_program(&catalog, &mut objects, owner, &mut borrowed, 1);
        for (i, byte) in world.random.bytes().into_iter().enumerate() {
            assert_eq!(
                source.bus.read8(0xE0 + i as u32),
                byte,
                "seed {seed} byte {i}"
            );
        }
    }
}
