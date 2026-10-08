//! Entire indexed scene-six path (the attract flyover): spawned children,
//! the D767 shape hand-off, selected-player pose copies, the published
//! motion export and the action gate stepped as the action stream does.
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
        // No scene-six actor installs a death override: common death runs.
        Ok(None)
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
fn complete_scene_six_matches_original_children_shapes_player_copies_and_gate_steps() {
    let bytes = rom();
    let catalog = authored_paths::catalog();
    for pilot in [0u8, 3] {
        for protection in [0, 0x8F] {
            for (variant, release) in [u16::MAX, 300].into_iter().enumerate() {
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
                root.base.path = Some(authored_paths::SCENE_SIX);
                root.base.hit_points = 1;
                root.base.attack_power = 1;
                root.base.flags.reclaim_on_pool_pressure = false;
                root.extension.path_state.needs_path_initialization = true;
                for (offset, value) in [(0x2B, 0xFA11), (0x19, 0x7E1E)] {
                    source.bus.write16(base + offset, value);
                }
                for (offset, value) in [(0x1B, 0x7F), (0x20, 8), (0x2D, 1), (0x2E, 1)] {
                    source.bus.write8(base + offset, value);
                }
                native.world.primary_player = Some(player);
                native.world.encounter_signals =
                    Some(sf2_game::path_program::EncounterSignals { raised: 0x5A00 | u16::from(pilot) });
                source.bus.write16(WRAM + 0xD77D, 0x5A00 | u16::from(pilot));
                let motion = sf2_game::path_motion::PublishedPlayerMotion {
                    position: sf2_game::Vector3 { x: -397, y: 977, z: 317 },
                    delta: sf2_game::Vector3 { x: 31, y: -39, z: -13 },
                };
                native.world.published_motion = Some(motion);
                for (offset, value) in [
                    (0xD7EC, motion.position.x),
                    (0xD7EE, motion.position.y),
                    (0xD7F0, motion.position.z),
                    (0x1E1C, motion.delta.x),
                    (0x1E1E, motion.delta.y),
                    (0x1E20, motion.delta.z),
                ] {
                    source.bus.write16(WRAM + offset, value as u16);
                }
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
                let selection = 6u8;
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
                records.boundary = Some(Default::default());
                for offset in [0x6BED, 0x6BEF, 0x6BF1] {
                    source.bus.write16(WRAM + SLOT + offset, 0);
                }
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
                // The player allocation lives at a fixed source slot outside
                // the source pool; charge the native copy and offset totals.
                let before = execution.paths.runtime.resources.available_capacity();
                let actor = native.objects.get(player).unwrap();
                let storage = sf2_game::player_storage::PlayerStorage {
                    fine_pitch: u16::from(actor.base.pitch.units()) << 8,
                    fine_yaw: u16::from(actor.base.yaw.units()) << 8,
                    bank: actor.base.roll,
                    retained_shield: 0,
                };
                source.bus.write16(WRAM + SLOT + 0x6AB9, storage.fine_pitch);
                source.bus.write16(WRAM + SLOT + 0x6ABB, storage.fine_yaw);
                source.bus.write8(WRAM + SLOT + 0x6ABD, storage.bank.units());
                let resource = execution
                    .paths
                    .runtime
                    .resources
                    .allocate_owned(player, 472, sf2_game::program_state::ProgramData::PlayerStorage(storage))
                    .unwrap();
                // Records bind to the storage identity: rebind after it exists.
                let records = native.world.player(&native.objects, player).unwrap().clone();
                native.objects.get_mut(player).unwrap().base.player_storage = Some(resource);
                native.world.bind_player(&native.objects, player, records).unwrap();
                let storage_cost = before - execution.paths.runtime.resources.available_capacity();
                let mut callbacks = Callbacks;
                let mut cue_read = 0u16;
                let mut peak = 3;
                let mut removed = 0;
                for visit in 0..450u16 {
                    native.world.strategy_clock = visit;
                    source.bus.write16(0xC4, visit);
                    // The action stream's five gate steps ($0D:C82F); a
                    // second variant clears the gate early (closed-gate exit).
                    if [182, 249, 293, 327, 416].contains(&visit) || visit == release {
                        let gate = native.world.action_gate.as_mut().unwrap();
                        gate.code = if visit == release { 0 } else { gate.code.wrapping_add(1) };
                        source.bus.write8(0x1D72, gate.code);
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
                        // Death effects: the primary view marker; the
                        // secondary cue is suppressed by scene flag 1AA6 bit 02.
                        execution.controls.death_effects =
                            Some(sf2_game::common_destruction::EffectInputs {
                                spawn: native.world.spawn_defaults.unwrap(),
                                primary_marker: marker.position,
                                secondary_marker: None,
                            });
                        source.bus.write8(0x1AA6, source.bus.read8(0x1AA6) | 2);
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
                        source.bus.read16(WRAM + 0xD77D),
                        native.world.encounter_signals.unwrap().raised,
                        "encounter signals visit {visit}"
                    );
                    assert_eq!(
                        source.bus.read16(WRAM + 0x1E20),
                        native.world.published_motion.unwrap().delta.z as u16,
                        "motion export visit {visit}"
                    );
                    super::special_exit_tests::compare_actor(&source, &native, player, visit);
                    let records = native.world.player(&native.objects, player).unwrap();
                    let returned = records.boundary.unwrap().return_position;
                    for (offset, value) in [(0x6BED, returned.x), (0x6BEF, returned.y), (0x6BF1, returned.z)] {
                        assert_eq!(source.bus.read16(WRAM + SLOT + offset), value as u16, "return position visit {visit}");
                    }
                    let storage = sf2_game::player_storage::get(&native.objects, &execution.paths.runtime.resources, player);
                    if let Ok(storage) = storage {
                        assert_eq!(source.bus.read16(WRAM + SLOT + 0x6AB9), storage.fine_pitch, "fine pitch visit {visit}");
                        assert_eq!(source.bus.read16(WRAM + SLOT + 0x6ABB), storage.fine_yaw, "fine yaw visit {visit}");
                        assert_eq!(source.bus.read8(WRAM + SLOT + 0x6ABD), storage.bank.units(), "bank visit {visit}");
                    }
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
                        execution.paths.runtime.resources.available_capacity() + storage_cost,
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
                assert!(peak >= 8, "peak {peak}");
                let _ = removed;
                let _ = base;
            }
        }
    }
}
