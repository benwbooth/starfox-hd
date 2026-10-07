//! Full ordinary-exit graph and newborn traversal against original execution.
//! The player and camera are real independent owners, not replayed outputs.
use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_program::{ActionGate, SelectedAuxiliaryState};
use sf2_game::path_protection::{DeflectionProtection, LinkedEffectActivity};
use sf2_game::path_scene_state::{CameraTrackingTarget, EncounterHandoff};
use sf2_game::path_sound::{CueListener, CueMarker, MarkerInputs};
use sf2_game::player_engine_sound::EngineSoundControl;
use sf2_game::scene_path_world::AudioRouting;
use sf2_game::scene_strategy::{SceneActors, SceneCallbacks, SceneExecution};
use sf2_game::strategy_schedule::{StrategyCompletion, StrategyHost};
use sf2_game::view_transition::FixedViewAngles;
use sf2_game::{authored_paths, Behavior, ObjectId, RandomState};
use sf_oracle::{call_near, Entry};

const VIEW: u32 = 0x033F;
struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("ordinary-exit actors must execute real strategies")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected ordinary-exit death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected ordinary-exit map callback")
    }
}

use super::special_exit_tests::compare_actor;

#[test]
fn complete_ordinary_exit_matches_original_all_pilots_directions_and_shield_states() {
    let rom = rom();
    let catalog = authored_paths::catalog();
    for pilot in 0..6u8 {
        for protection in [0u8, 0xFF] {
            for direction in 0..8u8 {
                let mode = if pilot & 1 == 0 { 0x10 } else { 0x30 };
                let mut source = Source::new(&rom, 0);
                source.bus.enable_gsu();
                for (i, &byte) in rom[0x50000..0x54E00].iter().enumerate() {
                    source.bus.write8(0x7F7E00 + i as u32, byte);
                }
                source.run(0x7F1737, None, 0, OWNER, true);
                let mut native = Native::new(&mut source, 3, 87, 193, 0xEF73);
                let player = native.owner;
                let owner = native.objects.active_ids()[1];
                let view = native.objects.active_ids()[0];
                let base = u32::from(address(Some(owner)));
                let root = native.objects.get_mut(owner).unwrap();
                root.base.behavior = Behavior::FollowPath;
                root.base.path = Some(authored_paths::ORDINARY_SCENE_EXIT);
                root.base.hit_points = 1;
                root.base.attack_power = 1;
                root.base.flags.reclaim_on_pool_pressure = false;
                root.extension.path_state.needs_path_initialization = true;
                source.bus.write16(base + 0x2B, 0xCF18);
                source.bus.write16(base + 0x19, 0x7E1E);
                source.bus.write8(base + 0x1B, 0x7F);
                source.bus.write8(base + 0x20, 8);
                source.bus.write8(base + 0x2D, 1);
                source.bus.write8(base + 0x2E, 1);
                native.world.primary_player = Some(player);
                native.world.fixed_players[0] = Some(view);
                native.world.action_gate = Some(ActionGate { code: 0 });
                native.world.camera_tracking = Some(CameraTrackingTarget::default());
                native.world.engine_sound_control = Some(EngineSoundControl::from_bits(0xED));
                native.world.linked_effect_activity =
                    Some(LinkedEffectActivity { recent_spawn: 0xAD });
                native.world.handoff = Some(EncounterHandoff {
                    player_flags: 0xC0,
                    x: 1717,
                    z: -9123,
                    heading_word: u16::from_le_bytes([0x73, direction]),
                });
                native.world.scene.active_pilot = Some(pilot);
                native.world.scene.active_shield = Some(if pilot & 1 == 0 { 80 } else { 7 });
                source
                    .bus
                    .write8(0x1DD1, native.world.scene.active_shield.unwrap());
                native.world.scene.encounter_node_mode =
                    Some(if pilot & 2 == 0 { 0 } else { 0x80 });
                source.bus.write8(
                    WRAM + 0xD79B,
                    native.world.scene.encounter_node_mode.unwrap(),
                );
                native.world.scene.entry_heading = Some(pilot.wrapping_mul(43).wrapping_add(17));
                source
                    .bus
                    .write8(0x1BA9, native.world.scene.entry_heading.unwrap());
                native.world.scene.player_configuration = Some(if pilot & 1 == 0 { 9 } else { 3 });
                native.world.scene.encounter_location = Some(if pilot & 2 == 0 { 2 } else { 5 });
                native.world.random = RandomState::new([
                    pilot.wrapping_add(1),
                    protection.wrapping_add(31),
                    mode,
                    0xAB,
                ]);
                let player_state = native.world.player_mut(&native.objects, player).unwrap();
                player_state.protection = Some(DeflectionProtection::from_control(protection));
                player_state.auxiliary = Some(SelectedAuxiliaryState {
                    mode,
                    action_flags: 0,
                    stored_world_position: Default::default(),
                    stored_rotation: Default::default(),
                });
                player_state.target_control = Some(Default::default());
                player_state.target_control.as_mut().unwrap().mode =
                    if pilot & 1 == 0 { 8 } else { 7 };
                player_state.contact = Some(Default::default());
                player_state.contact.as_mut().unwrap().hit.reserve_shield =
                    if pilot & 2 == 0 { 80 } else { 0 };
                player_state.contact.as_mut().unwrap().hit.feedback_duration = 37;
                player_state.contact.as_mut().unwrap().hit.feedback_flags = 0x80;
                source.bus.write16(0x12C3, address(Some(player)));
                source.bus.write8(WRAM + SLOT + 0x6C02, protection);
                source.bus.write8(WRAM + SLOT + 0x6AA0, mode);
                source
                    .bus
                    .write16(WRAM + SLOT + 0x6C1C, if pilot & 1 == 0 { 8 } else { 7 });
                source
                    .bus
                    .write8(WRAM + SLOT + 0x6C00, if pilot & 2 == 0 { 80 } else { 0 });
                source.bus.write8(WRAM + SLOT + 0x6C11, 37);
                source.bus.write8(WRAM + SLOT + 0x6C12, 0x80);
                source.bus.write8(0x1CE5, 0xED);
                source.bus.write8(0x1DDF, 0xAD);
                source.bus.write8(0x1D74, 0xC0);
                source.bus.write16(0x1D88, 1717);
                source.bus.write16(0x1D8C, (-9123i16) as u16);
                source
                    .bus
                    .write16(0x1D8E, u16::from_le_bytes([0x73, direction]));
                source.bus.write8(0x1E14, pilot);
                source
                    .bus
                    .write8(0x1DE2, native.world.scene.player_configuration.unwrap());
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
                let mut shields = 0;
                for visit in 0..240u16 {
                    native.world.strategy_clock = visit;
                    source.bus.write16(0xC4, visit);
                    // The separately owned player action publishes gate three at
                    // decision time seventy. This fixture supplies that input;
                    // it does not claim the unported action continuation.
                    if visit == 70 {
                        native.world.action_gate.as_mut().unwrap().code = 3;
                        source.bus.write8(0x1D72, 3);
                    }
                    // Release any remaining shield after the controller settles.
                    if visit == 170 {
                        native.world.action_gate.as_mut().unwrap().code = 0;
                        source.bus.write8(0x1D72, 0);
                    }
                    let mut pending = native.objects.active_ids().first().copied();
                    while let Some(id) = pending {
                        if id == player || id == view {
                            pending = native.objects.get(id).unwrap().base.next;
                            continue;
                        }
                        let base = u32::from(address(Some(id)));
                        let camera = native.objects.get(view).unwrap();
                        let marker = CueMarker {
                            identity: CueListener::PrimaryFallback,
                            position: camera.base.position,
                            bearing: FixedViewAngles::capture(camera).heading(),
                        };
                        native.world.audio_routing = Some(AudioRouting {
                            listeners: [CueListener::PrimaryPlayer; 2],
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
                        source.run(0x7F3565, Some(0x7F357B), 0, base as u16, true);
                        let mut host = SceneActors {
                            objects: &mut native.objects,
                            world: &mut native.world,
                            execution: &mut execution,
                            catalog: &catalog,
                            callbacks: &mut callbacks,
                            statement_budget: 256,
                        };
                        host.run_strategy(id, visit).unwrap_or_else(|error| {
                            panic!("pilot {pilot} protection {protection} mode {mode} direction {direction} visit {visit} actor {}: {error:?}", id.index())
                        });
                        pending = host.objects.get(id).unwrap().base.next;
                        for (i, byte) in native.world.random.bytes().into_iter().enumerate() {
                            assert_eq!(
                                source.bus.read8(0xE0 + i as u32),
                                byte,
                                "random visit {visit} actor {} byte {i}",
                                id.index()
                            );
                        }
                        compare_actor(&source, &native, id, visit);
                    }
                    peak = peak.max(native.objects.len());
                    shields = shields.max(
                        native
                            .objects
                            .active_objects()
                            .filter(|(_, a)| a.base.behavior == Behavior::ExitShield)
                            .count(),
                    );
                    for id in native.objects.active_ids().to_vec() {
                        if native.objects.get(id).unwrap().base.flags.remove_after_tick {
                            let snapshot: Vec<_> = native
                                .objects
                                .active_objects()
                                .map(|(id, a)| {
                                    (
                                        id,
                                        a.base.attachment,
                                        a.base.attachment_next,
                                        a.extension.path_state.motion.attached_coordinates,
                                        a.extension.path_state.motion.refresh_child_chain,
                                        a.base.flags.remove_after_tick,
                                        source.bus.read16(u32::from(address(Some(id))) + 0x29),
                                    )
                                })
                                .collect();
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
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.retire_object(id)))
                                .unwrap_or_else(|_| panic!("retire pilot {pilot} protection {protection} direction {direction} visit {visit} actor {} snapshot {snapshot:?}", id.index())).unwrap();
                        }
                    }
                    native.compare_pool(&source);
                    assert_eq!(
                        execution.paths.runtime.resources.available_capacity(),
                        source.available()
                    );
                    for (i, byte) in native.world.random.bytes().into_iter().enumerate() {
                        assert_eq!(
                            source.bus.read8(0xE0 + i as u32),
                            byte,
                            "random visit {visit}"
                        );
                    }
                    for (offset, value) in [
                        (0x1D72, native.world.action_gate.unwrap().code),
                        (0x1D74, native.world.handoff.unwrap().player_flags),
                        (0x1CE5, native.world.engine_sound_control.unwrap().bits()),
                        (
                            0x1DDF,
                            native.world.linked_effect_activity.unwrap().recent_spawn,
                        ),
                    ] {
                        assert_eq!(
                            source.bus.read8(offset),
                            value,
                            "shared byte {offset:X} visit {visit}"
                        );
                    }
                    assert_eq!(
                        source.bus.read16(0x1DFF),
                        address(native.world.camera_tracking.unwrap().actor)
                    );
                    assert_eq!(
                        source.bus.read16(0x1E0B),
                        native.world.published_camera_roll.unwrap()
                    );
                    let hit = native
                        .world
                        .player(&native.objects, player)
                        .unwrap()
                        .contact
                        .unwrap()
                        .hit;
                    assert_eq!(
                        source.bus.read8(WRAM + SLOT + 0x6C11),
                        hit.feedback_duration
                    );
                    assert_eq!(source.bus.read8(WRAM + SLOT + 0x6C12), hit.feedback_flags);
                    let camera = native.objects.get(view).unwrap();
                    for (offset, value) in [
                        (12, camera.base.position.x),
                        (14, camera.base.position.y),
                        (16, camera.base.position.z),
                    ] {
                        assert_eq!(
                            source.bus.read16(VIEW + offset),
                            value as u16,
                            "view position {visit}"
                        );
                    }
                    let angles = FixedViewAngles::capture(camera);
                    for (offset, value) in [(18, angles.pitch), (20, angles.yaw), (22, angles.roll)]
                    {
                        assert_eq!(
                            source.bus.read16(VIEW + offset),
                            value,
                            "view angle {visit}"
                        );
                    }
                    for event in native.world.audio.take_events().into_iter().flatten() {
                        let sf2_game::SoundEvent::Authored(cue) = event else {
                            panic!("non-authored exit cue");
                        };
                        let encoded = u16::from(cue.id)
                            | (u16::from(cue.parameter()) << 8)
                            | if cue.target == PlayerTarget::Secondary {
                                0x8000
                            } else {
                                0
                            };
                        assert_eq!(
                            source.bus.read16(0x1CF6 + u32::from(cue_read)),
                            encoded,
                            "cue visit {visit}"
                        );
                        cue_read = (cue_read + 2) & 31;
                    }
                    assert_eq!(
                        source.bus.read16(0x1D16),
                        cue_read,
                        "cue cursor visit {visit}"
                    );
                }
                assert!(peak >= 8);
                assert_eq!(shields > 0, protection & 31 != 0);
                assert_eq!(
                    native.objects.get(owner).unwrap().base.behavior,
                    Behavior::PathMovement
                );
                assert_eq!(native.world.engine_sound_control.unwrap().bits(), 0xED);
                assert_eq!(native.world.handoff.unwrap().player_flags & 13, 13);
                assert!(native
                    .objects
                    .active_objects()
                    .all(|(_, a)| a.base.behavior != Behavior::ExitShield));
            }
        }
    }
}
