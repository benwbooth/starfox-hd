//! Exit-controller fixed-view commands against original handlers and math.
//! Only operands/initial state are synthetic; every handler executes unchanged.
use super::{actor, rom, Source, WRAM};
use sf2_game::path_commands::{ControlCommand, ControlStep};
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_invocation::InvocationWorld;
use sf2_game::path_program::{PathCatalog, Statement};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::path_scene_state::CameraTrackingTarget;
use sf2_game::scene_path_world::ScenePathWorld;
use sf2_game::view_transition::{FixedViewAngles, FixedViewCommand};
use sf2_game::{ObjectId, ObjectStore, PathCursor, PathId, RandomState, Vector3};

const OWNER: u16 = 0x0500;
const VIEW: u16 = 0x033F;
const TARGET: u16 = 0x0600;

fn cursor(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    }
}

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    runtime: PathRuntime,
    owner: ObjectId,
    view: ObjectId,
    target: ObjectId,
}

impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let view = actor(&mut objects);
        let target = actor(&mut objects);
        let mut world = ScenePathWorld::new(RandomState::default());
        world.fixed_players[0] = Some(view);
        world.camera_tracking = Some(CameraTrackingTarget {
            actor: Some(target),
        });
        Self {
            objects,
            world,
            runtime: PathRuntime::default(),
            owner,
            view,
            target,
        }
    }

    fn run(
        &mut self,
        source: &mut Source,
        command: FixedViewCommand,
        owner_alias: bool,
        target_alias: bool,
    ) {
        let owner = if owner_alias { self.view } else { self.owner };
        let target = if target_alias { self.view } else { self.target };
        self.world.camera_tracking.as_mut().unwrap().actor = Some(target);
        for (id, address) in [
            (self.owner, OWNER),
            (self.view, VIEW),
            (self.target, TARGET),
        ] {
            let a = self.objects.get(id).unwrap();
            for (offset, value) in [
                (12, a.base.position.x),
                (14, a.base.position.y),
                (16, a.base.position.z),
            ] {
                source
                    .bus
                    .write16(u32::from(address) + offset, value as u16);
            }
            let angles = FixedViewAngles::capture(a);
            for (offset, value) in [(18, angles.pitch), (20, angles.yaw), (22, angles.roll)] {
                source.bus.write16(u32::from(address) + offset, value);
            }
        }
        source
            .bus
            .write16(0x1DFF, if target_alias { VIEW } else { TARGET });
        source.bus.write8(0x149D, 0xED);
        self.runtime.steering.unchanged_axes = 0xED;
        source.bus.write16(0xF9, 0x1000);
        source.bus.write8(0xFB, 0x7E);
        source.bus.write16(0x1001, VIEW);
        let (entry, stop) = match command {
            FixedViewCommand::CopyPosition => (0x7FBFF6, 0x7FCABE),
            FixedViewCommand::ChasePosition => (0x7FC028, 0x7FCABE),
            FixedViewCommand::CopyRotation => (0x7FC005, 0x7FCABE),
            FixedViewCommand::CopyRotationFromView => (0x7FC0BF, 0x7FCAE8),
            FixedViewCommand::ChaseRotation => (0x7FC069, 0x7FCABE),
            FixedViewCommand::SetYawWord(_) => (0x09BD8D, 0x09BD9F),
            FixedViewCommand::EaseYawTowardThreeQuarterTurn => (0x07F52B, 0x07F54D),
            FixedViewCommand::AimTracking { pitch_shift, chase } => {
                source.bus.write8(0x1003, pitch_shift);
                (if chase { 0x7FBE8C } else { 0x7FBE38 }, 0x7FCAA9)
            }
        };
        source.run(
            entry,
            Some(stop),
            0,
            if owner_alias { VIEW } else { OWNER },
            true,
        );
        self.objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
        let catalog = PathCatalog::new(vec![vec![
            Statement::FixedView {
                command,
                next: cursor(1),
            },
            Statement::Control(ControlCommand::Hold),
        ]])
        .unwrap();
        let mut world = self
            .world
            .path_world(&self.objects, owner, PlayerTarget::Primary)
            .unwrap();
        assert_eq!(
            self.runtime
                .enter_program(&catalog, &mut self.objects, owner, &mut world, 4)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        for (id, address) in [
            (self.owner, OWNER),
            (self.view, VIEW),
            (self.target, TARGET),
        ] {
            let a = self.objects.get(id).unwrap();
            for (offset, value) in [
                (12, a.base.position.x),
                (14, a.base.position.y),
                (16, a.base.position.z),
            ] {
                assert_eq!(
                    source.bus.read16(u32::from(address) + offset),
                    value as u16,
                    "{command:?} position {address:04X}+{offset}"
                );
            }
            let angles = FixedViewAngles::capture(a);
            for (offset, value) in [(18, angles.pitch), (20, angles.yaw), (22, angles.roll)] {
                assert_eq!(
                    source.bus.read16(u32::from(address) + offset),
                    value,
                    "{command:?} angle {address:04X}+{offset}"
                );
            }
        }
        assert_eq!(
            source.bus.read8(0x149D),
            self.runtime.steering.unchanged_axes
        );
    }
}

#[test]
fn exit_view_copy_and_tracking_aim_match_original_including_aliases_and_wrapped_geometry() {
    let rom = rom();
    let mut source = Source::new(&rom, 0);
    for (index, &byte) in rom[0x50000..0x54E00].iter().enumerate() {
        source.bus.write8(0x7F7E00 + index as u32, byte);
    }
    let mut f = Fixture::new();
    let mut random = 0xBE73_5D19u32;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random as u16
    };
    for case in 0..16_384 {
        for id in [f.owner, f.view, f.target] {
            let actor = f.objects.get_mut(id).unwrap();
            actor.base.position = Vector3 {
                x: next() as i16,
                y: next() as i16,
                z: next() as i16,
            };
            FixedViewAngles {
                pitch: next(),
                yaw: next(),
                roll: next(),
            }
            .write_to(actor);
        }
        let command = match case % 8 {
            0 => FixedViewCommand::CopyPosition,
            5 => FixedViewCommand::CopyRotationFromView,
            6 => FixedViewCommand::ChaseRotation,
            7 => FixedViewCommand::SetYawWord(0x8000),
            1 => FixedViewCommand::CopyRotation,
            4 => FixedViewCommand::ChasePosition,
            mode => FixedViewCommand::AimTracking {
                pitch_shift: (case / 4) as u8,
                chase: mode == 3,
            },
        };
        f.run(&mut source, command, case & 0x400 != 0, case & 0x800 != 0);
    }
    // Every source roll word including the half-turn tie; fixed aim still
    // quarter-chases roll instead of snapping it to zero.
    for roll in 0..=u16::MAX {
        FixedViewAngles {
            pitch: 0,
            yaw: 0,
            roll,
        }
        .write_to(f.objects.get_mut(f.view).unwrap());
        f.run(
            &mut source,
            FixedViewCommand::AimTracking {
                pitch_shift: 1,
                chase: false,
            },
            false,
            false,
        );
    }
    assert_eq!(source.bus.read16(WRAM + 0x1001), VIEW);
    // Every wrapped position delta, including the half-turn tie. This form
    // chases Z only once and never alters the view's fine-angle aliases.
    for word in 0..=u16::MAX {
        f.objects.get_mut(f.view).unwrap().base.position = Vector3::default();
        f.objects.get_mut(f.owner).unwrap().base.position = Vector3 {
            x: word as i16,
            y: word.rotate_left(5) as i16,
            z: word.wrapping_neg() as i16,
        };
        f.run(&mut source, FixedViewCommand::ChasePosition, false, false);
    }
    // Every yaw word through the three-quarter-turn ease (`$07:F52B`),
    // with pitch/roll carrying distinct values that must be left alone.
    for yaw in 0..=u16::MAX {
        FixedViewAngles {
            pitch: yaw.rotate_left(3),
            yaw,
            roll: !yaw,
        }
        .write_to(f.objects.get_mut(f.view).unwrap());
        f.run(
            &mut source,
            FixedViewCommand::EaseYawTowardThreeQuarterTurn,
            false,
            false,
        );
    }
}

#[test]
fn node_exit_wrap_and_view_damping_match_original_for_every_coordinate_and_yaw_word() {
    let rom = rom();
    let mut source = Source::new(&rom, 0);
    let mut f = Fixture::new();
    let catalog = PathCatalog::new(vec![vec![
        Statement::InitializeNodeExitCamera { next: cursor(1) },
        Statement::DampNodeExitCamera { next: cursor(2) },
        Statement::Control(ControlCommand::Hold),
    ]])
    .unwrap();
    for word in 0..=u16::MAX {
        let position = Vector3 {
            x: word as i16,
            y: word.rotate_left(5) as i16,
            z: word.wrapping_neg() as i16,
        };
        let initial = FixedViewAngles {
            pitch: word.rotate_right(3),
            yaw: word,
            roll: !word,
        };
        let owner = f.objects.get_mut(f.owner).unwrap();
        owner.base.position = position;
        FixedViewAngles {
            pitch: 0xCD12,
            yaw: 0xB934,
            roll: 0x8756,
        }
        .write_to(owner);
        owner.base.path = Some(cursor(0));
        for (offset, value) in [(12, position.x), (14, position.y), (16, position.z)] {
            source.bus.write16(u32::from(OWNER) + offset, value as u16);
        }
        for (offset, value) in [(18, 0xCD12), (20, 0xB934), (22, 0x8756)] {
            source.bus.write16(u32::from(OWNER) + offset, value);
        }
        initial.write_to(f.objects.get_mut(f.view).unwrap());
        for (offset, value) in [(18, initial.pitch), (20, initial.yaw), (22, initial.roll)] {
            source.bus.write16(u32::from(VIEW) + offset, value);
        }
        source.bus.write16(0x1E52, word);
        f.world.camera_projection_offset = Some(word as i16);
        source.run(0x06FA66, None, 0, OWNER, true);
        source.run(0x07F501, None, 0, OWNER, true);
        let mut world = f
            .world
            .path_world(&f.objects, f.owner, PlayerTarget::Primary)
            .unwrap();
        assert_eq!(
            f.runtime
                .enter_program(&catalog, &mut f.objects, f.owner, &mut world, 4)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let owner = f.objects.get(f.owner).unwrap();
        for (offset, value) in [
            (12, owner.base.position.x),
            (14, owner.base.position.y),
            (16, owner.base.position.z),
        ] {
            assert_eq!(source.bus.read16(u32::from(OWNER) + offset), value as u16);
        }
        for (id, address) in [(f.owner, OWNER), (f.view, VIEW)] {
            let actual = FixedViewAngles::capture(f.objects.get(id).unwrap());
            for (offset, value) in [(18, actual.pitch), (20, actual.yaw), (22, actual.roll)] {
                assert_eq!(
                    source.bus.read16(u32::from(address) + offset),
                    value,
                    "word {word} actor {address:X} field {offset}"
                );
            }
        }
        assert_eq!(
            source.bus.read16(0x1E52),
            f.world.camera_projection_offset.unwrap() as u16
        );
    }
}

struct Callbacks;
impl sf2_game::scene_strategy::SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<sf2_game::strategy_schedule::StrategyCompletion, Self::Error> {
        panic!("exit actors must execute their real authored strategies")
    }
    fn death_override(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<sf2_game::strategy_schedule::StrategyCompletion>, Self::Error> {
        panic!("unexpected exit death override")
    }
    fn resume_map_on_death(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<(), Self::Error> {
        panic!("unexpected exit death map callback")
    }
}

#[test]
fn complete_node_exit_keeps_original_camera_children_callbacks_and_map_continuation() {
    use super::surface_particle_tests::{address, Native};
    use sf2_game::authored_paths;
    use sf2_game::path_program::ActionGate;
    use sf2_game::path_radio::RadioLayout;
    use sf2_game::path_scene_state::{
        EncounterCoordination, EncounterObjectiveCounts, SceneEventFlags,
    };
    use sf2_game::path_sound::CueListener;
    use sf2_game::scene_map::{MapCatalog, MapCursor, MapInstruction, SceneMap};
    use sf2_game::scene_path_world::AudioRouting;
    use sf2_game::scene_strategy::{SceneActors, SceneExecution};
    use sf2_game::strategy_schedule::StrategyHost;
    use sf2_game::view_transition::ViewTransitionMode;
    use sf_oracle::{call_near, Entry};
    let rom = rom();
    let catalog = authored_paths::catalog();
    let map_program: [MapInstruction<(), ()>; 2] = [MapInstruction::Stop, MapInstruction::Stop];
    let map_catalog = MapCatalog::new(&map_program, &[]).unwrap();
    for gate in [0u8, 1, 0x80, 0xFF] {
        for variant in 0..4u8 {
            let mut source = Source::new(&rom, 0);
            source.bus.enable_gsu();
            for (index, &byte) in rom[0x50000..0x54E00].iter().enumerate() {
                source.bus.write8(0x7F7E00 + index as u32, byte);
            }
            source.run(
                0x7F1737,
                None,
                0,
                super::surface_particle_tests::OWNER,
                true,
            );
            let mut native = Native::new(&mut source, 2, 87, 193, 0xEF73);
            let ids = native.objects.active_ids().to_vec();
            let player = native.owner;
            let view = ids[0];
            native
                .objects
                .get_mut(player)
                .unwrap()
                .extension
                .texture_scroll_x = 0;
            source
                .bus
                .write8(WRAM + u32::from(address(Some(player))) + 0x1CDA, 0);
            native.world.objective_counts = Some(EncounterObjectiveCounts {
                remaining_word: 0xAB01,
                ..Default::default()
            });
            native.world.node_exit = sf2_game::player_node_exit::NodeExitState {
                presentation_flags: Some(0xA7),
                completion_code: Some(u8::from(gate == 0)),
            };
            source.bus.write16(WRAM + 0xD7F4, 0xAB01);
            source.bus.write8(0x1E08, 0xA7);
            source.bus.write8(0x1E17, u8::from(gate == 0));
            source.run(
                0x06A045,
                Some(0x06A0A5),
                0,
                super::surface_particle_tests::OWNER,
                true,
            );
            let mut execution = SceneExecution::default();
            let mut callbacks = Callbacks;
            let owner = SceneActors {
                objects: &mut native.objects,
                world: &mut native.world,
                execution: &mut execution,
                catalog: &catalog,
                callbacks: &mut callbacks,
                statement_budget: 256,
            }
            .advance_player_node_exit()
            .unwrap()
            .unwrap();
            let root_address = u32::from(address(Some(owner)));
            assert_eq!(source.bus.read16(root_address + 0x2B), 0xB8C5);
            assert_eq!(source.bus.read16(root_address + 0x19), 0x7E1E);
            assert_eq!(source.bus.read8(root_address + 0x1B), 0x7F);
            super::special_exit_tests::compare_actor(&source, &native, owner, 0);
            native.compare_pool(&source);
            let position = Vector3 {
                x: -31111,
                y: 29876,
                z: -17777,
            };
            native.objects.get_mut(view).unwrap().base.position = position;
            let angles = FixedViewAngles {
                pitch: 0xCBA7,
                yaw: 0x7FFF,
                roll: 0xEDA2,
            };
            angles.write_to(native.objects.get_mut(view).unwrap());
            for (offset, value) in [(12, position.x), (14, position.y), (16, position.z)] {
                source.bus.write16(u32::from(VIEW) + offset, value as u16);
            }
            for (offset, value) in [(18, angles.pitch), (20, angles.yaw), (22, angles.roll)] {
                source.bus.write16(u32::from(VIEW) + offset, value);
            }
            native.world.primary_player = Some(player);
            native.world.fixed_players[0] = Some(view);
            native.world.camera_tracking = Some(CameraTrackingTarget::default());
            native.world.action_gate = Some(ActionGate { code: gate });
            native.world.scene.node_presentation_variant = Some(variant);
            native.world.scene.player_configuration = Some(3);
            native.world.scene.encounter_location = Some(5);
            native.world.scene_events = Some(SceneEventFlags::default());
            native.world.coordination = Some(EncounterCoordination::default());
            native.world.view_transition_mode = Some(ViewTransitionMode::default());
            native.world.audio_routing = Some(AudioRouting {
                listeners: [CueListener::PrimaryPlayer; 2],
                markers: None,
            });
            native.world.radio = Some((
                Default::default(),
                RadioLayout {
                    compact_panel: false,
                    tracked_screen_y: 80,
                },
            ));
            let mut map = SceneMap::new(&map_catalog, MapCursor::from_index(0)).unwrap();
            map.save_continuation(&map_catalog, MapCursor::from_index(1))
                .unwrap();
            native.world.map = Some(map);
            source.bus.write16(0x12C3, address(Some(player)));
            source.bus.write8(0x1D72, gate);
            source.bus.write8(0x1E09, variant);
            source.bus.write8(0x1DE2, 3);
            source.bus.write16(0x1BB5, 5);
            source.bus.write8(0x1D77, 0x04);
            source.bus.write16(0x1D78, 0xCAFE);
            source.bus.write8(0x192E, 0x06);
            source.bus.write16(0x1657, 0xFACE);
            source.bus.write8(0x1E31, 80);
            let mut root_retired = false;
            let mut most_actors = native.objects.len();
            let mut cue_read = 0u16;
            for visit in 0..160u16 {
                native.world.strategy_clock = visit;
                source.bus.write16(0xC4, visit);
                // Follow the live successor after each visit. New children
                // inserted after the current actor run during this same pass.
                let mut pending = native.objects.active_ids().first().copied();
                while let Some(id) = pending {
                    if id == player {
                        source.run(
                            0x06A045,
                            Some(0x06A0A5),
                            0,
                            super::surface_particle_tests::OWNER,
                            true,
                        );
                        let mut host = SceneActors {
                            objects: &mut native.objects,
                            world: &mut native.world,
                            execution: &mut execution,
                            catalog: &catalog,
                            callbacks: &mut callbacks,
                            statement_budget: 256,
                        };
                        assert_eq!(host.advance_player_node_exit().unwrap(), None);
                        assert_eq!(
                            source.bus.read8(0x1E08),
                            host.world.node_exit.presentation_flags.unwrap()
                        );
                        assert_eq!(
                            source.bus.read16(WRAM + 0xD7F4),
                            host.world.objective_counts.unwrap().remaining_word
                        );
                    }
                    if id == player || id == view {
                        pending = native.objects.get(id).unwrap().base.next;
                        continue;
                    }
                    let base = u32::from(address(Some(id)));
                    // Include the outer suspension gate and immediate-retire
                    // reset, then stop at the next-actor selection boundary.
                    source.run(0x7F3565, Some(0x7F357B), 0, base as u16, true);
                    let listener = native.objects.get(view).unwrap();
                    execution.controls.loop_listener =
                        Some(sf2_game::positional_audio::LoopListener {
                            position: listener.base.position,
                            bearing: FixedViewAngles::capture(listener).heading(),
                        });
                    let mut host = SceneActors {
                        objects: &mut native.objects,
                        world: &mut native.world,
                        execution: &mut execution,
                        catalog: &catalog,
                        callbacks: &mut callbacks,
                        statement_budget: 256,
                    };
                    host.run_strategy(id, visit).unwrap();
                    let a = host.objects.get(id).unwrap();
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
                            "gate {gate} variant {variant} visit {visit} actor {} field {offset:X}",
                            id.index()
                        );
                    }
                    for (offset, actual) in [
                        (0x12, a.base.pitch.units()),
                        (0x14, a.base.yaw.units()),
                        (0x16, a.base.roll.units()),
                        (0x18, a.base.speed),
                        (0x13, a.base.child_number),
                        (0x17, a.base.wait_timer),
                        (0x2D, a.base.hit_points),
                        (0x2E, a.base.attack_power),
                    ] {
                        assert_eq!(
                            source.bus.read8(base + offset),
                            actual,
                            "gate {gate} variant {variant} visit {visit} actor {} field {offset:X}",
                            id.index()
                        );
                    }
                    assert_eq!(
                        source.bus.read16(base + 4),
                        0xBC9C + a.base.shape.catalog_index() as u16 * 28
                    );
                    assert_eq!(
                        source.bus.read8(base + 0x25) & 8 != 0,
                        a.base.flags.remove_after_tick
                    );
                    for (offset, actual) in [
                        (0x1CE2, a.extension.path_state.motion_phase),
                        (0x1CE4, a.extension.path_state.script_value),
                    ] {
                        assert_eq!(source.bus.read16(WRAM + base + offset), actual);
                    }
                    for (offset, actual) in [
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
                            "gate {gate} variant {variant} visit {visit} actor {} byte {offset:X}",
                            id.index()
                        );
                    }
                    pending = a.base.next;
                    assert_eq!(source.bus.read16(base), address(pending));
                }
                // End marks deferred removal. The real cleanup pass follows
                // all strategies; descendants must still see their parent's
                // final pose until that pass reaches the parent and children.
                let retired = native.objects.active_ids().to_vec();
                for id in retired {
                    if native.objects.get(id).unwrap().base.flags.remove_after_tick {
                        let base = u32::from(address(Some(id)));
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
                        let mut host = SceneActors {
                            objects: &mut native.objects,
                            world: &mut native.world,
                            execution: &mut execution,
                            catalog: &catalog,
                            callbacks: &mut callbacks,
                            statement_budget: 256,
                        };
                        host.retire_object(id).unwrap();
                        root_retired |= id == owner;
                    }
                }
                most_actors = most_actors.max(native.objects.len());
                native.compare_pool(&source);
                assert_eq!(
                    execution.paths.runtime.resources.available_capacity(),
                    source.available()
                );
                let camera = native.objects.get(view).unwrap();
                for (offset, actual) in [
                    (12, camera.base.position.x),
                    (14, camera.base.position.y),
                    (16, camera.base.position.z),
                ] {
                    assert_eq!(
                        source.bus.read16(u32::from(VIEW) + offset),
                        actual as u16,
                        "camera position visit {visit}"
                    );
                }
                let angles = FixedViewAngles::capture(camera);
                for (offset, actual) in [(18, angles.pitch), (20, angles.yaw), (22, angles.roll)] {
                    assert_eq!(
                        source.bus.read16(u32::from(VIEW) + offset),
                        actual,
                        "camera angle visit {visit}"
                    );
                }
                assert_eq!(
                    source.bus.read16(0x1E44),
                    native.world.camera_projection_base.unwrap() as u16
                );
                if let Some(offset) = native.world.camera_projection_offset {
                    assert_eq!(source.bus.read16(0x1E52), offset as u16);
                }
                assert_eq!(
                    native.world.map.as_ref().unwrap().cursor().index(),
                    usize::from(gate != 0)
                );
                assert_eq!(
                    source.bus.read16(0x1657),
                    if gate == 0 { 0xFACE } else { 0xCAFE }
                );
                assert_eq!(source.bus.read8(0x192E), if gate == 0 { 6 } else { 4 });
                assert_eq!(
                    source.bus.read16(0x1B84),
                    native.world.view_transition_mode.unwrap().flags
                );
                assert_eq!(
                    source.bus.read8(WRAM + 0xD7D5),
                    native.world.coordination.unwrap().transition_ready
                );
                assert_eq!(
                    source.bus.read16(0x1DFF),
                    address(native.world.camera_tracking.unwrap().actor)
                );
                assert_eq!(source.bus.read8(0x1CDA), 2);
                assert_eq!(
                    native.world.audio.pending_music_control(),
                    Some(sf2_game::path_sound::MusicControlRequest::EncounterExit)
                );
                let (radio, _) = native.world.radio.unwrap();
                assert_eq!(
                    source.bus.read8(WRAM + 0xCF31) as usize,
                    radio.message.index()
                );
                assert_eq!(source.bus.read8(WRAM + 0xCF32) != 0, radio.pending);
                assert_eq!(source.bus.read16(WRAM + 0xD744), radio.panel_y);
                assert_eq!(source.bus.read8(WRAM + 0xD759) != 0, radio.top_placement);
                for event in native.world.audio.take_events().into_iter().flatten() {
                    let sf2_game::SoundEvent::Authored(cue) = event else {
                        panic!("non-authored exit cue")
                    };
                    let encoded = u16::from(cue.id)
                        | (u16::from(cue.parameter()) << 8)
                        | if cue.target == PlayerTarget::Secondary {
                            0x8000
                        } else {
                            0
                        };
                    assert_eq!(source.bus.read16(0x1CF6 + u32::from(cue_read)), encoded);
                    cue_read = (cue_read + 2) & 31;
                }
                assert_eq!(source.bus.read16(0x1D16), cue_read);
            }
            assert!(root_retired);
            assert!(most_actors >= 5, "both child generations must run");
            assert_eq!(
                native.objects.len(),
                2,
                "the complete attachment tree retires"
            );
            let selected = native.objects.get(player).unwrap();
            for (offset, actual) in [
                (18, selected.base.pitch.units()),
                (20, selected.base.yaw.units()),
                (22, selected.base.roll.units()),
            ] {
                assert_eq!(
                    source.bus.read8(u32::from(address(Some(player))) + offset),
                    actual
                );
            }
        }
    }
}
