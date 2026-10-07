use super::*;
use crate::path_commands::ControlCommand;
use crate::path_control::PlayerTarget;
use crate::path_invocation::InvocationWorld;
use crate::path_program::{PathCatalog, ProgramError, Statement};
use crate::path_runtime::PathRuntime;
use crate::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use crate::{Angle, PathCursor, PathId, RandomState, Vector3};

fn fixture() -> (
    ObjectStore,
    ObjectId,
    ObjectSpawnDefaults,
    LinkedEffectActivity,
) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::from_catalog_index(2),
            Behavior::Unassigned,
        ))
        .unwrap();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.position = Vector3 {
        x: -32768,
        y: 791,
        z: 32767,
    };
    actor.base.pitch = Angle::from_units(241);
    actor.base.yaw = Angle::from_units(198);
    actor.base.roll = Angle::from_units(117);
    (
        objects,
        owner,
        ObjectSpawnDefaults {
            group: 41,
            run_when_paused: false,
        },
        LinkedEffectActivity { recent_spawn: 232 },
    )
}

fn protected() -> Option<DeflectionProtection> {
    Some(DeflectionProtection::from_control(0xE1))
}

#[test]
fn zero_primary_count_skips_defaults_activity_pool_and_owner_observations() {
    let (mut objects, owner, _, _) = fixture();
    while objects.len() < OBJECT_CAPACITY {
        objects
            .allocate(objects.get(owner).unwrap().clone())
            .unwrap();
    }
    let before = objects.clone();
    for bits in (0..=255u8).filter(|bits| bits & 31 == 0) {
        assert_eq!(
            install(
                &mut objects,
                owner,
                Some(DeflectionProtection::from_control(bits)),
                None,
                None
            ),
            Ok(None)
        );
        assert_eq!(objects, before);
    }
    assert_eq!(
        install(&mut objects, owner, protected(), None, None),
        Err(ExitShieldError::ObjectPoolExhausted)
    );
    assert_eq!(objects, before);
}

#[test]
fn birth_allocates_fresh_siblings_and_uses_current_pose_without_extension_parent() {
    let (mut objects, owner, defaults, mut activity) = fixture();
    let one = install(
        &mut objects,
        owner,
        protected(),
        Some(defaults),
        Some(&mut activity),
    )
    .unwrap()
    .unwrap();
    objects.get_mut(one).unwrap().base.flags.remove_after_tick = true;
    let two = install(
        &mut objects,
        owner,
        protected(),
        Some(defaults),
        Some(&mut activity),
    )
    .unwrap()
    .unwrap();
    assert_ne!(one, two);
    assert_eq!(objects.get(owner).unwrap().base.attachment_next, Some(one));
    assert_eq!(objects.get(one).unwrap().base.attachment_next, Some(two));
    let actor = objects.get(two).unwrap();
    assert_eq!(actor.base.behavior, Behavior::ExitShield);
    assert_eq!(actor.base.shape, SHIELD_SHAPE);
    assert_eq!(actor.base.child_number, CHILD_NUMBER);
    assert_eq!(actor.base.attachment, Some(owner));
    assert_eq!(actor.extension.parent, None);
    assert_eq!(
        actor.base.position,
        objects.get(owner).unwrap().base.position
    );
    assert_eq!(actor.base.pitch, objects.get(owner).unwrap().base.pitch);
    assert_eq!(actor.base.yaw, objects.get(owner).unwrap().base.yaw);
    assert_eq!(actor.base.roll, objects.get(owner).unwrap().base.roll);
    assert_eq!(
        (
            actor.base.hit_points,
            actor.base.attack_power,
            actor.extension.spawn_group
        ),
        (1, 1, 255)
    );
    assert!(actor.base.contacts.run_when_paused);
    assert!(actor.base.flags.collision_disabled);
    assert!(actor.base.flags.general_search_eligible);
    assert!(!actor.extension.path_state.needs_path_initialization);
    assert_eq!(activity.recent_spawn, 1);
}

#[test]
fn missing_activity_preserves_allocated_formatted_child_before_fault() {
    let (mut objects, owner, defaults, _) = fixture();
    assert_eq!(
        install(&mut objects, owner, protected(), Some(defaults), None),
        Err(ExitShieldError::MissingSpawnActivity)
    );
    assert_eq!(objects.len(), 2);
    let child = objects.get(owner).unwrap().base.attachment_next.unwrap();
    let child = objects.get(child).unwrap();
    assert_eq!(child.base.behavior, Behavior::ExitShield);
    assert_eq!(
        child.base.position,
        objects.get(owner).unwrap().base.position
    );
    assert!(child.base.flags.collision_disabled);
    assert!(!child.extension.path_state.needs_path_initialization);
}

#[test]
fn strategy_preserves_pose_spins_then_lazily_reads_parent_and_action_gate() {
    for (visible, empty, gate) in [
        (false, false, None),
        (true, true, None),
        (true, false, Some(0)),
        (true, false, Some(255)),
        (true, false, None),
    ] {
        let (mut objects, owner, defaults, mut activity) = fixture();
        let child = install(
            &mut objects,
            owner,
            protected(),
            Some(defaults),
            Some(&mut activity),
        )
        .unwrap()
        .unwrap();
        objects.get_mut(owner).unwrap().base.flags.visible = visible;
        if empty {
            objects.get_mut(owner).unwrap().base.shape = ShapeId::EMPTY;
        }
        let actor = objects.get_mut(child).unwrap();
        actor.extension.relative_rotation.pitch = Angle::from_units(251);
        actor.extension.relative_rotation.yaw = Angle::from_units(249);
        actor.extension.relative_rotation.roll = Angle::from_units(255);
        actor.extension.path_state.animation.shape.initialize(217);
        let position = actor.base.position;
        let result = step(&mut objects, child, gate);
        let missing = visible && !empty && gate.is_none();
        assert_eq!(
            result,
            if missing {
                Err(ExitShieldError::MissingActionGate)
            } else {
                Ok(())
            }
        );
        let actor = objects.get(child).unwrap();
        assert_eq!(actor.base.position, position);
        assert_eq!(actor.extension.relative_rotation.pitch.units(), 3);
        assert_eq!(actor.extension.relative_rotation.yaw.units(), 249);
        assert_eq!(actor.extension.relative_rotation.roll.units(), 5);
        assert!(actor.base.flags.maximum_draw_distance);
        assert!(actor.base.flags.collision_disabled);
        assert_eq!(
            actor.base.flags.remove_after_tick,
            !visible || empty || gate == Some(0)
        );
        assert_eq!(
            actor.extension.path_state.animation.shape.packed(),
            if visible && !empty { SHIELD_FRAME } else { 217 }
        );
    }
}

#[test]
fn missing_parent_preserves_spin_but_not_frame_or_retirement() {
    let (mut objects, owner, defaults, mut activity) = fixture();
    let child = install(
        &mut objects,
        owner,
        protected(),
        Some(defaults),
        Some(&mut activity),
    )
    .unwrap()
    .unwrap();
    objects.get_mut(child).unwrap().base.attachment = None;
    assert_eq!(
        step(&mut objects, child, None),
        Err(ExitShieldError::MissingAttachment)
    );
    let actor = objects.get(child).unwrap();
    assert_eq!(actor.extension.relative_rotation.pitch.units(), 8);
    assert_eq!(actor.extension.relative_rotation.roll.units(), 6);
    assert_eq!(actor.extension.path_state.animation.shape.packed(), 0);
    assert!(!actor.base.flags.remove_after_tick);
}

#[test]
fn optional_child_retirement_preserves_absent_chain_without_relaxing_other_callers() {
    use crate::path_relationships::{apply, RelationshipCommand};
    let (mut objects, owner, defaults, mut activity) = fixture();
    let child = install(
        &mut objects,
        owner,
        protected(),
        Some(defaults),
        Some(&mut activity),
    )
    .unwrap()
    .unwrap();
    let original = objects.clone();
    apply(
        &mut objects,
        owner,
        RelationshipCommand::RetireOptionalChild { number: 5, allow_absent_parent: false },
    )
    .unwrap();
    assert_eq!(objects, original);
    assert_eq!(
        apply(
            &mut objects,
            owner,
            RelationshipCommand::RetireChild { number: 5 }
        ),
        Err(RelationshipError::MissingChild { owner, number: 5 })
    );
    assert_eq!(objects, original);
    objects.get_mut(child).unwrap().base.child_number = 5;
    let mut expected = objects.clone();
    expected
        .get_mut(child)
        .unwrap()
        .base
        .flags
        .remove_after_tick = true;
    apply(
        &mut objects,
        owner,
        RelationshipCommand::RetireOptionalChild { number: 5, allow_absent_parent: false },
    )
    .unwrap();
    assert_eq!(objects, expected);
}

#[test]
fn scene_path_reads_live_primary_protection_and_publishes_canonical_engine_sound() {
    let (mut objects, owner, defaults, activity) = fixture();
    let primary = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::from_catalog_index(2),
            Behavior::Unassigned,
        ))
        .unwrap();
    let secondary = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::from_catalog_index(2),
            Behavior::Unassigned,
        ))
        .unwrap();
    let mut world = ScenePathWorld::new(RandomState::default());
    world.primary_player = Some(primary);
    world.secondary_player = Some(secondary);
    world.spawn_defaults = Some(defaults);
    world.linked_effect_activity = Some(activity);
    for (player, protection) in [
        (primary, protected()),
        (secondary, Some(DeflectionProtection::default())),
    ] {
        world
            .bind_player(
                &objects,
                player,
                PlayerPathRecords {
                    protection,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    let cursor = |index| PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    };
    let commands = [
        Statement::InstallExitShield { next: cursor(1) },
        Statement::AssignEngineSoundControl {
            value: 8,
            next: cursor(2),
        },
        Statement::Control(ControlCommand::Hold),
    ];
    let catalog = PathCatalog::new(vec![commands.to_vec()]).unwrap();
    let mut runtime = PathRuntime::default();
    objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
    let mut input = world
        .path_world(&objects, owner, PlayerTarget::Secondary)
        .unwrap();
    let _ = runtime
        .step_program(&catalog, &mut objects, owner, &mut input)
        .unwrap();
    assert!(objects.get(owner).unwrap().base.attachment_next.is_some());
    let mut input = world
        .path_world(&objects, owner, PlayerTarget::Secondary)
        .unwrap();
    assert_eq!(
        runtime.step_program(&catalog, &mut objects, owner, &mut input),
        Err(ProgramError::MissingEngineSoundControl)
    );
    assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(1)));
    world.engine_sound_control = Some(crate::player_engine_sound::EngineSoundControl::from_bits(
        255,
    ));
    let mut input = world
        .path_world(&objects, owner, PlayerTarget::Secondary)
        .unwrap();
    let _ = runtime
        .step_program(&catalog, &mut objects, owner, &mut input)
        .unwrap();
    assert_eq!(world.engine_sound_control.unwrap().bits(), 8);
    assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(2)));
}
