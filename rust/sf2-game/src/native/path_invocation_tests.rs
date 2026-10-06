use super::super::path_fields::{Axis, ByteField, ByteOperation, WordField, WordOperation};
use super::super::path_invocation::{
    InvocationAlreadyActive, InvocationEntry, InvocationError, InvocationWorld, PathInvocation,
};
use super::super::path_motion::PlayerDisplacement;
use super::super::path_runtime::TriggerWorldInputs;
use super::super::path_scene_state::{SceneEventCommand, SceneEventFlags};
use super::super::path_triggers::{Trigger, TriggerKind};
use super::super::platform_carry::CarriedPlayer;
use super::super::{
    authored_paths, Behavior, ObjectKind, ObjectSpawnDefaults, PathId, Rotation, ShapeId, Vector3,
};
use super::tests::world;
use super::*;

fn cursor(path: u16, command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(path),
        command_index,
    }
}

fn slot(selected: super::super::path_control::PlayerTarget) -> usize {
    match selected {
        super::super::path_control::PlayerTarget::Primary => 0,
        super::super::path_control::PlayerTarget::Secondary => 1,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Services {
    random: RandomState,
    auxiliary: [SelectedAuxiliaryState; 2],
    carried: [Option<CarriedPlayer>; 2],
    events: Option<SceneEventFlags>,
    statement_selections: Vec<usize>,
    statement_actors: Vec<ObjectId>,
    attached_shots: Vec<(ObjectId, super::super::path_shots::ActiveShots)>,
    displacement_selections: Vec<usize>,
    callback_poses: Vec<Vector3>,
    carry_visits: usize,
    fail_secondary: bool,
    fail_displacement: bool,
    fail_carry: bool,
}

impl Default for Services {
    fn default() -> Self {
        Self {
            random: RandomState::default(),
            auxiliary: [
                SelectedAuxiliaryState {
                    mode: 0xA0,
                    action_flags: 0,
                    stored_world_position: Vector3::default(),
                    stored_rotation: Rotation::default(),
                },
                SelectedAuxiliaryState {
                    mode: 0xB0,
                    action_flags: 0,
                    stored_world_position: Vector3::default(),
                    stored_rotation: Rotation::default(),
                },
            ],
            carried: [None; 2],
            events: None,
            statement_selections: Vec::new(),
            statement_actors: Vec::new(),
            attached_shots: Vec::new(),
            displacement_selections: Vec::new(),
            callback_poses: Vec::new(),
            carry_visits: 0,
            fail_secondary: false,
            fail_displacement: false,
            fail_carry: false,
        }
    }
}

impl InvocationWorld for Services {
    type Error = &'static str;

    fn path_world(
        &mut self,
        objects: &ObjectStore,
        actor: ObjectId,
        selected: super::super::path_control::PlayerTarget,
    ) -> Result<PathWorld<'_>, Self::Error> {
        let slot = slot(selected);
        if self.fail_secondary && slot == 1 {
            return Err("secondary not supplied");
        }
        self.statement_selections.push(slot);
        self.statement_actors.push(actor);
        let mut inputs = world(&mut self.random);
        inputs.selected_auxiliary = Some(&mut self.auxiliary[slot]);
        inputs.scene_events = self.events.as_mut();
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        if let Some(linked) = objects.get(actor).and_then(|actor| actor.base.attachment) {
            inputs.linked_shot_count = self.attached_shots.iter_mut()
                .find(|(owner, _)| *owner == linked)
                .map(|(owner, state)| super::super::path_shots::LinkedShotCount { owner: *owner, state });
        }
        Ok(inputs)
    }

    fn displacement(
        &mut self,
        selected: super::super::path_control::PlayerTarget,
    ) -> Result<PlayerDisplacement, Self::Error> {
        if self.fail_displacement {
            return Err("motion not supplied");
        }
        let selected = slot(selected);
        self.displacement_selections.push(selected);
        Ok(PlayerDisplacement {
            world_delta: Vector3 {
                x: if selected == 0 { 10 } else { 20 },
                y: 0,
                z: 0,
            },
            suppress_horizontal: false,
        })
    }

    fn trigger_inputs(
        &mut self,
        objects: &ObjectStore,
        owner: ObjectId,
        _: super::super::path_control::PlayerTarget,
    ) -> Result<TriggerWorldInputs, Self::Error> {
        self.callback_poses
            .push(objects.get(owner).unwrap().base.position);
        Ok(TriggerWorldInputs::default())
    }

    fn carried_players(&mut self) -> Result<&mut [Option<CarriedPlayer>; 2], Self::Error> {
        if self.fail_carry {
            return Err("carry not supplied");
        }
        self.carry_visits += 1;
        Ok(&mut self.carried)
    }
}

fn actor(path: PathCursor) -> Object {
    let mut actor = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
    actor.base.path = Some(path);
    actor
}

fn callback_fixture() -> (PathInvocation, ObjectStore, ObjectId, Services, PathCatalog) {
    let mut objects = ObjectStore::new();
    let mut original = actor(cursor(0, 0));
    original.base.position.x = 1;
    original.base.velocity.x = 5;
    original.base.contacts.hit_by_secondary = true;
    original
        .extension
        .path_state
        .motion
        .follow_player_displacement = true;
    original.extension.path_state.motion.carry_selected_player = true;
    original.extension.path_state.motion_delta.x = 1;
    let owner = objects.allocate(original).unwrap();
    let mut invocation = PathInvocation::default();
    invocation
        .runtime
        .add_trigger(
            &mut objects,
            owner,
            Trigger {
                path: cursor(1, 0),
                kind: TriggerKind::PlayerContact,
                timer: 0,
            },
        )
        .unwrap();
    // The first callback's health write must be visible to this predicate.
    invocation
        .runtime
        .add_trigger(
            &mut objects,
            owner,
            Trigger {
                path: cursor(2, 0),
                kind: TriggerKind::ZeroHealth,
                timer: 0,
            },
        )
        .unwrap();
    let mut services = Services::default();
    services.carried = [
        Some(CarriedPlayer {
            enabled: true,
            carrier: Some(owner),
            origin: Vector3 {
                x: 1000,
                y: 0,
                z: 0,
            },
            fine_yaw: 0,
        }),
        Some(CarriedPlayer {
            enabled: true,
            carrier: Some(owner),
            origin: Vector3 { x: 100, y: 0, z: 0 },
            fine_yaw: 0,
        }),
    ];
    let catalog = PathCatalog::new(vec![
        vec![
            Statement::Control(ControlCommand::WaitOne { next: cursor(0, 1) }),
            Statement::Control(ControlCommand::End),
        ],
        vec![
            Statement::SelectedAuxiliary {
                command: SelectedAuxiliaryCommand::SetModeLowNibbleOne,
                next: cursor(1, 1),
            },
            Statement::Mutate {
                mutation: Mutation::Byte {
                    field: ByteField::Health,
                    operation: ByteOperation::Assign(ByteOperand::Literal(0)),
                },
                next: cursor(1, 2),
            },
            Statement::Mutate {
                mutation: Mutation::Word {
                    field: WordField::Velocity(Axis::X),
                    operation: WordOperation::Assign(WordOperand::Literal(7)),
                },
                next: cursor(1, 3),
            },
            Statement::Control(ControlCommand::Return),
        ],
        vec![
            Statement::Mutate {
                mutation: Mutation::Byte {
                    field: ByteField::AttackPower,
                    operation: ByteOperation::Assign(ByteOperand::Literal(73)),
                },
                next: cursor(2, 1),
            },
            Statement::Control(ControlCommand::Return),
        ],
    ])
    .unwrap();
    (invocation, objects, owner, services, catalog)
}

#[test]
fn complete_invocation_uses_entry_motion_then_live_callbacks_then_selected_carry() {
    let (mut invocation, mut objects, owner, mut services, catalog) = callback_fixture();
    invocation.begin(owner, InvocationEntry::Program).unwrap();
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut services, 100),
        Ok(owner)
    );
    assert!(!invocation.is_active());
    let actor = objects.get(owner).unwrap();
    assert_eq!(actor.base.position.x, 16); // Initial 1 + primary displacement 10 + velocity 5.
    assert_eq!(actor.base.velocity.x, 7);
    assert_eq!(actor.base.attack_power, 73);
    assert_eq!(actor.base.path, Some(cursor(0, 1)));
    assert!(!actor.base.contacts.hit_by_secondary);
    assert_eq!(services.displacement_selections, [0]);
    assert_eq!(services.auxiliary[0].mode, 0xA0);
    assert_eq!(services.auxiliary[1].mode, 0xB1);
    assert_eq!(services.carried[0].unwrap().origin.x, 1000);
    assert_eq!(services.carried[1].unwrap().origin.x, 116);
    assert_eq!(services.carry_visits, 1);
    assert!(services.callback_poses.iter().all(|pose| pose.x == 16));
    assert_eq!(services.statement_selections, [0, 1, 1, 1, 1, 1, 1]);
    assert_eq!(services.statement_actors, [owner; 7]);
}

#[test]
fn every_budget_split_preserves_full_state_and_never_reintegrates_movement() {
    let (mut expected, mut expected_objects, owner, mut expected_services, catalog) =
        callback_fixture();
    expected.begin(owner, InvocationEntry::Program).unwrap();
    expected
        .resume(&catalog, &mut expected_objects, &mut expected_services, 100)
        .unwrap();
    for chunk in 1..=25 {
        let (mut invocation, mut objects, owner, mut services, _) = callback_fixture();
        invocation.begin(owner, InvocationEntry::Program).unwrap();
        let before = (invocation.clone(), objects.clone(), services.clone());
        assert_eq!(
            invocation.resume(&catalog, &mut objects, &mut services, 0),
            Err(InvocationError::BudgetExceeded { completed_steps: 0 })
        );
        assert_eq!(
            (invocation.clone(), objects.clone(), services.clone()),
            before
        );
        assert_eq!(
            invocation.begin(owner, InvocationEntry::Movement),
            Err(InvocationAlreadyActive)
        );
        for _ in 0..100 {
            match invocation.resume(&catalog, &mut objects, &mut services, chunk) {
                Ok(actor) => {
                    assert_eq!(actor, owner);
                    break;
                }
                Err(InvocationError::BudgetExceeded { completed_steps }) => {
                    assert_eq!(completed_steps, chunk)
                }
                error => panic!("unexpected invocation result {error:?}"),
            }
        }
        assert_eq!(invocation, expected);
        assert_eq!(objects, expected_objects);
        assert_eq!(services, expected_services);
        assert_eq!(
            invocation.resume(&catalog, &mut objects, &mut services, 100),
            Err(InvocationError::NotActive)
        );
    }
}

#[test]
fn missing_world_inputs_resume_each_exact_service_boundary_without_replaying_effects() {
    let (mut invocation, mut objects, owner, mut services, catalog) = callback_fixture();
    invocation.begin(owner, InvocationEntry::Program).unwrap();
    services.fail_displacement = true;
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut services, 100),
        Err(InvocationError::World("motion not supplied"))
    );
    assert_eq!(objects.get(owner).unwrap().base.position.x, 1);
    assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 1)));
    services.fail_displacement = false;
    services.fail_secondary = true;
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut services, 100),
        Err(InvocationError::World("secondary not supplied"))
    );
    let moved = objects.get(owner).unwrap().base.position;
    assert_eq!(services.displacement_selections, [0]);
    services.fail_secondary = false;
    services.fail_carry = true;
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut services, 100),
        Err(InvocationError::World("carry not supplied"))
    );
    assert_eq!(objects.get(owner).unwrap().base.position, moved);
    assert_eq!(objects.get(owner).unwrap().base.attack_power, 73);
    let statements = services.statement_selections.clone();
    services.fail_carry = false;
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut services, 100),
        Ok(owner)
    );
    assert_eq!(objects.get(owner).unwrap().base.position, moved);
    assert_eq!(services.statement_selections, statements);
    assert_eq!(services.displacement_selections, [0]);
}

#[test]
fn borrowed_actor_immediate_next_refreshes_world_selection_before_the_next_statement() {
    let mut objects = ObjectStore::new();
    let owner = objects.allocate(actor(cursor(0, 0))).unwrap();
    let mut borrowed_actor = actor(cursor(1, 0));
    borrowed_actor
        .extension
        .path_state
        .conditions
        .selected_player = super::super::path_control::PlayerTarget::Secondary;
    borrowed_actor.base.velocity.x = 7;
    let borrowed = objects.allocate(borrowed_actor).unwrap();
    objects.get_mut(owner).unwrap().base.attachment = Some(borrowed);
    objects.get_mut(owner).unwrap().base.velocity.x = 999;
    let catalog = PathCatalog::new(vec![vec![
        Statement::SelectActor {
            selection: ActorSelection::Linked,
            next: cursor(0, 1),
        },
        Statement::Control(ControlCommand::BeginLoop {
            iterations: 2,
            next: cursor(0, 2),
        }),
        Statement::SelectedAuxiliary {
            command: SelectedAuxiliaryCommand::SetModeLowNibbleOne,
            next: cursor(0, 3),
        },
        Statement::Control(ControlCommand::Next {
            immediate: true,
            next: cursor(0, 4),
        }),
        Statement::Control(ControlCommand::WaitOne { next: cursor(0, 5) }),
        Statement::Control(ControlCommand::End),
    ]])
    .unwrap();
    let mut invocation = PathInvocation::default();
    let mut services = Services::default();
    invocation.begin(owner, InvocationEntry::Program).unwrap();
    for _ in 0..30 {
        match invocation.resume(&catalog, &mut objects, &mut services, 1) {
            Ok(result) => {
                assert_eq!(result, borrowed);
                break;
            }
            Err(InvocationError::BudgetExceeded { .. }) => {}
            error => panic!("borrowed invocation failed {error:?}"),
        }
    }
    assert!(!invocation.is_active());
    assert_eq!(
        (services.auxiliary[0].mode, services.auxiliary[1].mode),
        (0xA1, 0xB1)
    );
    assert_eq!(services.statement_selections, [0, 0, 0, 0, 1, 1, 1]);
    assert_eq!(services.statement_actors, [owner, borrowed, borrowed, borrowed, borrowed, borrowed, borrowed]);
    assert_eq!(objects.get(owner).unwrap().base.position.x, 0);
    assert_eq!(objects.get(borrowed).unwrap().base.position.x, 7);
    assert_eq!(objects.get(borrowed).unwrap().base.path, Some(cursor(0, 5)));
}

#[test]
fn world_resolves_borrowed_actors_attachment_on_each_statement_and_missing_input_resume() {
    use super::super::path_shots::{ActiveShots, ShotCountCommand};
    let mut objects = ObjectStore::new();
    let owner = objects.allocate(actor(cursor(0, 0))).unwrap();
    let borrowed = objects.allocate(actor(cursor(1, 0))).unwrap();
    let attached_player = objects.allocate(actor(cursor(2, 0))).unwrap();
    objects.get_mut(owner).unwrap().base.attachment = Some(borrowed);
    objects.get_mut(borrowed).unwrap().base.attachment = Some(attached_player);
    let catalog = PathCatalog::new(vec![vec![
        Statement::SelectActor { selection: ActorSelection::Linked, next: cursor(0, 1) },
        Statement::LinkedShotCount { command: ShotCountCommand::Increment, next: cursor(0, 2) },
        Statement::Control(ControlCommand::End),
    ]]).unwrap();
    let mut invocation = PathInvocation::default();
    let mut services = Services::default();
    services.attached_shots.push((borrowed, ActiveShots::from_count(41)));
    invocation.begin(owner, InvocationEntry::Program).unwrap();
    assert_eq!(invocation.resume(&catalog, &mut objects, &mut services, 100),
        Err(InvocationError::Program(ProgramError::MissingLinkedShotCount)));
    assert_eq!(services.statement_actors, [owner, borrowed]);
    assert_eq!(services.attached_shots[0].1, ActiveShots::from_count(41));
    services.attached_shots.push((attached_player, ActiveShots::from_count(7)));
    assert_eq!(invocation.resume(&catalog, &mut objects, &mut services, 100), Ok(borrowed));
    assert_eq!(services.statement_actors, [owner, borrowed, borrowed, borrowed]);
    assert_eq!(services.statement_selections, [0; 4]);
    assert_eq!(services.attached_shots,
        [(borrowed, ActiveShots::from_count(41)), (attached_player, ActiveShots::from_count(8))]);
    assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
    assert!(objects.get(borrowed).unwrap().base.flags.remove_after_tick);
}

#[test]
fn program_fault_resumes_after_prior_publication_without_a_second_entry_or_movement() {
    let catalog = PathCatalog::new(vec![vec![
        Statement::Mutate {
            mutation: Mutation::Word {
                field: WordField::ScriptValue,
                operation: WordOperation::Increment,
            },
            next: cursor(0, 1),
        },
        Statement::SceneEvent {
            command: SceneEventCommand::Assign(WordOperand::Actor(WordField::ScriptValue)),
            next: cursor(0, 2),
        },
        Statement::Control(ControlCommand::End),
    ]])
    .unwrap();
    let mut objects = ObjectStore::new();
    let owner = objects.allocate(actor(cursor(0, 0))).unwrap();
    let mut invocation = PathInvocation::default();
    let mut services = Services::default();
    invocation.begin(owner, InvocationEntry::Program).unwrap();
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut services, 100),
        Err(InvocationError::Program(ProgramError::MissingSceneEvents))
    );
    assert_eq!(
        objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .script_value,
        1
    );
    services.events = Some(SceneEventFlags { bits: 900 });
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut services, 100),
        Ok(owner)
    );
    assert_eq!(services.events.unwrap().bits, 1);
    assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
    assert_eq!(objects.len(), 1, "retirement belongs to the scheduler");
    assert!(services.displacement_selections.is_empty());
    assert!(services.callback_poses.is_empty());
    assert_eq!(services.carry_visits, 0);
}

#[test]
fn death_tail_visits_callbacks_without_pre_callback_motion_and_hold_does_not_enter_path() {
    let catalog = PathCatalog::new(vec![
        vec![Statement::MarkForDeath],
        vec![
            Statement::Mutate {
                mutation: Mutation::Byte {
                    field: ByteField::AttackPower,
                    operation: ByteOperation::Assign(ByteOperand::Literal(73)),
                },
                next: cursor(1, 1),
            },
            Statement::Control(ControlCommand::Return),
        ],
    ])
    .unwrap();
    let mut objects = ObjectStore::new();
    let mut original = actor(cursor(0, 0));
    original.base.position.x = 11;
    original.base.velocity.x = 7;
    original
        .extension
        .path_state
        .motion
        .follow_player_displacement = true;
    let owner = objects.allocate(original).unwrap();
    let mut invocation = PathInvocation::default();
    invocation
        .runtime
        .add_trigger(
            &mut objects,
            owner,
            Trigger {
                path: cursor(1, 0),
                kind: TriggerKind::ZeroHealth,
                timer: 0,
            },
        )
        .unwrap();
    let mut services = Services::default();
    invocation.begin(owner, InvocationEntry::Program).unwrap();
    invocation
        .resume(&catalog, &mut objects, &mut services, 100)
        .unwrap();
    assert_eq!(objects.get(owner).unwrap().base.position.x, 11);
    assert_eq!(objects.get(owner).unwrap().base.attack_power, 73);
    assert!(services.displacement_selections.is_empty());
    invocation
        .runtime
        .clear_triggers(&mut objects, owner)
        .unwrap();
    let value = objects.get_mut(owner).unwrap();
    value.base.path = None;
    value.base.behavior = Behavior::PathMovement;
    value.extension.path_state.conditions.selected_player =
        super::super::path_control::PlayerTarget::Secondary;
    let statements = services.statement_selections.clone();
    invocation.begin(owner, InvocationEntry::Movement).unwrap();
    invocation
        .resume(&catalog, &mut objects, &mut services, 100)
        .unwrap();
    assert_eq!(services.statement_selections, statements);
    assert_eq!(
        services.displacement_selections,
        [0],
        "held motion preserves shared selection"
    );
}

#[test]
fn authored_exhaust_completes_two_moves_then_end_without_recycling_the_actor() {
    let catalog = authored_paths::catalog();
    let mut objects = ObjectStore::new();
    let mut original = actor(authored_paths::ALTERNATE_EXHAUST);
    original.base.velocity.x = 7;
    let owner = objects.allocate(original).unwrap();
    let mut invocation = PathInvocation::default();
    let mut services = Services::default();
    for (visit, expected_x) in [7, 14, 14].into_iter().enumerate() {
        invocation.begin(owner, InvocationEntry::Program).unwrap();
        assert_eq!(
            invocation.resume(&catalog, &mut objects, &mut services, 100),
            Ok(owner)
        );
        assert_eq!(objects.get(owner).unwrap().base.position.x, expected_x);
        assert_eq!(
            objects.get(owner).unwrap().base.flags.remove_after_tick,
            visit == 2
        );
        assert_eq!(objects.len(), 1);
    }
    assert_eq!(services.random, RandomState::default());
}

#[test]
fn invalid_callback_yield_latches_without_starting_nested_movement() {
    let (mut invocation, mut objects, owner, mut services, _) = callback_fixture();
    let catalog = PathCatalog::new(vec![
        vec![Statement::Control(ControlCommand::WaitOne {
            next: cursor(0, 1),
        })],
        vec![Statement::Control(ControlCommand::WaitOne {
            next: cursor(1, 1),
        })],
    ])
    .unwrap();
    invocation.begin(owner, InvocationEntry::Program).unwrap();
    let expected = Err(InvocationError::UnexpectedExit {
        actor: owner,
        step: ControlStep::Movement,
    });
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut services, 100),
        expected
    );
    assert_eq!(objects.get(owner).unwrap().base.position.x, 16);
    let before = (invocation.clone(), objects.clone(), services.clone());
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut services, 100),
        expected
    );
    assert_eq!((invocation, objects, services), before);
}

#[test]
fn invalid_runtime_storage_after_movement_latches_instead_of_moving_again_on_retry() {
    let (mut invocation, mut objects, owner, mut services, catalog) = callback_fixture();
    invocation.runtime.resources.release_owner(owner);
    invocation.begin(owner, InvocationEntry::Program).unwrap();
    let expected = Err(InvocationError::Runtime(PathRuntimeError::Triggers(
        super::super::path_triggers::TriggerError::MissingStorage,
    )));
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut services, 100),
        expected
    );
    assert_eq!(objects.get(owner).unwrap().base.position.x, 16);
    let before = (invocation.clone(), objects.clone(), services.clone());
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut services, 100),
        expected
    );
    assert_eq!((invocation, objects, services), before);
}
