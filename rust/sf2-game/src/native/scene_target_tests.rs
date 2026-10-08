use super::*;
use crate::path_commands::{ControlCommand, ControlStep};
use crate::path_control::PlayerTarget;
use crate::path_invocation::{InvocationEntry, InvocationError, InvocationWorld, PathInvocation};
use crate::path_program::{PathCatalog, ProgramError, Statement};
use crate::player_storage::{self, PlayerScore, PlayerStorageInputs};
use crate::scene_path_world::{PlayerPathRecords, ScenePathWorld, WorldInputError};
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::view_transition::FixedViewAngles;
use crate::{Behavior, Object, ObjectKind, ObjectStore, PathCursor, PathId, RandomState, ShapeId};

fn actor(objects: &mut ObjectStore) -> ObjectId {
    objects
        .allocate(Object::new(
            ObjectKind::Enemy,
            ShapeId::EMPTY,
            Behavior::FollowPath,
        ))
        .unwrap()
}

fn initial(owner: ObjectId, flags: u8) -> TargetSelection {
    TargetSelection {
        display_status: 213,
        control_flags: flags,
        forced_owner: Some(owner),
        candidate: Some(owner),
        distance: 17439,
        auxiliary_distance: 9021,
        position: Vector3 {
            x: 31291,
            y: -103,
            z: -27653,
        },
        pitch: 35791,
        yaw: 13967,
        screen: [109, 241],
        clipped_yaw: 173,
        compass_position: Vector3 { x: 181, y: -191, z: 193 },
        compass_distance: 197,
    }
}

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

fn storage(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    paths: &mut PathInvocation,
    owner: ObjectId,
) {
    player_storage::replace(
        objects,
        world,
        &mut paths.runtime,
        owner,
        PlayerStorageInputs {
            pilot_code: 2,
            reserve_shield: 31,
            score: PlayerScore::default(),
        },
    )
    .unwrap();
}

#[test]
fn initialization_preserves_all_nonreset_fields_and_never_clears_existing_control_bits() {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    for flags in 0..=u8::MAX {
        for reflection in [false, true] {
            let mut value = initial(owner, flags);
            let mut expected = value;
            expected.candidate = None;
            expected.position = Vector3::default();
            expected.auxiliary_distance = u16::MAX;
            expected.control_flags |= if reflection { 0x40 } else { 0xC0 };
            value.initialize(reflection);
            assert_eq!(value, expected);
        }
    }
}

#[test]
fn fixed_view_retains_fine_angles_while_warning_heading_uses_only_the_yaw_high_byte() {
    let mut view = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Unassigned);
    for yaw in 0..=u16::MAX {
        let pitch = yaw.wrapping_mul(47);
        let angles = FixedViewAngles {
            pitch,
            yaw,
            roll: yaw.wrapping_mul(91),
        };
        let position = Vector3 {
            x: -301,
            y: 429,
            z: -789,
        };
        assert_eq!(angles.heading().units(), (yaw >> 8) as u8);
        view.base.position = position;
        angles.write_to(&mut view);
        assert_eq!(FixedViewAngles::capture(&view), angles);
        let anchor = TargetAnchor::from_view(&view);
        assert_eq!(anchor.position, position);
        assert_eq!(anchor.pitch, pitch);
        assert_eq!(anchor.yaw, yaw);
    }
}

#[test]
fn scene_target_reads_live_fixed_view_and_mutates_primary_not_path_selected_player() {
    let mut objects = ObjectStore::new();
    let one = actor(&mut objects);
    let two = actor(&mut objects);
    let view = actor(&mut objects);
    let owner = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::new([1, 7, 17, 33]));
    let mut paths = PathInvocation::default();
    storage(&mut objects, &mut world, &mut paths, one);
    storage(&mut objects, &mut world, &mut paths, two);
    world.fixed_players[0] = Some(view);
    let catalog = PathCatalog::new(vec![vec![
        Statement::ConsiderPrimaryTarget { next: at(1) },
        Statement::Control(ControlCommand::Hold),
    ]])
    .unwrap();
    for primary in [one, two, one] {
        let secondary = if primary == one { two } else { one };
        world.primary_player = Some(primary);
        world.secondary_player = Some(secondary);
        world
            .player_mut(&objects, secondary)
            .unwrap()
            .target_selection = Some(initial(owner, 255));
        for (pitch, yaw, position) in [
            (
                127,
                0,
                Vector3 {
                    x: 13,
                    y: 29,
                    z: -47,
                },
            ),
            (
                128,
                128,
                Vector3 {
                    x: -1301,
                    y: -93,
                    z: 59,
                },
            ),
            (
                3583,
                5375,
                Vector3 {
                    x: 2901,
                    y: 99,
                    z: -1705,
                },
            ),
        ] {
            objects.get_mut(view).unwrap().base.position = position;
            FixedViewAngles {
                pitch,
                yaw,
                roll: 0,
            }
            .write_to(objects.get_mut(view).unwrap());
            let candidate = Vector3 {
                x: position.x.wrapping_add(50),
                y: 33,
                z: position.z.wrapping_add(70),
            };
            let source = objects.get_mut(owner).unwrap();
            source.base.position = candidate;
            source.base.path = Some(at(0));
            // Ordinary player angles and position must not replace the view.
            objects.get_mut(primary).unwrap().base.position = Vector3 {
                x: -9001,
                y: 9,
                z: 901,
            };
            let selection = TargetSelection {
                distance: u16::MAX,
                ..Default::default()
            };
            world
                .player_mut(&objects, primary)
                .unwrap()
                .target_selection = Some(selection);
            let mut expected = selection;
            assert!(consider(
                &mut expected,
                owner,
                candidate,
                TargetAnchor {
                    position,
                    pitch,
                    yaw
                }
            ));
            let secondary_before = *world.player(&objects, secondary).unwrap();
            let input = &mut world
                .path_world(&objects, owner, PlayerTarget::Secondary)
                .unwrap();
            assert_eq!(input.selected, Some(secondary));
            assert_eq!(
                paths
                    .runtime
                    .step_program(&catalog, &mut objects, owner, input)
                    .map(|exit| exit.step),
                Ok(ControlStep::Continue)
            );
            assert_eq!(
                world.player(&objects, primary).unwrap().target_selection,
                Some(expected)
            );
            assert_eq!(
                world.player(&objects, secondary).unwrap(),
                &secondary_before
            );
            assert_eq!(objects.get(owner).unwrap().base.path, Some(at(1)));
        }
    }
    assert_eq!(world.random.bytes(), [1, 7, 17, 33]);
}

#[test]
fn absent_view_or_player_record_stops_at_target_statement_then_resumes_same_invocation() {
    for missing in 0..3 {
        let mut objects = ObjectStore::new();
        let player = actor(&mut objects);
        let view = actor(&mut objects);
        let owner = actor(&mut objects);
        objects.get_mut(owner).unwrap().base.path = Some(at(0));
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut paths = PathInvocation::default();
        storage(&mut objects, &mut world, &mut paths, player);
        let selection = TargetSelection {
            distance: u16::MAX,
            display_status: 255,
            ..Default::default()
        };
        world.player_mut(&objects, player).unwrap().target_selection =
            (missing != 2).then_some(selection);
        world.fixed_players[0] = (missing != 0).then_some(view);
        if missing == 1 {
            objects.remove(view).unwrap();
        }
        let catalog = PathCatalog::new(vec![vec![
            Statement::ConsiderPrimaryTarget { next: at(1) },
            Statement::Control(ControlCommand::Hold),
        ]])
        .unwrap();
        paths.begin(owner, InvocationEntry::Program).unwrap();
        let before = *world.player(&objects, player).unwrap();
        assert_eq!(
            paths.resume(&catalog, &mut objects, &mut world, 10),
            Err(InvocationError::Program(ProgramError::MissingPrimaryTarget))
        );
        assert_eq!(world.player(&objects, player).unwrap(), &before);
        assert_eq!(objects.get(owner).unwrap().base.path, Some(at(0)));
        assert!(paths.is_active());
        world.fixed_players[0] = Some(view);
        if missing == 1 {
            assert_eq!(actor(&mut objects), view);
        }
        world.player_mut(&objects, player).unwrap().target_selection = Some(selection);
        assert_eq!(
            paths.resume(&catalog, &mut objects, &mut world, 10),
            Ok(owner)
        );
        assert_eq!(
            world
                .player(&objects, player)
                .unwrap()
                .target_selection
                .unwrap()
                .candidate,
            Some(owner)
        );
        assert!(!paths.is_active());
    }
}

#[test]
fn camera_motion_and_restoration_are_seen_by_the_next_target_statement_without_republication() {
    for snap in [false, true] {
        let mut objects = ObjectStore::new();
        let player = actor(&mut objects);
        let view = actor(&mut objects);
        let owner = actor(&mut objects);
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut paths = PathInvocation::default();
        storage(&mut objects, &mut world, &mut paths, player);
        world.fixed_players[0] = Some(view);
        let original_angles = FixedViewAngles {
            pitch: 0xAF31,
            yaw: 0xCA97,
            roll: 0x7359,
        };
        original_angles.write_to(objects.get_mut(view).unwrap());
        objects.get_mut(view).unwrap().base.position = Vector3 {
            x: -701,
            y: 901,
            z: -319,
        };
        let saved = crate::view_transition::ViewBaseSnapshot::capture(objects.get(view).unwrap());
        let candidate = objects.get_mut(owner).unwrap();
        candidate.base.position = Vector3 {
            x: 1501,
            y: -351,
            z: 997,
        };
        candidate.base.pitch = crate::Angle::from_units(83);
        candidate.base.yaw = crate::Angle::from_units(141);
        candidate.base.roll = crate::Angle::from_units(213);
        candidate.base.path = Some(at(0));
        let cleared = TargetSelection {
            distance: u16::MAX,
            ..Default::default()
        };
        world.player_mut(&objects, player).unwrap().target_selection = Some(cleared);
        let catalog = PathCatalog::new(vec![vec![
            Statement::MoveFixedView { snap, next: at(1) },
            Statement::ConsiderPrimaryTarget { next: at(2) },
            Statement::Control(ControlCommand::Hold),
        ]])
        .unwrap();
        paths.begin(owner, InvocationEntry::Program).unwrap();
        paths
            .resume(&catalog, &mut objects, &mut world, 20)
            .unwrap();
        assert_ne!(
            FixedViewAngles::capture(objects.get(view).unwrap()),
            original_angles
        );
        let mut expected = cleared;
        consider(
            &mut expected,
            owner,
            objects.get(owner).unwrap().base.position,
            TargetAnchor::from_view(objects.get(view).unwrap()),
        );
        assert_eq!(
            world.player(&objects, player).unwrap().target_selection,
            Some(expected)
        );

        saved.restore(objects.get_mut(view).unwrap());
        assert_eq!(
            FixedViewAngles::capture(objects.get(view).unwrap()),
            original_angles
        );
        world.player_mut(&objects, player).unwrap().target_selection = Some(cleared);
        objects.get_mut(owner).unwrap().base.path = Some(at(1));
        paths.begin(owner, InvocationEntry::Program).unwrap();
        paths
            .resume(&catalog, &mut objects, &mut world, 20)
            .unwrap();
        let mut after_restore = cleared;
        consider(
            &mut after_restore,
            owner,
            objects.get(owner).unwrap().base.position,
            TargetAnchor::from_view(objects.get(view).unwrap()),
        );
        assert_ne!(expected, after_restore);
        assert_eq!(
            world.player(&objects, player).unwrap().target_selection,
            Some(after_restore)
        );
    }
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("target reset must not run movement")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("target reset must not run death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("target reset must not resume map")
    }
}

#[test]
fn scene_reset_uses_caller_and_missing_shared_mode_latches_after_earlier_candidate_writes() {
    for reflection in [None, Some(false), Some(true)] {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let other = actor(&mut objects);
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut execution = SceneExecution::default();
        let catalog = PathCatalog::new(vec![]).unwrap();
        let mut callbacks = Callbacks;
        world.primary_player = Some(other);
        world.reflect_all_contacts = reflection;
        let records = PlayerPathRecords {
            target_selection: Some(initial(other, 27)),
            ..Default::default()
        };
        world.bind_player(&objects, owner, records).unwrap();
        world.bind_player(&objects, other, records).unwrap();
        let mut scene = SceneActors {
            objects: &mut objects,
            world: &mut world,
            execution: &mut execution,
            catalog: &catalog,
            callbacks: &mut callbacks,
            statement_budget: 10,
        };
        let result = scene.initialize_player_target(owner);
        let mut expected = records;
        let target = expected.target_selection.as_mut().unwrap();
        target.candidate = None;
        target.position = Vector3::default();
        target.auxiliary_distance = u16::MAX;
        if let Some(value) = reflection {
            target.control_flags |= if value { 0x40 } else { 0xC0 };
            assert_eq!(result, Ok(()));
            assert!(!scene.execution.is_faulted());
        } else {
            assert_eq!(
                result,
                Err(SceneError::World(WorldInputError::MissingReflectionMode))
            );
            assert!(scene.execution.is_faulted());
            scene.world.reflect_all_contacts = Some(false);
            assert_eq!(
                scene.initialize_player_target(owner),
                Err(SceneError::Faulted)
            );
        }
        assert_eq!(*scene.world.player(scene.objects, owner).unwrap(), expected);
        assert_eq!(*scene.world.player(scene.objects, other).unwrap(), records);
        assert_eq!(scene.world.primary_player, Some(other));
    }
}
