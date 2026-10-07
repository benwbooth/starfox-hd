//! Full special-exit graph and newborn traversal against original execution.
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
        panic!("special-exit actors must execute real strategies")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected special-exit death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected special-exit map callback")
    }
}

fn compare_actor(source: &Source, native: &Native, id: ObjectId, visit: u16) {
    let a = native.objects.get(id).unwrap();
    let base = u32::from(address(Some(id)));
    for (offset, actual) in [
        (12, a.base.position.x),
        (14, a.base.position.y),
        (16, a.base.position.z),
        (0x32, a.base.velocity.x),
        (0x34, a.base.velocity.y),
        (0x36, a.base.velocity.z),
        (0x1CCF, a.extension.relative_position.x),
        (0x1CD1, a.extension.relative_position.y),
        (0x1CD3, a.extension.relative_position.z),
    ] {
        assert_eq!(
            source.bus.read16(WRAM + base + offset) as i16,
            actual,
            "visit {visit} actor {} word {offset:X}",
            id.index()
        );
    }
    for (offset, actual) in [
        (18, a.base.pitch.units()),
        (20, a.base.yaw.units()),
        (22, a.base.roll.units()),
        (0x18, a.base.speed),
        (0x0A, a.base.target_speed),
        (0x13, a.base.child_number),
        (0x17, a.base.wait_timer),
        (0x2D, a.base.hit_points),
        (0x2E, a.base.attack_power),
        (0x15, a.extension.path_state.repeat_counter),
        (0x1CCB, a.extension.path_state.animation.shape.packed()),
        (0x1CCA, a.extension.path_state.animation.color.packed()),
        (0x1CD5, a.extension.relative_rotation.pitch.units()),
        (0x1CD6, a.extension.relative_rotation.yaw.units()),
        (0x1CD7, a.extension.relative_rotation.roll.units()),
    ] {
        assert_eq!(
            source.bus.read8(WRAM + base + offset),
            actual,
            "visit {visit} actor {} byte {offset:X}",
            id.index()
        );
    }
    for (offset, actual) in [
        (0x1CE2, a.extension.path_state.motion_phase),
        (0x1CE4, a.extension.path_state.script_value),
        (
            0x1CCD,
            a.extension
                .material_set
                .map_or(0, |material| material.catalog_token()),
        ),
    ] {
        assert_eq!(
            source.bus.read16(WRAM + base + offset),
            actual,
            "visit {visit} actor {} word {offset:X}",
            id.index()
        );
    }
    assert_eq!(
        source.bus.read16(base + 4),
        0xBC9C + a.base.shape.catalog_index() as u16 * 28
    );
    assert_eq!(
        source.bus.read16(WRAM + base + 0x1CD8),
        address(a.extension.parent)
    );
    for (offset, mask, actual) in [
        (0x21, 1, a.base.flags.collision_disabled),
        (0x23, 2, !a.base.flags.visible),
        (0x25, 8, a.base.flags.remove_after_tick),
        (0x26, 8, a.base.contacts.run_when_paused),
        (0x26, 16, a.base.flags.maximum_draw_distance),
        (9, 1, a.base.flags.far_sort_bias),
    ] {
        assert_eq!(
            source.bus.read8(base + offset) & mask != 0,
            actual,
            "visit {visit} actor {} flag {offset:X}:{mask:X}",
            id.index()
        );
    }
}

#[test]
fn clock_mask_branch_matches_original_all_byte_pairs_without_consuming_ifnot() {
    use sf2_game::path_commands::ControlCommand;
    use sf2_game::path_program::{PathCatalog, PathWorld, Statement};
    use sf2_game::path_runtime::PathRuntime;
    use sf2_game::{ObjectStore, PathCursor, PathId};
    let mut source = Source::new(&rom(), 0);
    let bytes = rom();
    for (i, &byte) in bytes[0x50000..0x54E00].iter().enumerate() {
        source.bus.write8(0x7F7E00 + i as u32, byte);
    }
    let mut objects = ObjectStore::new();
    let owner = super::actor(&mut objects);
    let cursor = |index| PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    };
    let mut runtime = PathRuntime::default();
    let mut random = RandomState::default();
    for inverted in [false, true] {
        for mask in 0..=255u8 {
            let catalog = PathCatalog::new(vec![vec![
                Statement::ClockBitsClear {
                    mask,
                    taken: cursor(2),
                    next: cursor(1),
                },
                Statement::Control(ControlCommand::Hold),
                Statement::Control(ControlCommand::Hold),
            ]])
            .unwrap();
            for clock in 0..=255u8 {
                source.bus.write16(0xF9, 0x1000);
                source.bus.write8(0xFB, 0x7E);
                source.bus.write8(0x1001, mask);
                source.bus.write16(0x1002, 0x7777);
                source.bus.write8(0xC4, clock);
                source.bus.write8(WRAM + 0xB272, u8::from(inverted));
                source.bus.write16(0x052B, 0x1000);
                source.run(0x7FBD06, Some(0x7F7E75), 0, 0x0500, true);
                runtime.branch.invert_next = inverted;
                objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
                let _ = runtime
                    .step_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut PathWorld::unbound(&mut random, clock),
                    )
                    .unwrap();
                assert_eq!(
                    source.bus.read16(0x052B),
                    if objects.get(owner).unwrap().base.path == Some(cursor(2)) {
                        0x7777
                    } else {
                        0x1004
                    },
                    "clock {clock} mask {mask}"
                );
                assert_eq!(
                    source.bus.read8(WRAM + 0xB272),
                    u8::from(runtime.branch.invert_next)
                );
                assert_eq!(runtime.branch.invert_next, inverted);
            }
        }
    }
}

#[test]
fn complete_special_exit_matches_original_all_pilots_primary_shield_states_and_both_sound_branches()
{
    let rom = rom();
    let catalog = authored_paths::catalog();
    for pilot in 0..6u8 {
        for protection in [0u8, 0x80, 0xFF] {
            for mode in [0x10, 0x30] {
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
                root.base.path = Some(authored_paths::SPECIAL_SCENE_EXIT);
                root.base.hit_points = 1;
                root.base.attack_power = 1;
                root.base.flags.reclaim_on_pool_pressure = false;
                root.extension.path_state.needs_path_initialization = true;
                source.bus.write16(base + 0x2B, 0xD27B);
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
                    player_flags: 0xC5,
                    x: 1717,
                    z: -9123,
                    heading_word: 0xEA73,
                });
                native.world.scene.active_pilot = Some(pilot);
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
                source.bus.write8(0x1D74, 0xC5);
                source.bus.write16(0x1D88, 1717);
                source.bus.write16(0x1D8C, (-9123i16) as u16);
                source.bus.write16(0x1D8E, 0xEA73);
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
                for visit in 0..200u16 {
                    native.world.strategy_clock = visit;
                    source.bus.write16(0xC4, visit);
                    // An external owner releases the gate after the authored
                    // controller has held it at two for many retained visits.
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
                        if visit == 96 && id == owner {
                            source.byte_accesses = Some((0x25, Vec::new()));
                        }
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
                            panic!("pilot {pilot} protection {protection} mode {mode} visit {visit} actor {}: {error:?}", id.index())
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
                // Source's missing child-five retirement really writes the
                // null-index scratch byte. The full continuation never reads
                // that write; no synthetic child is inserted to hide it.
                let (_, accesses) = source.byte_accesses.as_ref().unwrap();
                let store = accesses
                    .iter()
                    .position(|(write, _)| *write)
                    .expect("original null-child scratch store");
                assert_ne!(accesses[store].1 & 8, 0);
                assert!(
                    accesses[store + 1..].iter().all(|(write, _)| *write),
                    "retirement scratch became observable: {accesses:?}"
                );
                assert_eq!(shields, usize::from(protection & 31 != 0));
                assert_eq!(
                    native.objects.get(owner).unwrap().base.behavior,
                    Behavior::PathMovement
                );
                assert_eq!(native.world.engine_sound_control.unwrap().bits(), 8);
                assert!(native
                    .objects
                    .active_objects()
                    .all(|(_, a)| a.base.behavior != Behavior::ExitShield));
            }
        }
    }
}
