//! Execution of decoded native path catalogs. Source addresses and operand
//! encodings belong to offline lowering, never to this dispatcher. A catalog
//! contains only implemented statements; missing entries are errors, not NOPs.
//!
//! Common entry (`$7F:7E53`) selects the actor's player once. Immediate advances
//! (`$7F:7E75`) retain selection. Movement and callback services remain explicit
//! scheduler boundaries; dispatch does not advance time or tick other actors.

use super::path_commands::{BranchCommand, ControlCommand, ControlStep, MotionCommand};
use super::path_conditions::{Predicate, SpatialCondition};
use super::path_fields::{ByteOperand, Mutation, WordOperand};
use super::path_runtime::{PathRuntime, PathRuntimeError};
use super::{Object, ObjectId, ObjectStore, PathCursor, RandomState};

/// Shared world inputs, borrowed rather than duplicated per actor or path.
/// The caller owns clock advancement and random state across every service.
pub struct PathWorld<'a> {
    pub audio: Option<super::path_sound::PathAudio<'a>>,
    /// Primary player identity, independent of the current selected slot.
    pub primary_player: Option<ObjectId>,
    pub selected: Option<ObjectId>,
    /// Fixed player actors, distinct from the live selected/primary pointers.
    pub fixed_players: [Option<ObjectId>; 2],
    /// Fresh primary auxiliary mode and retained displacement, required only
    /// by the one-time primary-motion inheritance action.
    pub primary_motion: Option<PrimaryMotionInput>,
    pub primary_control: Option<super::path_player_control::PrimaryControl<'a>>,
    pub countdown: Option<&'a mut super::path_countdown::PathCountdown>,
    /// Fresh selected auxiliary observations for this invocation; absent
    /// observations are an error only when a statement actually needs them.
    pub selected_auxiliary: Option<AuxiliaryContinuationInput>,
    /// Fresh initializer-mode observations. Missing inputs fault only if
    /// this invocation reaches a spawn; they are not guessed from pause state.
    pub spawn_defaults: Option<super::ObjectSpawnDefaults>,
    pub random: &'a mut RandomState,
    pub animation_clock: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuxiliaryContinuationInput {
    pub mode: u8,
    pub action_flags: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimaryMotionInput {
    pub auxiliary_mode: u8,
    pub displacement: super::Vector3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedAuxiliaryCondition {
    Continuation,
    ActionBit40,
}

impl SelectedAuxiliaryCondition {
    fn sample(self, input: AuxiliaryContinuationInput) -> Predicate {
        match self {
            Self::Continuation => Predicate::SelectedAuxiliaryContinuation {
                mode: input.mode,
                action_flags: input.action_flags,
            },
            Self::ActionBit40 => Predicate::AnyByteBitsSet {
                value: input.action_flags,
                mask: 0x40,
            },
        }
    }
}

/// Retained expressions are sampled from the live owner on every execution.
/// In particular, an immediate loop must not retain the first iteration's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorCondition {
    EqualShape(super::ShapeId),
    EqualByte(ByteOperand, ByteOperand),
    EqualWord(WordOperand, WordOperand),
    BetweenByte {
        value: ByteOperand,
        lower: ByteOperand,
        upper: ByteOperand,
    },
    BetweenWord {
        value: WordOperand,
        lower: WordOperand,
        upper: WordOperand,
    },
    NonzeroByte(ByteOperand),
    NonzeroWord(WordOperand),
    ZeroByte(ByteOperand),
    ZeroWord(WordOperand),
    SecondByteLess(ByteOperand, ByteOperand),
    SecondWordLess(WordOperand, WordOperand),
    AnyByteBitsSet(ByteOperand, ByteOperand),
    AnyWordBitsSet(WordOperand, WordOperand),
}

impl ActorCondition {
    fn sample(self, actor: &Object) -> Predicate {
        match self {
            Self::EqualShape(shape) => Predicate::EqualWord {
                value: actor.base.shape.catalog_index() as u16,
                expected: shape.catalog_index() as u16,
            },
            Self::EqualByte(a, b) => Predicate::EqualByte {
                value: a.read(actor),
                expected: b.read(actor),
            },
            Self::EqualWord(a, b) => Predicate::EqualWord {
                value: a.read(actor),
                expected: b.read(actor),
            },
            Self::BetweenByte {
                value,
                lower,
                upper,
            } => Predicate::BetweenByte {
                value: value.read(actor),
                lower: lower.read(actor),
                upper: upper.read(actor),
            },
            Self::BetweenWord {
                value,
                lower,
                upper,
            } => Predicate::BetweenWord {
                value: value.read(actor),
                lower: lower.read(actor),
                upper: upper.read(actor),
            },
            Self::NonzeroByte(value) => Predicate::NonzeroByte(value.read(actor)),
            Self::NonzeroWord(value) => Predicate::NonzeroWord(value.read(actor)),
            Self::ZeroByte(value) => Predicate::ZeroByte(value.read(actor)),
            Self::ZeroWord(value) => Predicate::ZeroWord(value.read(actor)),
            Self::SecondByteLess(a, b) => Predicate::SecondByteLess {
                first: a.read(actor),
                second: b.read(actor),
            },
            Self::SecondWordLess(a, b) => Predicate::SecondWordLess {
                first: a.read(actor),
                second: b.read(actor),
            },
            Self::AnyByteBitsSet(value, mask) => Predicate::AnyByteBitsSet {
                value: value.read(actor),
                mask: mask.read(actor),
            },
            Self::AnyWordBitsSet(value, mask) => Predicate::AnyWordBitsSet {
                value: value.read(actor),
                mask: mask.read(actor),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Statement {
    Appearance {
        command: super::path_appearance::AppearanceCommand,
        next: PathCursor,
    },
    RunWhenPaused {
        enabled: bool,
        next: PathCursor,
    },
    /// Named source inline action: latch phase low byte to one if the primary
    /// player's view-side filter is enabled; otherwise leave it unchanged.
    LatchPrimaryViewFilter {
        next: PathCursor,
    },
    InheritPrimaryHorizontalMotion {
        next: PathCursor,
    },
    PlayerControl {
        command: super::path_player_control::PlayerControlCommand,
        next: PathCursor,
    },
    Sound {
        cue: super::path_sound::AuthoredCue,
        next: PathCursor,
    },
    SpatialLoop {
        sound: Option<super::SpatialLoop>,
        next: PathCursor,
    },
    Countdown {
        command: super::path_countdown::CountdownCommand,
        next: PathCursor,
    },
    MarkerSound {
        id: u8,
        mode: super::path_sound::MarkerCueMode,
        next: PathCursor,
    },
    SpawnChild {
        kind: super::ObjectKind,
        parameters: super::path_spawn::ChildSpawn,
        next: PathCursor,
    },
    Relationship {
        command: super::path_relationships::RelationshipCommand,
        next: PathCursor,
    },
    CopySelectedTransform {
        command: super::path_relationships::SelectedTransformCommand,
        next: PathCursor,
    },
    SelectedAuxiliaryBranch {
        condition: SelectedAuxiliaryCondition,
        taken: PathCursor,
        next: PathCursor,
    },
    Random {
        mutation: super::path_random::RandomMutation,
        next: PathCursor,
    },
    DisableCollision {
        next: PathCursor,
    },
    Animation {
        command: super::path_appearance::AnimationCommand,
        next: PathCursor,
    },
    Sprite {
        color: u8,
        size: u8,
        next: PathCursor,
    },
    /// Control statements whose operands are literal or semantic cursors.
    Control(ControlCommand),
    Branch(BranchCommand),
    Motion {
        command: MotionCommand,
        next: PathCursor,
    },
    Facing {
        command: super::path_steering::FacingCommand,
        next: PathCursor,
    },
    Contact {
        command: super::path_contact::ContactCommand,
        next: PathCursor,
    },
    Mutate {
        mutation: Mutation,
        next: PathCursor,
    },
    Wait {
        duration: ByteOperand,
        next: PathCursor,
    },
    WaitChase {
        field: super::path_fields::ByteField,
        target: ByteOperand,
        next: PathCursor,
    },
    Repeat {
        count: ByteOperand,
        target: PathCursor,
        next: PathCursor,
    },
    BeginLoop {
        iterations: WordOperand,
        next: PathCursor,
    },
    Compare {
        condition: ActorCondition,
        taken: PathCursor,
        next: PathCursor,
    },
    Spatial {
        condition: SpatialCondition,
        taken: PathCursor,
        next: PathCursor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    MissingPrimaryPlayer,
    MissingPrimaryMotion,
    MissingPrimaryControl,
    MissingAudio,
    MissingSoundMarkers,
    MissingCountdown,
    Spawn(super::path_spawn::SpawnError),
    MissingSpawnDefaults,
    Relationship(super::path_relationships::RelationshipError),
    MissingSelectedAuxiliary,
    Runtime(PathRuntimeError),
    MissingStatement(PathCursor),
    TooManyPaths,
    TooManyStatements {
        path_index: usize,
    },
    /// Diagnostic guard only, not a source WAIT or successful tick completion.
    BudgetExceeded {
        cursor: PathCursor,
        executed: usize,
    },
}

impl From<PathRuntimeError> for ProgramError {
    fn from(error: PathRuntimeError) -> Self {
        Self::Runtime(error)
    }
}

/// Dense, immutable semantic indices. There is no fallback to encoded scripts
/// or an original-program executor when a path has not been lowered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCatalog {
    paths: Vec<Vec<Statement>>,
}

impl PathCatalog {
    pub fn new(paths: Vec<Vec<Statement>>) -> Result<Self, ProgramError> {
        const INDEX_CAPACITY: usize = u16::MAX as usize + 1;
        if paths.len() > INDEX_CAPACITY {
            return Err(ProgramError::TooManyPaths);
        }
        for (path_index, path) in paths.iter().enumerate() {
            if path.len() > INDEX_CAPACITY {
                return Err(ProgramError::TooManyStatements { path_index });
            }
        }
        Ok(Self { paths })
    }

    pub fn statement(&self, cursor: PathCursor) -> Result<Statement, ProgramError> {
        self.paths
            .get(usize::from(cursor.path.catalog_index()))
            .and_then(|path| path.get(usize::from(cursor.command_index)))
            .copied()
            .ok_or(ProgramError::MissingStatement(cursor))
    }
}

impl PathRuntime {
    /// Execute a new common-entry invocation. A callback already entered by
    /// step_callbacks uses resume_program, preserving its selected-player state.
    pub fn enter_program(
        &mut self,
        catalog: &PathCatalog,
        objects: &mut ObjectStore,
        owner: ObjectId,
        world: &mut PathWorld<'_>,
        budget: usize,
    ) -> Result<ControlStep, ProgramError> {
        self.check_execution_owner(owner)?;
        self.initialize_path_strategy(objects, owner)?;
        self.enter(objects, owner)?;
        self.resume_program(catalog, objects, owner, world, budget)
    }

    /// Run only immediate statements. Budget exhaustion retains the next live
    /// cursor and returns an error; it must never manufacture a movement tick.
    pub fn resume_program(
        &mut self,
        catalog: &PathCatalog,
        objects: &mut ObjectStore,
        owner: ObjectId,
        world: &mut PathWorld<'_>,
        budget: usize,
    ) -> Result<ControlStep, ProgramError> {
        self.check_execution_owner(owner)?;
        for executed in 0..=budget {
            let actor = objects
                .get(owner)
                .ok_or(PathRuntimeError::MissingActor(owner))?;
            let cursor = actor
                .base
                .path
                .ok_or(PathRuntimeError::MissingPath(owner))?;
            if executed == budget {
                return Err(ProgramError::BudgetExceeded { cursor, executed });
            }
            let statement = catalog.statement(cursor)?;
            let outcome = match statement {
                Statement::Appearance { command, next } => {
                    let actor = objects.get_mut(owner).expect("validated appearance owner");
                    command.apply(actor);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::RunWhenPaused { enabled, next } => {
                    let actor = objects.get_mut(owner).expect("validated pause-mode owner");
                    actor.base.contacts.run_when_paused = enabled;
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::LatchPrimaryViewFilter { next } => {
                    let primary = world
                        .primary_player
                        .ok_or(ProgramError::MissingPrimaryPlayer)?;
                    let filtered = objects
                        .get(primary)
                        .ok_or(PathRuntimeError::MissingActor(primary))?
                        .base
                        .flags
                        .view_side_filter;
                    let actor = objects.get_mut(owner).expect("validated phase-latch owner");
                    if filtered {
                        let phase = &mut actor.extension.path_state.motion_phase;
                        *phase = (*phase & 0xFF00) | 1;
                    }
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::CopySelectedTransform { command, next } => {
                    super::path_relationships::copy_selected_transform(
                        objects,
                        owner,
                        world.selected,
                        command,
                    )
                    .map_err(ProgramError::Relationship)?;
                    objects
                        .get_mut(owner)
                        .expect("validated transform-copy owner")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::InheritPrimaryHorizontalMotion { next } => {
                    let primary = world
                        .primary_player
                        .ok_or(ProgramError::MissingPrimaryPlayer)?;
                    let input = world
                        .primary_motion
                        .ok_or(ProgramError::MissingPrimaryMotion)?;
                    let velocity = objects
                        .get(primary)
                        .ok_or(PathRuntimeError::MissingActor(primary))?
                        .base
                        .velocity;
                    let actor = objects.get_mut(owner).expect("validated inheritance owner");
                    super::path_motion::inherit_horizontal_motion(
                        actor,
                        velocity,
                        input.displacement,
                        input.auxiliary_mode,
                    );
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::PlayerControl { command, next } => {
                    use super::path_player_control::{primary_position, PlayerControlCommand};
                    let primary = world
                        .primary_player
                        .ok_or(ProgramError::MissingPrimaryPlayer)?;
                    let player = objects
                        .get(primary)
                        .ok_or(PathRuntimeError::MissingActor(primary))?;
                    let pose = (player.base.position, player.base.pitch, player.base.yaw);
                    let input = world
                        .primary_control
                        .as_mut()
                        .ok_or(ProgramError::MissingPrimaryControl)?;
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated player-control owner");
                    match command {
                        PlayerControlCommand::Configure(range) => {
                            input.target.configure(owner, actor.base.position, range)
                        }
                        PlayerControlCommand::ConfigureDoubledLowByte(range) => input
                            .target
                            .configure_doubled_low_byte(owner, actor.base.position, range),
                        PlayerControlCommand::ConfigureAlternateAxes(range) => input
                            .target
                            .configure_alternate_axes(owner, actor.base.position, range),
                        PlayerControlCommand::LockForLinkedMode => {
                            input.target.lock_for_linked_mode(input.linked_mode)
                        }
                        PlayerControlCommand::FollowPrimaryPosition => {
                            actor.base.position =
                                primary_position(pose.0, pose.1, pose.2, input.linked_mode)
                        }
                        PlayerControlCommand::RefreshOwnedOrigin => input
                            .target
                            .refresh_owned_origin(owner, actor.base.position),
                    }
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::Sound { cue, next } => {
                    world
                        .audio
                        .as_mut()
                        .ok_or(ProgramError::MissingAudio)?
                        .queue(cue, self.selected_player());
                    objects
                        .get_mut(owner)
                        .expect("validated sound owner")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::SpatialLoop { sound, next } => {
                    let actor = objects.get_mut(owner).expect("validated sound owner");
                    actor.extension.spatial_loop = sound;
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::Countdown { command, next } => {
                    let countdown = world
                        .countdown
                        .as_mut()
                        .ok_or(ProgramError::MissingCountdown)?;
                    let actor = objects.get_mut(owner).expect("validated countdown owner");
                    countdown.apply(actor, command);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::MarkerSound { id, mode, next } => {
                    let source = objects
                        .get(owner)
                        .expect("validated sound owner")
                        .base
                        .position;
                    world
                        .audio
                        .as_mut()
                        .ok_or(ProgramError::MissingAudio)?
                        .queue_marker(id, mode, source, self.selected_player())
                        .map_err(|_| ProgramError::MissingSoundMarkers)?;
                    objects
                        .get_mut(owner)
                        .expect("validated sound owner")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::SpawnChild {
                    kind,
                    parameters,
                    next,
                } => {
                    let defaults = world
                        .spawn_defaults
                        .ok_or(ProgramError::MissingSpawnDefaults)?;
                    // Validate the independent child's native entry before
                    // allocating; absent catalog coverage is never a no-op.
                    if let Some(path) = parameters.path {
                        catalog.statement(path)?;
                    }
                    self.spawns
                        .child(objects, owner, kind, parameters, defaults)
                        .map_err(ProgramError::Spawn)?;
                    objects
                        .get_mut(owner)
                        .expect("validated spawn caller")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::Relationship { command, next } => {
                    super::path_relationships::apply(objects, owner, command)
                        .map_err(ProgramError::Relationship)?;
                    objects
                        .get_mut(owner)
                        .expect("validated relationship owner")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::SelectedAuxiliaryBranch {
                    condition,
                    taken,
                    next,
                } => {
                    let input = world
                        .selected_auxiliary
                        .ok_or(ProgramError::MissingSelectedAuxiliary)?;
                    self.execute_branch(
                        objects,
                        owner,
                        BranchCommand::Test {
                            predicate: condition.sample(input),
                            taken,
                            next,
                        },
                    )
                }
                Statement::Random { mutation, next } => {
                    self.execute_random(objects, owner, world.random, mutation, next)
                }
                Statement::DisableCollision { next } => {
                    self.execute_disable_collision(objects, owner, next)
                }
                Statement::Animation { command, next } => {
                    self.execute_animation(objects, owner, command, next)
                }
                Statement::Sprite { color, size, next } => {
                    self.execute_sprite(objects, owner, color, size, next)
                }
                Statement::Control(command) => self.execute_control(objects, owner, command),
                Statement::Branch(command) => self.execute_branch(objects, owner, command),
                Statement::Motion { command, next } => {
                    self.execute_motion(objects, owner, command, next)
                }
                Statement::Contact { command, next } => {
                    let actor = objects.get_mut(owner).expect("validated contact owner");
                    command.apply(actor);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::Facing { command, next } => self.execute_facing(
                    objects,
                    owner,
                    command,
                    super::path_steering::FacingTargets {
                        selected: world.selected,
                        primary: world.primary_player,
                        fixed_players: world.fixed_players,
                    },
                    next,
                ),
                Statement::Mutate { mutation, next } => {
                    self.execute_mutation(objects, owner, mutation, next)
                }
                Statement::Wait { duration, next } => {
                    let duration = duration.read(actor);
                    self.execute_control(objects, owner, ControlCommand::Wait { duration, next })
                }
                Statement::WaitChase {
                    field,
                    target,
                    next,
                } => {
                    let current = field.read(actor);
                    let target = target.read(actor);
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated waiting chase owner");
                    field.write(actor, super::path_fields::chase_byte(current, target));
                    if current == target {
                        actor.base.path = Some(next);
                        Ok(ControlStep::Continue)
                    } else {
                        Ok(ControlStep::Movement)
                    }
                }
                Statement::Repeat {
                    count,
                    target,
                    next,
                } => {
                    let count = count.read(actor);
                    self.execute_control(
                        objects,
                        owner,
                        ControlCommand::Repeat {
                            count,
                            target,
                            next,
                        },
                    )
                }
                Statement::BeginLoop { iterations, next } => {
                    let iterations = iterations.read(actor);
                    self.execute_control(
                        objects,
                        owner,
                        ControlCommand::BeginLoop { iterations, next },
                    )
                }
                Statement::Compare {
                    condition,
                    taken,
                    next,
                } => {
                    let predicate = condition.sample(actor);
                    self.execute_branch(
                        objects,
                        owner,
                        BranchCommand::Test {
                            predicate,
                            taken,
                            next,
                        },
                    )
                }
                Statement::Spatial {
                    condition,
                    taken,
                    next,
                } => self.execute_spatial_branch(
                    objects,
                    owner,
                    world.selected,
                    condition,
                    taken,
                    next,
                ),
            }?;
            if outcome != ControlStep::Continue {
                super::path_appearance::publish_animation(
                    objects.get_mut(owner).expect("executed actor remains live"),
                    world.animation_clock,
                );
                return Ok(outcome);
            }
        }
        unreachable!("inclusive budget iteration always returns")
    }
}

#[cfg(test)]
mod tests {
    use super::super::path_control::PlayerTarget;
    use super::super::path_fields::{ByteField, ByteOperation, WordField};
    use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
    use super::super::path_triggers::{Trigger, TriggerKind};
    use super::super::{Behavior, ObjectKind, PathId, ShapeId};
    use super::*;

    fn cursor(path: u16, command_index: u16) -> PathCursor {
        PathCursor {
            path: PathId::from_catalog_index(path),
            command_index,
        }
    }

    fn world(random: &mut RandomState) -> PathWorld<'_> {
        PathWorld {
            audio: None,
            primary_player: None,
            selected: None,
            fixed_players: [None; 2],
            primary_motion: None,
            primary_control: None,
            countdown: None,
            selected_auxiliary: None,
            spawn_defaults: None,
            random,
            animation_clock: 0,
        }
    }

    fn setup() -> (PathRuntime, ObjectStore, ObjectId, RandomState) {
        let mut objects = ObjectStore::new();
        let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
        actor.base.path = Some(cursor(0, 0));
        let owner = objects.allocate(actor).unwrap();
        (
            PathRuntime::default(),
            objects,
            owner,
            RandomState::default(),
        )
    }

    #[test]
    fn positional_loop_writes_are_retained_not_queued_and_zero_disables_them() {
        use super::super::SpatialLoop;
        for initial in [
            None,
            Some(SpatialLoop::CapitalEngine),
            SpatialLoop::from_authored_control(255),
        ] {
            for value in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                runtime.branch.invert_next = true;
                let actor = objects.get_mut(owner).unwrap();
                actor.base.wait_timer = 59;
                actor.extension.spatial_loop = initial;
                actor.extension.animation_frame = 123;
                actor.extension.texture_scroll_x = 87;
                let initial_random = random.clone();
                let sound = SpatialLoop::from_authored_control(value);
                assert_eq!(sound.map_or(0, SpatialLoop::authored_control), value);
                assert_eq!(sound.is_none(), value == 0);
                let mut expected = actor.clone();
                expected.extension.spatial_loop = sound;
                expected.base.path = Some(cursor(0, 1));
                let catalog = PathCatalog::new(vec![vec![Statement::SpatialLoop {
                    sound,
                    next: cursor(0, 1),
                }]])
                .unwrap();
                // No cue queue or listener is required for a retained write.
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1
                    ),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 1),
                        executed: 1
                    })
                );
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert_eq!(random, initial_random);
                assert!(runtime.branch.invert_next);
            }
        }
    }

    #[test]
    fn countdown_statements_require_shared_state_before_mutating_and_preserve_ifnot() {
        use super::super::path_countdown::{CountdownCommand, PathCountdown};
        use super::super::path_fields::ByteField;
        for command in [
            CountdownCommand::CopyTo(ByteField::Part),
            CountdownCommand::Assign(ByteOperand::Actor(ByteField::Part)),
            CountdownCommand::Increment,
            CountdownCommand::Decrement,
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.branch.invert_next = true;
            objects.get_mut(owner).unwrap().extension.path_state.part = 211;
            let before = objects.get(owner).unwrap().clone();
            let original_random = random.clone();
            let catalog = PathCatalog::new(vec![vec![Statement::Countdown {
                command,
                next: cursor(0, 1),
            }]])
            .unwrap();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingCountdown)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
            let mut countdown = PathCountdown { remaining: 143 };
            let mut expected = before;
            let expected_value = match command {
                CountdownCommand::CopyTo(_) => {
                    expected.extension.path_state.part = 143;
                    143
                }
                CountdownCommand::Assign(_) => 211,
                CountdownCommand::Increment => 144,
                CountdownCommand::Decrement => 142,
            };
            expected.base.path = Some(cursor(0, 1));
            let mut inputs = world(&mut random);
            inputs.countdown = Some(&mut countdown);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert_eq!(countdown.remaining, expected_value);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
        }
    }

    #[test]
    fn complete_shared_countdown_root_samples_live_state_and_stops_at_zero() {
        use super::super::path_countdown::PathCountdown;
        use super::super::{authored_paths, collision_pass::ExclusionGroups};
        let catalog = authored_paths::catalog();
        for initial in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let original_random = random.clone();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::SHARED_COUNTDOWN_SERVICE);
            actor.base.wait_timer = 63;
            actor.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(0xF8);
            actor.base.contacts.credits_hit_side = true;
            actor.base.contacts.mutually_non_damaging = true;
            actor.base.contacts.first_strategy_visit = true;
            actor.base.contacts.suppress_attack_damage = true;
            actor.base.velocity = super::super::Vector3 {
                x: 12,
                y: -31,
                z: 7,
            };
            let mut expected = actor.clone();
            expected.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(0xE8);
            expected.base.contacts.run_when_paused = true;
            expected.base.flags.visible = false;
            expected.base.flags.collision_disabled = true;
            expected.base.path = Some(cursor(0, 4));
            let mut countdown = PathCountdown { remaining: initial };
            for visit in 0..(u16::from(initial) + 3) {
                let before = countdown.remaining;
                expected.extension.path_state.part = before;
                let mut inputs = world(&mut random);
                inputs.countdown = Some(&mut countdown);
                assert_eq!(
                    runtime
                        .resume_program(&catalog, &mut objects, owner, &mut inputs, 8)
                        .unwrap(),
                    ControlStep::Movement
                );
                assert_eq!(countdown.remaining, before.saturating_sub(1));
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert_eq!(random, original_random);
                // Reassign the shared record after it has already reached
                // zero: the next loop must not reuse its old actor snapshot.
                if visit == u16::from(initial) + 1 {
                    countdown.remaining = 177;
                }
            }
        }
    }

    #[test]
    fn facing_statements_sample_live_targets_without_advancing_motion_or_other_state() {
        use super::super::path_steering::{face, FacingCommand, FacingTargets};
        use super::super::{Angle, Vector3};
        for command in [
            FacingCommand::SelectedImmediate,
            FacingCommand::SelectedSmooth,
            FacingCommand::SelectedYaw,
            FacingCommand::FixedPlayerImmediate,
            FacingCommand::LinkedSmooth,
            FacingCommand::LinkedImmediate,
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let targets = [(100, 50, 0), (-100, -75, 10), (7, 200, -200)].map(|(x, y, z)| {
                let mut actor =
                    Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
                actor.base.position = Vector3 { x, y, z };
                objects.allocate(actor).unwrap()
            });
            let actor = objects.get_mut(owner).unwrap();
            actor.base.attachment = Some(targets[2]);
            actor.base.pitch = Angle::from_units(50);
            actor.base.yaw = Angle::from_units(70);
            actor.base.roll = Angle::from_units(90);
            actor.base.velocity = Vector3 {
                x: 11,
                y: -22,
                z: 33,
            };
            actor.base.wait_timer = 19;
            actor.base.hit_flags = 0xA5;
            runtime.branch.invert_next = true;
            runtime.steering.unchanged_axes = 7;
            let initial_random = random.clone();
            let catalog = PathCatalog::new(vec![vec![Statement::Facing {
                command,
                next: cursor(0, 0),
            }]])
            .unwrap();
            // Switch live selection AND move all candidate targets between
            // resumes. Fixed players remain distinct from the selected ID.
            for selected in [targets[0], targets[1], targets[2]] {
                let observations = FacingTargets {
                    selected: Some(selected),
                    primary: Some(targets[0]),
                    fixed_players: [Some(targets[1]), Some(targets[2])],
                };
                let mut expected_objects = objects.clone();
                let mut expected_steering = runtime.steering;
                face(
                    &mut expected_objects,
                    owner,
                    command,
                    observations,
                    &mut expected_steering,
                )
                .unwrap();
                let mut inputs = world(&mut random);
                inputs.selected = observations.selected;
                inputs.primary_player = observations.primary;
                inputs.fixed_players = observations.fixed_players;
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 0),
                        executed: 1
                    })
                );
                for id in [owner, targets[0], targets[1], targets[2]] {
                    assert_eq!(objects.get(id), expected_objects.get(id));
                }
                assert_eq!(runtime.steering, expected_steering);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, initial_random);
                for id in targets {
                    let position = &mut objects.get_mut(id).unwrap().base.position;
                    position.x = position.x.wrapping_add(200);
                    position.y = position.y.wrapping_sub(100);
                }
            }
        }
    }

    #[test]
    fn facing_missing_inputs_fault_but_absent_link_is_a_source_defined_advance() {
        use super::super::path_steering::{FacingCommand, SteeringError};
        for (command, error) in [
            (
                FacingCommand::SelectedImmediate,
                Some(SteeringError::MissingSelected),
            ),
            (
                FacingCommand::FixedPlayerImmediate,
                Some(SteeringError::MissingFixedPlayer),
            ),
            (FacingCommand::LinkedImmediate, None),
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.steering.unchanged_axes = 11;
            runtime.branch.invert_next = true;
            let mut expected = objects.get(owner).unwrap().clone();
            let catalog = PathCatalog::new(vec![vec![Statement::Facing {
                command,
                next: cursor(0, 1),
            }]])
            .unwrap();
            let result =
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1);
            if let Some(error) = error {
                assert_eq!(
                    result,
                    Err(ProgramError::Runtime(PathRuntimeError::Steering(error)))
                );
            } else {
                expected.base.path = Some(cursor(0, 1));
                assert_eq!(
                    result,
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 1),
                        executed: 1
                    })
                );
            }
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert_eq!(runtime.steering.unchanged_axes, 11);
            assert!(runtime.branch.invert_next);
        }
    }

    #[test]
    fn linked_rotation_refresh_advances_without_selection_and_faults_on_dangling_link() {
        use super::super::path_relationships::{RelationshipCommand, RelationshipError};
        use super::super::{Angle, Rotation};
        let (mut runtime, mut objects, owner, mut random) = setup();
        let linked = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let original_random = random;
        runtime.branch.invert_next = true;
        let catalog = PathCatalog::new(vec![vec![Statement::Relationship {
            command: RelationshipCommand::RefreshLinkedRotation,
            next: cursor(0, 1),
        }]])
        .unwrap();
        for link in [Some(linked), None, Some(owner), Some(linked)] {
            let target = objects.get_mut(linked).unwrap();
            target.base.pitch = Angle::from_units(90);
            target.base.yaw = Angle::from_units(160);
            target.base.roll = Angle::from_units(230);
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(cursor(0, 0));
            actor.base.attachment = link;
            actor.base.pitch = Angle::from_units(10);
            actor.base.yaw = Angle::from_units(20);
            actor.base.roll = Angle::from_units(30);
            actor.base.wait_timer = 99;
            actor.extension.relative_rotation = Rotation {
                pitch: Angle::from_units(1),
                yaw: Angle::from_units(2),
                roll: Angle::from_units(3),
            };
            let mut expected = actor.clone();
            if link == Some(linked) {
                expected.extension.relative_rotation = Rotation {
                    pitch: Angle::from_units(176),
                    yaw: Angle::from_units(116),
                    roll: Angle::from_units(56),
                };
            } else if link == Some(owner) {
                expected.extension.relative_rotation = Rotation::default();
            }
            expected.base.path = Some(cursor(0, 1));
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert!(runtime.branch.invert_next);
        }
        objects.remove(linked).unwrap();
        // Normal removal clears inbound links. Deliberately construct an
        // invalid retained link to exercise the diagnostic boundary.
        objects.get_mut(owner).unwrap().base.attachment = Some(linked);
        objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
        let before = objects.get(owner).unwrap().clone();
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::Relationship(RelationshipError::MissingActor(
                linked
            )))
        );
        assert_eq!(objects.get(owner).unwrap(), &before);
        assert_eq!(random, original_random);
    }

    #[test]
    fn selected_transform_copies_read_live_selection_and_preserve_neighboring_fields() {
        use super::super::path_relationships::{RelationshipError, SelectedTransformCommand};
        use super::super::{Angle, Vector3};
        for command in [
            SelectedTransformCommand::WorldPosition,
            SelectedTransformCommand::WorldRotation,
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let other = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.child_number = 73;
            actor.base.wait_timer = 57;
            actor.base.position = Vector3 {
                x: 100,
                y: -200,
                z: 300,
            };
            actor.base.velocity = Vector3 { x: 1, y: -2, z: 3 };
            actor.extension.relative_position = Vector3 {
                x: 40,
                y: -50,
                z: 60,
            };
            actor.extension.path_state.motion.relative_coordinates = true;
            actor.extension.parent = Some(other);
            runtime.branch.invert_next = true;
            let original_random = random;
            let catalog = PathCatalog::new(vec![vec![Statement::CopySelectedTransform {
                command,
                next: cursor(0, 1),
            }]])
            .unwrap();
            let before = objects.get(owner).unwrap().clone();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::Relationship(
                    RelationshipError::MissingSelected
                ))
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            for angle in 0..=u8::MAX {
                let target = objects.get_mut(other).unwrap();
                target.base.position = Vector3 {
                    x: (u16::from(angle) * 257) as i16,
                    y: i16::MIN,
                    z: i16::MAX,
                };
                target.base.pitch = Angle::from_units(angle);
                target.base.yaw = Angle::from_units(angle.wrapping_add(100));
                target.base.roll = Angle::from_units(angle.wrapping_sub(50));
                // Child number and elapsed wait sit between source angle
                // bytes, so a bulk rotation-word copy would be incorrect.
                target.base.child_number = angle;
                target.base.wait_timer = angle;
                for selected in [other, owner] {
                    objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
                    let mut expected = objects.get(owner).unwrap().clone();
                    let target = objects.get(selected).unwrap();
                    match command {
                        SelectedTransformCommand::WorldPosition => {
                            expected.base.position = target.base.position
                        }
                        SelectedTransformCommand::WorldRotation => {
                            expected.base.pitch = target.base.pitch;
                            expected.base.yaw = target.base.yaw;
                            expected.base.roll = target.base.roll;
                        }
                    }
                    expected.base.path = Some(cursor(0, 1));
                    let other_before = objects.get(other).unwrap().clone();
                    let mut inputs = world(&mut random);
                    inputs.selected = Some(selected);
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: cursor(0, 1),
                            executed: 1
                        })
                    );
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert_eq!(objects.get(other).unwrap(), &other_before);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(random, original_random);
                }
            }
        }
    }

    #[test]
    fn waiting_chase_checks_equality_before_update_and_does_not_reset_elapsed_wait() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![Statement::WaitChase {
            field: ByteField::ScriptParameter,
            target: ByteOperand::Actor(ByteField::Health),
            next: cursor(0, 1),
        }]])
        .unwrap();
        let original_random = random;
        runtime.branch.invert_next = true;
        for current in 0..=u8::MAX {
            for target in 0..=u8::MAX {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(cursor(0, 0));
                actor.extension.path_state.script_parameter = current;
                actor.base.hit_points = target;
                actor.base.wait_timer = 57;
                actor.base.velocity.x = 123;
                let mut expected = actor.clone();
                expected.extension.path_state.script_parameter =
                    super::super::path_fields::chase_byte(current, target);
                let result = runtime.resume_program(
                    &catalog,
                    &mut objects,
                    owner,
                    &mut world(&mut random),
                    1,
                );
                if current == target {
                    expected.base.path = Some(cursor(0, 1));
                    assert_eq!(
                        result,
                        Err(ProgramError::BudgetExceeded {
                            cursor: cursor(0, 1),
                            executed: 1
                        })
                    );
                } else {
                    // Even a final one-unit step yields at the same cursor.
                    assert_eq!(result, Ok(ControlStep::Movement));
                }
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, original_random);
            }
        }
    }

    #[test]
    fn primary_horizontal_inheritance_samples_mode_and_motion_without_movement() {
        use super::super::Vector3;
        for own_primary in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let other = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            let primary = if own_primary { owner } else { other };
            objects.get_mut(other).unwrap().base.velocity = Vector3 {
                x: 2345,
                y: -32,
                z: -6789,
            };
            let actor = objects.get_mut(owner).unwrap();
            actor.base.velocity = Vector3 {
                x: i16::MAX,
                y: 42,
                z: i16::MIN,
            };
            actor.base.position = Vector3 {
                x: 101,
                y: 202,
                z: 303,
            };
            actor.base.wait_timer = 57;
            runtime.branch.invert_next = true;
            let initial_random = random.clone();
            let catalog = PathCatalog::new(vec![vec![Statement::InheritPrimaryHorizontalMotion {
                next: cursor(0, 0),
            }]])
            .unwrap();
            for mode in 0..=u8::MAX {
                let displacement = Vector3 {
                    x: i16::MIN + i16::from(mode),
                    y: 999,
                    z: i16::MAX - i16::from(mode),
                };
                let mut expected = objects.get(owner).unwrap().clone();
                let other_before = objects.get(other).unwrap().clone();
                let addition = if (16..32).contains(&mode) {
                    objects.get(primary).unwrap().base.velocity
                } else {
                    displacement
                };
                expected.base.velocity.x = expected.base.velocity.x.wrapping_add(addition.x);
                expected.base.velocity.z = expected.base.velocity.z.wrapping_add(addition.z);
                let mut inputs = world(&mut random);
                inputs.primary_player = Some(primary);
                // Deliberately select a different actor: this action must
                // always use primary_player, including owner/primary aliasing.
                inputs.selected = Some(if own_primary { other } else { owner });
                inputs.primary_motion = Some(PrimaryMotionInput {
                    auxiliary_mode: mode,
                    displacement,
                });
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 0),
                        executed: 1
                    })
                );
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert_eq!(objects.get(other).unwrap(), &other_before);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, initial_random);
            }
        }
    }

    #[test]
    fn inheritance_missing_world_inputs_fault_before_mutating_actor() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![Statement::InheritPrimaryHorizontalMotion {
            next: cursor(0, 1),
        }]])
        .unwrap();
        let before = objects.get(owner).unwrap().clone();
        for (primary, error) in [
            (None, ProgramError::MissingPrimaryPlayer),
            (Some(owner), ProgramError::MissingPrimaryMotion),
        ] {
            let mut inputs = world(&mut random);
            inputs.primary_player = primary;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(error)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
        }
    }

    fn spawn_statement() -> Statement {
        Statement::SpawnChild {
            kind: ObjectKind::Effect,
            parameters: super::super::path_spawn::ChildSpawn {
                shape: ShapeId::from_catalog_index(9),
                path: Some(cursor(1, 0)),
                position: super::super::Vector3 {
                    x: -123,
                    y: 456,
                    z: -789,
                },
                rotation: super::super::Rotation::default(),
                hit_points: 1,
                attack_power: 2,
                number: 3,
            },
            next: cursor(0, 1),
        }
    }

    #[test]
    fn shape_assignment_and_equality_use_semantic_ids_without_reinitializing_actor() {
        use super::super::path_appearance::AppearanceCommand;
        let (mut runtime, mut objects, owner, mut random) = setup();
        let original_random = random;
        for index in 0..577 {
            for equal in [false, true] {
                for inverted in [false, true] {
                    let shape = ShapeId::from_catalog_index(index);
                    let other =
                        ShapeId::from_catalog_index(if equal { index } else { (index + 1) % 577 });
                    let catalog = PathCatalog::new(vec![vec![
                        Statement::Appearance {
                            command: AppearanceCommand::Shape(shape),
                            next: cursor(0, 1),
                        },
                        Statement::Compare {
                            condition: ActorCondition::EqualShape(other),
                            taken: cursor(0, 2),
                            next: cursor(0, 3),
                        },
                        Statement::Control(ControlCommand::End),
                        Statement::Control(ControlCommand::End),
                    ]])
                    .unwrap();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(cursor(0, 0));
                    actor.base.flags.visible = false;
                    actor.base.flags.scaled_sprite = true;
                    actor.base.flags.casts_shadow = true;
                    actor.base.flags.collision_disabled = true;
                    actor.extension.texture_scroll_x = 67;
                    actor.extension.animation_frame = 12;
                    actor.extension.path_state.motion_phase = 0xABCD;
                    let mut expected = actor.clone();
                    expected.base.shape = shape;
                    let next = cursor(0, if equal != inverted { 2 } else { 3 });
                    expected.base.path = Some(next);
                    runtime.branch.invert_next = inverted;
                    assert_eq!(
                        runtime.resume_program(
                            &catalog,
                            &mut objects,
                            owner,
                            &mut world(&mut random),
                            2
                        ),
                        Err(ProgramError::BudgetExceeded {
                            cursor: next,
                            executed: 2
                        })
                    );
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert!(!runtime.branch.invert_next);
                    assert_eq!(random, original_random);
                }
            }
        }
    }

    #[test]
    fn appearance_controls_advance_immediately_without_consuming_contact_or_inversion() {
        use super::super::path_appearance::AppearanceCommand;
        let commands = [
            AppearanceCommand::Visibility(false),
            AppearanceCommand::Collision(true),
            AppearanceCommand::Shadow(true),
            AppearanceCommand::MaximumDrawDistance(true),
            AppearanceCommand::Visibility(true),
            AppearanceCommand::Shadow(false),
            AppearanceCommand::MaximumDrawDistance(false),
            AppearanceCommand::FarSortBias(true),
            AppearanceCommand::FarSortBias(false),
        ];
        let mut statements = commands
            .iter()
            .enumerate()
            .map(|(index, command)| Statement::Appearance {
                command: *command,
                next: cursor(0, index as u16 + 1),
            })
            .collect::<Vec<_>>();
        statements.push(Statement::Control(ControlCommand::End));
        let catalog = PathCatalog::new(vec![statements]).unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        runtime.branch.invert_next_condition();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.flags.draw_list_admitted = true;
        actor.base.flags.collided = true;
        actor.base.hit_flags = 0xA5;
        actor.base.wait_timer = 19;
        actor.extension.path_state.conditions.hit_event_pending = true;
        let original_random = random;
        for (index, expected_flags) in [
            (false, true, false, false),
            (false, false, false, false),
            (false, false, true, false),
            (false, false, true, true),
            (true, false, true, true),
            (true, false, false, true),
            (true, false, false, false),
            (true, false, false, false),
            (true, false, false, false),
        ]
        .into_iter()
        .enumerate()
        {
            let mut expected = objects.get(owner).unwrap().clone();
            let flags = &mut expected.base.flags;
            (
                flags.visible,
                flags.collision_disabled,
                flags.casts_shadow,
                flags.maximum_draw_distance,
            ) = expected_flags;
            flags.far_sort_bias = index == 7;
            let next = cursor(0, index as u16 + 1);
            expected.base.path = Some(next);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: next,
                    executed: 1
                })
            );
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
        }
    }

    #[test]
    fn indexed_word_bit_branches_resample_live_fields_and_leave_ifnot_pending() {
        const MASKS: [u16; 128] = {
            let mut masks = [0; 128];
            let mut index = 0;
            while index < masks.len() {
                masks[index] = (index as u16 * 257) ^ 0x5AA5;
                index += 1;
            }
            masks
        };
        let catalog = PathCatalog::new(vec![vec![
            Statement::Compare {
                condition: ActorCondition::AnyWordBitsSet(
                    WordOperand::Actor(WordField::MotionPhase),
                    WordOperand::IndexedBitMask {
                        selector: ByteOperand::Actor(ByteField::Health),
                        masks: &MASKS,
                    },
                ),
                taken: cursor(0, 1),
                next: cursor(0, 2),
            },
            Statement::Control(ControlCommand::End),
            Statement::Control(ControlCommand::End),
        ]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let original_random = random;
        for selector in 0..=u8::MAX {
            let mask = MASKS[usize::from(selector.wrapping_sub(1).wrapping_mul(2)) / 2];
            for value in [0, mask, !mask, u16::MAX] {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(cursor(0, 0));
                actor.base.hit_points = selector;
                actor.extension.path_state.motion_phase = value;
                runtime.branch.invert_next_condition();
                let mut expected = actor.clone();
                let next = cursor(0, if value & mask != 0 { 1 } else { 2 });
                expected.base.path = Some(next);
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1
                    ),
                    Err(ProgramError::BudgetExceeded {
                        cursor: next,
                        executed: 1
                    })
                );
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert!(runtime.branch.invert_next);
            }
        }
        assert_eq!(random, original_random);
    }

    #[test]
    fn marker_sound_faults_before_mutation_and_resamples_fixed_marker_inputs() {
        use super::super::path_sound::{
            marker_cue, CueListener, CueMarker, MarkerCueMode, MarkerInputs, MarkerRange,
            PathAudio, PathSoundClass,
        };
        use super::super::{Angle, AudioState, SoundEvent, Vector3};
        for mode in [
            MarkerCueMode::DistanceBands(PathSoundClass::DistanceOnly),
            MarkerCueMode::DistanceBands(PathSoundClass::Positioned),
            MarkerCueMode::RangeLimited(MarkerRange::Near),
            MarkerCueMode::RangeLimited(MarkerRange::Wide),
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let original_random = random;
            let catalog = PathCatalog::new(vec![vec![Statement::MarkerSound {
                id: 255,
                mode,
                next: cursor(0, 1),
            }]])
            .unwrap();
            let before = objects.get(owner).unwrap().clone();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingAudio)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            let mut audio = AudioState::default();
            audio.queue(SoundEvent::HostileLaser);
            let mut inputs = world(&mut random);
            inputs.audio = Some(PathAudio {
                events: &mut audio,
                listeners: [CueListener::Other, CueListener::PrimaryFallback],
                markers: None,
            });
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::MissingSoundMarkers)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            assert_eq!(
                inputs
                    .audio
                    .as_mut()
                    .unwrap()
                    .events
                    .take_events()
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>(),
                vec![SoundEvent::HostileLaser]
            );
            for selected in [PlayerTarget::Primary, PlayerTarget::Secondary] {
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .selected_player = selected;
                runtime.enter(&objects, owner).unwrap();
                // The retained runtime selection must survive a change in the
                // owner's selection flag. Marker side is independently live.
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .selected_player = if selected == PlayerTarget::Primary {
                    PlayerTarget::Secondary
                } else {
                    PlayerTarget::Primary
                };
                for side in [PlayerTarget::Primary, PlayerTarget::Secondary] {
                    for distance in [0_i16, 799, 800, 1299, 1300, 5119, 5120, 32767] {
                        let markers = [
                            CueMarker {
                                identity: CueListener::PrimaryFallback,
                                position: Vector3 {
                                    x: 100,
                                    y: i16::MAX,
                                    z: 200,
                                },
                                bearing: Angle::from_units(64),
                            },
                            CueMarker {
                                identity: CueListener::Other,
                                position: Vector3 {
                                    x: -300,
                                    y: i16::MIN,
                                    z: -400,
                                },
                                bearing: Angle::from_units(192),
                            },
                        ];
                        let opposite = if side == PlayerTarget::Primary {
                            PlayerTarget::Secondary
                        } else {
                            PlayerTarget::Primary
                        };
                        let selected_sides = if selected == PlayerTarget::Primary {
                            [side, opposite]
                        } else {
                            [opposite, side]
                        };
                        let marker = markers[usize::from(side == PlayerTarget::Secondary)];
                        inputs.audio.as_mut().unwrap().markers = Some(MarkerInputs {
                            selected_sides,
                            markers,
                        });
                        let actor = objects.get_mut(owner).unwrap();
                        actor.base.path = Some(cursor(0, 0));
                        actor.base.position = Vector3 {
                            x: marker.position.x,
                            y: 777,
                            z: marker.position.z.wrapping_add(distance),
                        };
                        actor.base.wait_timer = 57;
                        actor.base.yaw = Angle::from_units(17);
                        let mut expected = actor.clone();
                        expected.base.path = Some(cursor(0, 1));
                        let expected_cue = marker_cue(255, mode, actor.base.position, marker);
                        inputs
                            .audio
                            .as_mut()
                            .unwrap()
                            .events
                            .queue(SoundEvent::HostileLaser);
                        runtime.branch.invert_next = true;
                        assert_eq!(
                            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                            Err(ProgramError::BudgetExceeded {
                                cursor: cursor(0, 1),
                                executed: 1
                            })
                        );
                        assert_eq!(objects.get(owner).unwrap(), &expected);
                        assert!(runtime.branch.invert_next);
                        assert_eq!(runtime.selected_player(), selected);
                        let mut expected_events = vec![SoundEvent::HostileLaser];
                        expected_events.extend(expected_cue.map(SoundEvent::Authored));
                        assert_eq!(
                            inputs
                                .audio
                                .as_mut()
                                .unwrap()
                                .events
                                .take_events()
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>(),
                            expected_events
                        );
                    }
                }
            }
            assert_eq!(inputs.random, &original_random);
        }
    }

    #[test]
    fn sound_requires_live_shared_audio_and_routes_using_retained_selection() {
        use super::super::path_sound::{AuthoredCue, CueListener, PathAudio};
        use super::super::{AudioState, SoundEvent};
        let (mut runtime, mut objects, owner, mut random) = setup();
        let cue = AuthoredCue::new(18, 127, PlayerTarget::Primary);
        let catalog = PathCatalog::new(vec![vec![
            Statement::Sound {
                cue,
                next: cursor(0, 1),
            },
            Statement::Control(ControlCommand::WaitOne { next: cursor(0, 0) }),
        ]])
        .unwrap();
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 4),
            Err(ProgramError::MissingAudio)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 0)));
        let before_random = random;
        let mut audio = AudioState::default();
        audio.queue(SoundEvent::HostileLaser);
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = PlayerTarget::Secondary;
        let mut inputs = world(&mut random);
        inputs.audio = Some(PathAudio {
            events: &mut audio,
            listeners: [CueListener::Other, CueListener::PrimaryFallback],
            markers: None,
        });
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 4),
            Ok(ControlStep::Movement)
        );
        // Immediate/callback resume uses the retained selected slot, not the
        // owner's subsequently changed condition or spatial selected actor.
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = PlayerTarget::Primary;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 4),
            Ok(ControlStep::Movement)
        );
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 4),
            Ok(ControlStep::Movement)
        );
        assert_eq!(inputs.random, &before_random);
        assert_eq!(
            inputs
                .audio
                .as_mut()
                .unwrap()
                .events
                .take_events()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            vec![
                SoundEvent::HostileLaser,
                SoundEvent::Authored(cue),
                SoundEvent::Authored(cue),
                SoundEvent::Authored(cue.for_listener(CueListener::Other))
            ]
        );
    }

    #[test]
    fn primary_view_latch_preserves_high_phase_and_does_not_use_selected_player() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let primary = objects
            .allocate(Object::new(
                ObjectKind::Enemy,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let catalog = PathCatalog::new(vec![vec![
            Statement::LatchPrimaryViewFilter { next: cursor(0, 1) },
            Statement::Control(ControlCommand::End),
        ]])
        .unwrap();
        let before_random = random;
        let mut inputs = world(&mut random);
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::MissingPrimaryPlayer)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 0)));
        inputs.primary_player = Some(primary);
        inputs.selected = Some(owner);
        runtime.branch.invert_next = true;
        for filtered in [false, true] {
            objects
                .get_mut(primary)
                .unwrap()
                .base
                .flags
                .view_side_filter = filtered;
            for high in [0, 0x7F00, 0x8000, 0xFF00] {
                for low in 0..=u8::MAX {
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(cursor(0, 0));
                    actor.base.flags.view_side_filter = !filtered;
                    actor.extension.path_state.motion_phase = high | u16::from(low);
                    actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
                    let mut expected = actor.clone();
                    expected.base.path = Some(cursor(0, 1));
                    if filtered {
                        expected.extension.path_state.motion_phase = high | 1;
                    }
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: cursor(0, 1),
                            executed: 1
                        })
                    );
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
                    assert!(runtime.branch.invert_next);
                }
            }
        }
        assert_eq!(inputs.random, &before_random);
        objects.remove(primary).unwrap();
        objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::Runtime(PathRuntimeError::MissingActor(
                primary
            )))
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 0)));
    }

    #[test]
    fn authored_callback_sprite_loops_then_primary_filter_or_aux_action_redirects_to_end() {
        use super::super::authored_paths;
        for primary_filtered in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let primary = objects
                .allocate(Object::new(
                    ObjectKind::Enemy,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            let catalog = authored_paths::catalog();
            let initial_random = random;
            {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::CALLBACK_GATED_SPRITE);
                actor.extension.path_state.motion_phase = 0xAB00;
                actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
            }
            for visit in 0..5 {
                let mut inputs = world(&mut random);
                inputs.primary_player = Some(primary);
                inputs.selected = Some(owner);
                inputs.selected_auxiliary = Some(AuxiliaryContinuationInput {
                    mode: 0,
                    action_flags: 0x40,
                });
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 16),
                    Ok(ControlStep::Movement)
                );
                let saved_main = objects.get(owner).unwrap().base.path;
                let actor = objects.get(owner).unwrap();
                assert!(actor.base.contacts.run_when_paused);
                assert!(actor.base.flags.collision_disabled);
                assert_eq!(actor.extension.path_state.motion_phase, 0xAB00);
                assert_eq!(
                    actor.extension.texture_scroll_x,
                    if visit < 3 { (visit + 1) * 4 } else { 0 }
                );
                assert_eq!(
                    actor
                        .extension
                        .path_state
                        .triggers
                        .entries(&runtime.resources, owner)
                        .unwrap()
                        .len(),
                    1
                );
                assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                assert!(matches!(
                    runtime
                        .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                        .unwrap(),
                    CallbackStep::Run(_)
                ));
                if visit == 4 {
                    objects
                        .get_mut(primary)
                        .unwrap()
                        .base
                        .flags
                        .view_side_filter = primary_filtered;
                    // A latched phase bypasses the auxiliary condition entirely.
                    inputs.selected_auxiliary =
                        (!primary_filtered).then_some(AuxiliaryContinuationInput {
                            mode: 0x40,
                            action_flags: 0,
                        });
                }
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 8),
                    Ok(ControlStep::ResumeCallbacks)
                );
                assert_eq!(
                    runtime
                        .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                        .unwrap(),
                    CallbackStep::Complete
                );
                assert_eq!(inputs.random, &initial_random);
                if visit < 4 {
                    assert_eq!(objects.get(owner).unwrap().base.path, saved_main);
                    assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
                } else {
                    let actor = objects.get(owner).unwrap();
                    assert_ne!(actor.base.path, saved_main);
                    assert_eq!(
                        actor.extension.path_state.motion_phase,
                        if primary_filtered { 0xAB01 } else { 0xAB00 }
                    );
                    assert_eq!(
                        catalog.statement(actor.base.path.unwrap()).unwrap(),
                        Statement::Control(ControlCommand::End)
                    );
                    assert!(!actor.base.flags.remove_after_tick);
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 2),
                        Ok(ControlStep::Ended)
                    );
                    assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
                }
            }
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }

    #[test]
    fn authored_repeated_children_run_independent_jitter_sound_and_color_paths() {
        use super::super::path_sound::{AuthoredCue, CueListener, PathAudio};
        use super::super::{
            authored_paths, path_appearance, AudioState, ObjectSpawnDefaults, SoundEvent,
        };
        let (mut runtime, mut objects, owner, mut random) = setup();
        {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::REPEATED_CHILD_SPRITE);
            actor.base.target_speed = 11;
            actor.extension.path_state.motion_phase = 0xAB00;
            actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
        }
        let before_random = random;
        let catalog = authored_paths::catalog();
        let mut audio = AudioState::default();
        let mut children = Vec::new();
        for invocation in 0..11 {
            let mut inputs = world(&mut random);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            // No audio is needed until the independently scheduled child runs.
            let outcome = runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 16)
                .unwrap();
            assert_eq!(
                outcome,
                if invocation < 10 {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            assert_eq!(random, before_random);
            if invocation % 5 == 0 {
                let child = runtime.spawns.last_spawn.unwrap();
                assert!(!children.contains(&child));
                children.push(child);
                assert!(
                    objects
                        .get(child)
                        .unwrap()
                        .extension
                        .path_state
                        .needs_path_initialization
                );
            }
            assert_eq!(objects.len(), 1 + children.len());
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase,
                0xAB00 | (4 - invocation % 5)
            );
            // DO snapshots its count; this must not shorten its remaining loop.
            objects.get_mut(owner).unwrap().base.target_speed = 1;
        }
        assert_eq!(children.len(), 3);
        assert_eq!(
            objects.get(owner).unwrap().base.first_child,
            Some(children[0])
        );
        assert_eq!(
            objects.get(children[0]).unwrap().base.next_sibling,
            Some(children[1])
        );
        assert_eq!(
            objects.get(children[1]).unwrap().base.next_sibling,
            Some(children[2])
        );
        let mut expected_random = random;
        for child in children {
            let mut jitter = || {
                let high = expected_random.next_byte();
                let low = expected_random.next_byte();
                (u16::from_be_bytes([high, low]) & 31) as i16 - 15
            };
            let position = super::super::Vector3 {
                x: jitter(),
                y: jitter(),
                z: jitter(),
            };
            for color in 0..4 {
                let mut inputs = world(&mut random);
                inputs.audio = Some(PathAudio {
                    events: &mut audio,
                    listeners: [CueListener::PrimaryPlayer, CueListener::Other],
                    markers: None,
                });
                let outcome = runtime
                    .enter_program(&catalog, &mut objects, child, &mut inputs, 16)
                    .unwrap();
                assert_eq!(
                    outcome,
                    if color < 3 {
                        ControlStep::Movement
                    } else {
                        ControlStep::Ended
                    }
                );
                let actor = objects.get_mut(child).unwrap();
                path_appearance::publish_animation(actor, 29);
                assert_eq!(actor.extension.color_frame, color);
                assert_eq!(actor.extension.texture_scroll_x, 254);
                assert_eq!(actor.extension.relative_position, position);
                assert!(!actor.extension.path_state.needs_path_initialization);
                assert_eq!(random, expected_random);
                assert_eq!(actor.base.flags.remove_after_tick, color == 3);
                let events = audio
                    .take_events()
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                assert_eq!(
                    events,
                    if color == 0 {
                        vec![SoundEvent::Authored(AuthoredCue::new(
                            18,
                            0,
                            PlayerTarget::Secondary,
                        ))]
                    } else {
                        vec![]
                    }
                );
            }
        }
    }

    #[test]
    fn spawning_continues_parent_immediately_and_child_runs_only_when_scheduled() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().extension.spawn_group = 45;
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = PlayerTarget::Secondary;
        let catalog = PathCatalog::new(vec![
            vec![
                spawn_statement(),
                Statement::Control(ControlCommand::WaitOne { next: cursor(0, 2) }),
                Statement::Control(ControlCommand::End),
            ],
            vec![
                Statement::Sprite {
                    color: 9,
                    size: 12,
                    next: cursor(1, 1),
                },
                Statement::Control(ControlCommand::End),
            ],
        ])
        .unwrap();
        let before_random = random;
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(super::super::ObjectSpawnDefaults {
            run_when_paused: true,
            group: 99,
        });
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8),
            Ok(ControlStep::Movement)
        );
        let child = runtime.spawns.last_spawn.unwrap();
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 2)));
        assert_eq!(objects.get(owner).unwrap().base.first_child, Some(child));
        assert_eq!(objects.get(child).unwrap().base.path, Some(cursor(1, 0)));
        assert_eq!(objects.get(child).unwrap().extension.texture_scroll_x, 0);
        assert_eq!(objects.get(child).unwrap().extension.spawn_group, 45);
        assert!(objects.get(child).unwrap().base.contacts.run_when_paused);
        assert!(
            objects
                .get(child)
                .unwrap()
                .extension
                .path_state
                .needs_path_initialization
        );
        assert!(!objects.get(child).unwrap().base.flags.casts_shadow);
        assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
        // Neither dispatch nor spawning runs movement or recursively ticks
        // the child; those remain the source strategy scheduler's boundaries.
        assert_eq!(
            objects.get(child).unwrap().base.position,
            super::super::Vector3::default()
        );
        inputs.spawn_defaults = None; // this child's path needs no spawn inputs
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, child, &mut inputs, 8),
            Ok(ControlStep::Ended)
        );
        assert_eq!(objects.get(child).unwrap().extension.texture_scroll_x, 12);
        assert!(
            !objects
                .get(child)
                .unwrap()
                .extension
                .path_state
                .needs_path_initialization
        );
        assert!(objects.get(child).unwrap().base.flags.casts_shadow);
        assert!(
            objects
                .get(child)
                .unwrap()
                .base
                .flags
                .exclude_from_shape_footprint_search
        );
        assert!(objects.get(child).unwrap().base.flags.maximum_draw_distance);
        assert!(objects.get(child).unwrap().base.contacts.latch_new_contact);
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 2)));
        assert_eq!(random, before_random);
        assert_eq!(runtime.spawns.last_spawn, Some(child));
    }

    #[test]
    fn missing_spawn_inputs_or_child_catalog_entry_preserve_allocation_and_caller_cursor() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![spawn_statement()]]).unwrap();
        let before = objects.clone();
        let mut inputs = world(&mut random);
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8),
            Err(ProgramError::MissingSpawnDefaults)
        );
        assert_eq!(objects, before);
        inputs.spawn_defaults = Some(super::super::ObjectSpawnDefaults::default());
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8),
            Err(ProgramError::MissingStatement(cursor(1, 0)))
        );
        assert_eq!(objects, before);
        assert_eq!(runtime.spawns.last_spawn, None);
    }

    #[test]
    fn spawn_failure_does_not_advance_to_a_successful_parent_continuation() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        for _ in 1..super::super::OBJECT_CAPACITY {
            objects
                .allocate(Object::new(
                    ObjectKind::Scenery,
                    ShapeId::EMPTY,
                    Behavior::Effect,
                ))
                .unwrap();
        }
        let before = objects.clone();
        runtime.spawns.last_spawn = Some(owner);
        let catalog = PathCatalog::new(vec![
            vec![spawn_statement(), Statement::Control(ControlCommand::End)],
            vec![Statement::Control(ControlCommand::End)],
        ])
        .unwrap();
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(super::super::ObjectSpawnDefaults::default());
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8),
            Err(ProgramError::Spawn(
                super::super::path_spawn::SpawnError::PoolExhausted
            ))
        );
        assert_eq!(objects, before);
        assert_eq!(runtime.spawns.last_spawn, None);
    }

    fn health(operation: ByteOperation, next: PathCursor) -> Statement {
        Statement::Mutate {
            mutation: Mutation::Byte {
                field: ByteField::Health,
                operation,
            },
            next,
        }
    }

    #[test]
    fn authored_detaching_sprite_only_unlinks_after_last_yield_and_without_action_gate() {
        use super::super::authored_paths;
        for gate in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let mut child = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
            child.base.attachment = Some(owner);
            child.base.child_number = 1;
            child.base.flags.remove_with_parent = true;
            child.extension.path_state.motion.attached_coordinates = true;
            let child = objects.allocate(child).unwrap();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::CHILD_DETACHING_SPRITE);
            actor.base.first_child = Some(child);
            actor.extension.path_state.motion.refresh_child_chain = true;
            actor.extension.path_state.motion_phase = 0xABCD;
            let catalog = authored_paths::catalog();
            let before_random = random;
            let visits = if gate { 2 } else { 3 };
            for visit in 0..visits {
                let mut inputs = world(&mut random);
                inputs.selected_auxiliary = Some(AuxiliaryContinuationInput {
                    mode: 0x80, // mode does not participate in the action-bit gate
                    action_flags: if gate { 0x40 } else { 0 },
                });
                let outcome = runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 16)
                    .unwrap();
                assert_eq!(
                    outcome,
                    if visit == visits - 1 {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                let linked = gate || visit != visits - 1;
                assert_eq!(
                    objects.get(child).unwrap().base.attachment,
                    linked.then_some(owner)
                );
                assert_eq!(
                    objects.get(owner).unwrap().base.first_child,
                    linked.then_some(child)
                );
                assert_eq!(
                    objects
                        .get(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .motion_phase,
                    0xAB00 | (visit + 1).min(2) as u16
                );
                assert_eq!(objects.get(owner).unwrap().extension.texture_scroll_x, 250);
            }
            assert_eq!(random, before_random);
            assert!(objects.get(child).is_some()); // detachment is not retirement
        }
    }

    #[test]
    fn auxiliary_action_gate_tests_only_action_bit_and_keeps_inversion() {
        for mode in 0..=u8::MAX {
            for action_flags in 0..=u8::MAX {
                let mut branch = super::super::path_conditions::BranchState { invert_next: true };
                assert_eq!(
                    branch.test(
                        SelectedAuxiliaryCondition::ActionBit40
                            .sample(AuxiliaryContinuationInput { mode, action_flags })
                    ),
                    action_flags & 0x40 != 0
                );
                assert!(branch.invert_next);
            }
        }
    }

    #[test]
    fn variable_loop_snapshots_unsigned_count_once_including_zero_wrap() {
        use super::super::path_fields::{WordField, WordOperation};
        for (initial, wide) in [
            (0, false),
            (1, false),
            (127, false),
            (128, false),
            (255, false),
            (257, true),
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.target_speed = initial as u8;
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .motion_phase = initial;
            let count = if wide {
                WordOperand::Actor(WordField::MotionPhase)
            } else {
                WordOperand::UnsignedByte(ByteOperand::Actor(ByteField::TargetSpeed))
            };
            let catalog = PathCatalog::new(vec![vec![
                Statement::BeginLoop {
                    iterations: count,
                    next: cursor(0, 1),
                },
                // The body overwrites BOTH possible count sources; the loop
                // must keep its saved count rather than sample either again.
                Statement::Mutate {
                    mutation: Mutation::Byte {
                        field: ByteField::TargetSpeed,
                        operation: ByteOperation::Assign(ByteOperand::Literal(0)),
                    },
                    next: cursor(0, 2),
                },
                Statement::Mutate {
                    mutation: Mutation::Word {
                        field: WordField::MotionPhase,
                        operation: WordOperation::Assign(WordOperand::Literal(0)),
                    },
                    next: cursor(0, 3),
                },
                Statement::Control(ControlCommand::Next {
                    immediate: false,
                    next: cursor(0, 4),
                }),
                Statement::Control(ControlCommand::End),
            ]])
            .unwrap();
            let expected = if initial == 0 {
                65536
            } else {
                usize::from(initial)
            };
            for visit in 1..=expected {
                let result = runtime
                    .enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 6)
                    .unwrap();
                assert_eq!(
                    result,
                    if visit == expected {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
            }
            assert_eq!(objects.get(owner).unwrap().base.target_speed, 0);
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase,
                0
            );
        }
    }

    #[test]
    fn authored_auxiliary_sprite_samples_new_input_and_preserves_pending_inversion() {
        use super::super::{authored_paths, path_motion};
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::AUXILIARY_GATED_SPRITE);
        objects.get_mut(owner).unwrap().base.velocity.x = 1;
        runtime.branch.invert_next = true;
        let catalog = authored_paths::catalog();
        for (visit, (size, color)) in [(1, 1), (3, 2), (5, 3), (0, 1), (0, 0)]
            .into_iter()
            .enumerate()
        {
            let mut inputs = PathWorld {
                audio: None,
                primary_player: None,
                selected: None,
                fixed_players: [None; 2],
                primary_motion: None,
                primary_control: None,
                spawn_defaults: None,
                countdown: None,
                // The initial four-count loop does not read this record.
                selected_auxiliary: (visit >= 3).then_some(AuxiliaryContinuationInput {
                    mode: if visit == 4 { 0x80 } else { 0 },
                    action_flags: 0x20,
                }),
                random: &mut random,
                animation_clock: 61,
            };
            let outcome = runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 16)
                .unwrap();
            assert_eq!(
                outcome,
                if visit < 4 {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            assert!(runtime.branch.invert_next);
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.extension.texture_scroll_x, size);
            assert_eq!(actor.extension.color_frame, color);
            assert_eq!(actor.extension.animation_frame, 61);
            assert!(actor.base.flags.collision_disabled);
            if outcome == ControlStep::Movement {
                assert!(!runtime
                    .begin_movement(
                        &mut objects,
                        owner,
                        path_motion::PlayerDisplacement::default()
                    )
                    .unwrap());
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
            }
            assert_eq!(
                objects.get(owner).unwrap().base.position.x,
                (visit + 1).min(4) as i16
            );
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }

    #[test]
    fn missing_auxiliary_observation_errors_at_branch_without_silent_fallthrough() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![
            Statement::SelectedAuxiliaryBranch {
                condition: SelectedAuxiliaryCondition::Continuation,
                taken: cursor(0, 2),
                next: cursor(0, 1),
            },
            Statement::Control(ControlCommand::WaitOne { next: cursor(0, 2) }),
            Statement::Control(ControlCommand::End),
        ]])
        .unwrap();
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 3),
            Err(ProgramError::MissingSelectedAuxiliary)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 0)));
        let mut inputs = world(&mut random);
        inputs.selected_auxiliary = Some(AuxiliaryContinuationInput {
            mode: 0,
            action_flags: 0,
        });
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 2),
            Ok(ControlStep::Movement)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 2)));
        assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
    }

    #[test]
    fn authored_primary_motion_path_exits_on_loop_limit_ground_or_new_contact() {
        use super::super::collision_pass::ExclusionGroups;
        use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
        use super::super::{authored_paths, path_motion, Vector3};
        // No movement is requested here: each yield remains an explicit
        // scheduler boundary. Independently exercise all three source exits.
        for (ground_visit, contact_visit, last_visit) in [
            (None, None, 9),
            (Some(0), None, 0),
            (Some(3), None, 3),
            (None, Some(2), 2),
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let mut primary =
                Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
            primary.base.velocity = Vector3 {
                x: 123,
                y: 456,
                z: -789,
            };
            let primary = objects.allocate(primary).unwrap();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::PRIMARY_MOTION_GROUND_LIMITED);
            actor.base.position.y = -100;
            actor.base.flags.casts_shadow = true;
            actor.base.contacts.exclusion_groups = ExclusionGroups::PATH_SPAWN;
            actor.base.contacts.first_strategy_visit = true;
            actor.base.contacts.suppress_attack_damage = true;
            actor.base.wait_timer = 37;
            let initial_random = random;
            let direction =
                path_motion::direction_velocity(actor.base.pitch, actor.base.yaw, 60, 1);
            let expected_velocity = Vector3 {
                x: direction.x.wrapping_add(123),
                y: direction.y,
                z: direction.z.wrapping_add(-789),
            };
            let primary_before = objects.get(primary).unwrap().clone();
            let catalog = authored_paths::catalog();
            for visit in 0..=last_visit {
                if ground_visit == Some(visit) {
                    objects.get_mut(owner).unwrap().base.position.y = 0;
                }
                let mut inputs = world(&mut random);
                // The inline action runs once. Later iterations require no
                // primary observations, and must not add its motion again.
                if visit == 0 {
                    inputs.primary_player = Some(primary);
                    inputs.primary_motion = Some(PrimaryMotionInput {
                        auxiliary_mode: 0x1F,
                        displacement: Vector3 {
                            x: -1,
                            y: -2,
                            z: -3,
                        },
                    });
                }
                let mut outcome = runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 32)
                    .unwrap();
                if contact_visit == Some(visit) {
                    assert_eq!(outcome, ControlStep::Movement);
                    objects
                        .get_mut(owner)
                        .unwrap()
                        .base
                        .contacts
                        .new_contact_latched = true;
                    assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                    assert!(matches!(
                        runtime
                            .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                            .unwrap(),
                        CallbackStep::Run(_)
                    ));
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 4),
                        Ok(ControlStep::ResumeCallbacks)
                    );
                    assert_eq!(
                        runtime
                            .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                            .unwrap(),
                        CallbackStep::Complete
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(
                        catalog.statement(actor.base.path.unwrap()).unwrap(),
                        Statement::Control(ControlCommand::End)
                    );
                    assert!(!actor.base.flags.remove_after_tick);
                    outcome = runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 2)
                        .unwrap();
                }
                assert_eq!(
                    outcome,
                    if visit == last_visit {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.velocity, expected_velocity);
                assert_eq!(
                    actor.base.position,
                    Vector3 {
                        x: 0,
                        y: if ground_visit == Some(visit) { 0 } else { -100 },
                        z: 0
                    }
                );
                assert_eq!(actor.base.hit_points, 120);
                assert_eq!(actor.base.attack_power, 2);
                assert_eq!(actor.base.target_speed, 10);
                assert_eq!(actor.base.speed, 60);
                assert!(!actor.base.flags.casts_shadow);
                assert!(actor.base.contacts.credits_hit_side);
                assert!(actor.base.contacts.mutually_non_damaging);
                assert!(actor.base.contacts.first_strategy_visit);
                assert!(actor.base.contacts.suppress_attack_damage);
                assert!(actor.base.contacts.suppress_hit_marker);
                assert_eq!(
                    actor.base.contacts.exclusion_groups,
                    ExclusionGroups::from_authored_class(0x88)
                );
                assert_eq!(actor.base.flags.remove_after_tick, visit == last_visit);
                assert_eq!(objects.get(primary).unwrap(), &primary_before);
                assert_eq!(random, initial_random);
            }
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }

    #[test]
    fn player_control_requires_live_primary_and_target_record_before_mutation() {
        use super::super::path_player_control::{
            primary_position, PlayerControlCommand, PlayerTargetControl, PrimaryControl,
        };
        for command in [
            PlayerControlCommand::Configure(-8),
            PlayerControlCommand::ConfigureDoubledLowByte(-8),
            PlayerControlCommand::ConfigureAlternateAxes(-8),
            PlayerControlCommand::LockForLinkedMode,
            PlayerControlCommand::FollowPrimaryPosition,
            PlayerControlCommand::RefreshOwnedOrigin,
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let before = objects.get(owner).unwrap().clone();
            let original_random = random;
            let catalog = PathCatalog::new(vec![vec![Statement::PlayerControl {
                command,
                next: cursor(0, 1),
            }]])
            .unwrap();
            let mut inputs = world(&mut random);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::MissingPrimaryPlayer)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            inputs.primary_player = Some(owner);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::MissingPrimaryControl)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            assert_eq!(inputs.random, &original_random);
            let mut control = PlayerTargetControl {
                owner: Some(owner),
                ..PlayerTargetControl::default()
            };
            let mut expected_control = control;
            let mut expected = before.clone();
            match command {
                PlayerControlCommand::Configure(range) => {
                    expected_control.configure(owner, before.base.position, range)
                }
                PlayerControlCommand::ConfigureDoubledLowByte(range) => {
                    expected_control.configure_doubled_low_byte(owner, before.base.position, range)
                }
                PlayerControlCommand::ConfigureAlternateAxes(range) => {
                    expected_control.configure_alternate_axes(owner, before.base.position, range)
                }
                PlayerControlCommand::LockForLinkedMode => {
                    expected_control.lock_for_linked_mode(true)
                }
                PlayerControlCommand::FollowPrimaryPosition => {
                    expected.base.position = primary_position(
                        before.base.position,
                        before.base.pitch,
                        before.base.yaw,
                        true,
                    )
                }
                PlayerControlCommand::RefreshOwnedOrigin => {
                    expected_control.refresh_owned_origin(owner, before.base.position)
                }
            }
            expected.base.path = Some(cursor(0, 1));
            inputs.primary_control = Some(PrimaryControl {
                target: &mut control,
                linked_mode: true,
            });
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert_eq!(
                *inputs.primary_control.as_ref().unwrap().target,
                expected_control
            );
            assert_eq!(inputs.random, &original_random);
        }
    }

    #[test]
    fn authored_primary_target_follower_runs_eight_live_updates_and_configures_only_once() {
        use super::super::path_player_control::{
            primary_position, PlayerTargetControl, PrimaryControl,
        };
        use super::super::path_sound::{AuthoredCue, CueListener, PathAudio};
        use super::super::{authored_paths, Angle, AudioState, SoundEvent, Vector3};
        for initially_linked in [false, true] {
            for locked in [false, true] {
                for owned in [false, true] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let primary = objects
                        .allocate(Object::new(
                            ObjectKind::Player,
                            ShapeId::EMPTY,
                            Behavior::PlayerFlight,
                        ))
                        .unwrap();
                    let original_random = random;
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(authored_paths::PRIMARY_TARGET_FOLLOWER);
                    actor.base.position = Vector3 { x: 1, y: 2, z: 3 };
                    actor.base.velocity = Vector3 { x: 9, y: -8, z: 7 };
                    actor.base.wait_timer = 57;
                    actor.base.pitch = Angle::from_units(21);
                    actor.base.yaw = Angle::from_units(22);
                    actor.base.roll = Angle::from_units(23);
                    actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
                    let stable_owner = actor.clone();
                    let mut control = PlayerTargetControl {
                        configuration_locked: locked,
                        offset_enabled: true,
                        owner: Some(if owned { owner } else { primary }),
                        origin: Vector3 {
                            x: -10,
                            y: -20,
                            z: -30,
                        },
                        range: -100,
                        positive_range: 200,
                        mode: 7,
                        ..PlayerTargetControl::default()
                    };
                    let mut expected_control = control;
                    expected_control.configure(owner, actor.base.position, -8);
                    expected_control.lock_for_linked_mode(initially_linked);
                    let mut audio = AudioState::default();
                    let catalog = authored_paths::catalog();
                    for visit in 0..8 {
                        let linked = if visit == 0 {
                            initially_linked
                        } else {
                            visit % 2 == 0
                        };
                        let player = objects.get_mut(primary).unwrap();
                        player.base.position = Vector3 {
                            x: i16::MAX - visit * 100,
                            y: i16::MIN + visit * 200,
                            z: visit * 300,
                        };
                        player.base.pitch = Angle::from_units((visit * 37) as u8);
                        player.base.yaw = Angle::from_units((visit * 51) as u8);
                        let expected_position = primary_position(
                            player.base.position,
                            player.base.pitch,
                            player.base.yaw,
                            linked,
                        );
                        let primary_before = player.clone();
                        expected_control.refresh_owned_origin(owner, expected_position);
                        let mut inputs = world(&mut random);
                        inputs.primary_player = Some(primary);
                        // A distinct selected actor must never replace primary.
                        inputs.selected = Some(owner);
                        inputs.primary_control = Some(PrimaryControl {
                            target: &mut control,
                            linked_mode: linked,
                        });
                        inputs.audio = Some(PathAudio {
                            events: &mut audio,
                            listeners: [CueListener::PrimaryPlayer, CueListener::Other],
                            markers: None,
                        });
                        runtime.branch.invert_next = true;
                        assert_eq!(
                            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 16),
                            Ok(if visit < 7 {
                                ControlStep::Movement
                            } else {
                                ControlStep::Ended
                            })
                        );
                        assert_eq!(
                            *inputs.primary_control.as_ref().unwrap().target,
                            expected_control
                        );
                        let expected_cues = if visit == 0 {
                            vec![SoundEvent::Authored(AuthoredCue::new(
                                50,
                                0,
                                PlayerTarget::Secondary,
                            ))]
                        } else {
                            vec![]
                        };
                        assert_eq!(
                            inputs
                                .audio
                                .as_mut()
                                .unwrap()
                                .events
                                .take_events()
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>(),
                            expected_cues
                        );
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.base.position, expected_position);
                        assert_eq!(actor.base.velocity, stable_owner.base.velocity);
                        assert_eq!(
                            (actor.base.pitch, actor.base.yaw, actor.base.roll),
                            (
                                stable_owner.base.pitch,
                                stable_owner.base.yaw,
                                stable_owner.base.roll
                            )
                        );
                        assert_eq!(actor.base.wait_timer, 57);
                        assert!(actor.base.flags.collision_disabled);
                        assert_eq!(actor.base.flags.remove_after_tick, visit == 7);
                        assert_eq!(objects.get(primary).unwrap(), &primary_before);
                        assert_eq!(inputs.random, &original_random);
                        assert!(runtime.branch.invert_next);
                    }
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn authored_alternate_exhaust_runs_complete_graph_with_two_movement_yields() {
        use super::super::{authored_paths, path_appearance, path_motion};
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::ALTERNATE_EXHAUST);
        objects.get_mut(owner).unwrap().base.velocity.x = 7;
        let catalog = authored_paths::catalog();
        assert_eq!(authored_paths::LOWERED_ROOT_COUNT, 12);
        assert_eq!(authored_paths::LOWERED_COMMAND_COUNT, 134);
        // Source DO 3 executes ADDCOL three times; NEXT only yields on its
        // first two decrements. The final pass reaches END without movement.
        for (invocation, color) in [1, 0, 1].into_iter().enumerate() {
            let outcome = runtime
                .enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8)
                .unwrap();
            assert_eq!(
                outcome,
                if invocation < 2 {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            if outcome == ControlStep::Movement {
                assert!(!runtime
                    .begin_movement(
                        &mut objects,
                        owner,
                        path_motion::PlayerDisplacement::default()
                    )
                    .unwrap());
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
            }
            let actor = objects.get_mut(owner).unwrap();
            let clock = 49 + invocation as u8;
            path_appearance::publish_animation(actor, clock);
            assert_eq!(actor.extension.color_frame, color);
            assert_eq!(actor.extension.animation_frame, clock);
            assert!(actor.base.flags.scaled_sprite);
            assert_eq!(actor.extension.depth_offset, 0);
            assert_eq!(actor.extension.texture_scroll_x, 0);
            assert_eq!(actor.base.position.x, 7 * (invocation + 1).min(2) as i16);
            assert_eq!(actor.base.flags.remove_after_tick, invocation == 2);
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }

    #[test]
    fn authored_color_cycle_sprite_waits_on_zero_then_cycles_before_retirement() {
        use super::super::{authored_paths, path_appearance, path_motion};
        let (mut runtime, mut objects, owner, mut random) = setup();
        {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::COLOR_CYCLE_SPRITE);
            actor.base.hit_points = 10;
            actor.base.hit_flags = 0x40;
            actor.base.contacts.new_contact_latched = true;
            actor.base.velocity.x = 7;
        }
        let catalog = authored_paths::catalog();
        // INITCOL 0 / WAITONE supplies the first visible color. DO 7 then
        // adds 1..7; the last NEXT completes and END retires without a yield.
        for color in 0..8u8 {
            let outcome = runtime
                .enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8)
                .unwrap();
            assert_eq!(
                outcome,
                if color < 7 {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            {
                let actor = objects.get_mut(owner).unwrap();
                path_appearance::publish_animation(actor, 133 + color);
                assert_eq!(actor.extension.color_frame, color);
                assert_eq!(actor.extension.animation_frame, 5 + color);
                assert!(actor.base.flags.collision_disabled);
                assert!(actor.base.flags.scaled_sprite);
                assert_eq!(actor.base.hit_points, 10);
                assert_eq!(actor.base.hit_flags, 0x40);
                assert_eq!(actor.extension.depth_offset, 0);
                assert_eq!(actor.extension.texture_scroll_x, 10);
                if color == 0 {
                    assert!(actor.base.contacts.new_contact_latched);
                }
            }
            if outcome == ControlStep::Movement {
                assert!(!runtime
                    .begin_movement(
                        &mut objects,
                        owner,
                        path_motion::PlayerDisplacement::default()
                    )
                    .unwrap());
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
            }
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.position.x, 7 * i16::from((color + 1).min(7)));
            assert_eq!(actor.base.flags.remove_after_tick, color == 7);
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }

    #[test]
    fn authored_particle_draws_six_shared_bytes_once_and_exits_at_phase_seven() {
        use super::super::{authored_paths, path_motion, Vector3};
        let (mut runtime, mut objects, owner, mut random) = setup();
        let initial = Vector3 {
            x: i16::MAX,
            y: i16::MIN,
            z: 42,
        };
        {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::RANDOMIZED_COLOR_PARTICLE);
            actor.base.position = initial;
            actor.extension.path_state.motion_phase = 0xA500;
        }
        let mut expected_random = random;
        let mut jitter = || {
            let high = expected_random.next_byte();
            let low = expected_random.next_byte();
            (u16::from_be_bytes([high, low]) & 15) as i16 - 7
        };
        let expected = Vector3 {
            x: initial.x.wrapping_add(jitter()),
            y: initial.y.wrapping_add(jitter()),
            z: initial.z.wrapping_add(jitter()),
        };
        let catalog = authored_paths::catalog();
        for phase in 1..=7 {
            let mut inputs = PathWorld {
                audio: None,
                primary_player: None,
                selected: None,
                fixed_players: [None; 2],
                primary_motion: None,
                primary_control: None,
                selected_auxiliary: None,
                countdown: None,
                spawn_defaults: None,
                random: &mut random,
                animation_clock: 93,
            };
            let outcome = runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 16)
                .unwrap();
            assert_eq!(
                outcome,
                if phase < 7 {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            assert_eq!(random, expected_random); // no per-iteration re-draw
            {
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.extension.path_state.motion_phase, 0xA500 | phase);
                assert_eq!(actor.extension.color_frame, phase as u8);
                assert_eq!(actor.extension.animation_frame, 93);
                assert_eq!(actor.base.target_speed, 0);
                assert_eq!(actor.base.acceleration, 5);
                assert_eq!(actor.base.position, expected);
                assert_eq!(actor.base.velocity.y, if phase < 7 { -1 } else { 0 });
            }
            if outcome == ControlStep::Movement {
                assert!(!runtime
                    .begin_movement(
                        &mut objects,
                        owner,
                        path_motion::PlayerDisplacement::default()
                    )
                    .unwrap());
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
                // Zero initial speed stays zero: the source's acceleration
                // service regenerates velocity before integrating the pose.
                assert_eq!(objects.get(owner).unwrap().base.position, expected);
            }
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
        // The next actor uses the SAME world stream, not a per-path seed.
        let mut second = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
        second.base.path = Some(authored_paths::RANDOMIZED_COLOR_PARTICLE);
        let second = objects.allocate(second).unwrap();
        for _ in 0..6 {
            expected_random.next_byte();
        }
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, second, &mut world(&mut random), 16),
            Ok(ControlStep::Movement)
        );
        assert_eq!(random, expected_random);
    }

    #[test]
    fn authored_local_jitter_returns_from_subroutine_then_runs_inverted_loop() {
        use super::super::{authored_paths, path_motion, Vector3};
        let (mut runtime, mut objects, owner, mut random) = setup();
        let initial = Vector3 {
            x: 100,
            y: -200,
            z: i16::MAX,
        };
        let velocity = Vector3 { x: 1, y: -2, z: 3 };
        {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::LOCAL_JITTER_SPRITE);
            actor.extension.relative_position = initial;
            actor.extension.path_state.motion.relative_coordinates = true;
            actor.extension.path_state.motion_phase = 0x5500;
            actor.base.velocity = velocity;
            actor.base.wait_timer = 97;
        }
        let mut expected_random = random;
        let mut jitter = || {
            let high = expected_random.next_byte();
            let low = expected_random.next_byte();
            (u16::from_be_bytes([high, low]) & 31) as i16 - 15
        };
        let mut expected = Vector3 {
            x: initial.x.wrapping_add(jitter()),
            y: initial.y.wrapping_add(jitter()),
            z: initial.z.wrapping_add(jitter()),
        };
        let catalog = authored_paths::catalog();
        for phase in 1..=3 {
            let outcome = runtime
                .enter_program(
                    &catalog,
                    &mut objects,
                    owner,
                    &mut PathWorld {
                        audio: None,
                        primary_player: None,
                        selected: None,
                        fixed_players: [None; 2],
                        primary_motion: None,
                        primary_control: None,
                        selected_auxiliary: None,
                        countdown: None,
                        spawn_defaults: None,
                        random: &mut random,
                        animation_clock: 29,
                    },
                    16,
                )
                .unwrap();
            assert_eq!(
                outcome,
                if phase < 3 {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            assert_eq!(random, expected_random);
            assert!(!runtime.branch.invert_next);
            {
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.extension.relative_position, expected);
                assert_eq!(actor.base.position, Vector3::default());
                assert_eq!(actor.extension.path_state.motion_phase, 0x5500 | phase);
                assert_eq!(actor.extension.color_frame, (phase % 3) as u8);
                assert_eq!(actor.extension.animation_frame, 29);
                assert_eq!(actor.extension.texture_scroll_x, 252);
                assert_eq!(actor.base.wait_timer, 97);
            }
            if outcome == ControlStep::Movement {
                assert!(!runtime
                    .begin_movement(
                        &mut objects,
                        owner,
                        path_motion::PlayerDisplacement::default()
                    )
                    .unwrap());
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
                expected.x = expected.x.wrapping_add(velocity.x);
                expected.y = expected.y.wrapping_add(velocity.y);
                expected.z = expected.z.wrapping_add(velocity.z);
            }
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }

    #[test]
    fn immediate_loop_samples_mutations_and_preserves_wait_boundary() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![
            health(ByteOperation::Assign(ByteOperand::Literal(3)), cursor(0, 1)),
            health(ByteOperation::Decrement, cursor(0, 2)),
            Statement::Compare {
                condition: ActorCondition::NonzeroByte(ByteOperand::Actor(ByteField::Health)),
                taken: cursor(0, 1),
                next: cursor(0, 3),
            },
            Statement::Control(ControlCommand::WaitOne { next: cursor(0, 4) }),
            Statement::Control(ControlCommand::End),
        ]])
        .unwrap();
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8),
            Ok(ControlStep::Movement)
        );
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.hit_points, 0);
        assert_eq!(actor.base.path, Some(cursor(0, 4)));
        assert!(!actor.base.flags.remove_after_tick);
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Ok(ControlStep::Ended)
        );
    }

    #[test]
    fn called_path_waits_then_resumes_saved_caller() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![
            vec![
                Statement::Control(ControlCommand::Call {
                    target: cursor(1, 0),
                    next: cursor(0, 1),
                }),
                Statement::Control(ControlCommand::End),
            ],
            vec![
                Statement::Wait {
                    duration: ByteOperand::Actor(ByteField::Health),
                    next: cursor(1, 1),
                },
                Statement::Control(ControlCommand::Return),
            ],
        ])
        .unwrap();
        objects.get_mut(owner).unwrap().base.hit_points = 2;
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 2),
            Ok(ControlStep::Movement)
        );
        assert_eq!(objects.get(owner).unwrap().base.wait_timer, 1);
        // Change the live operand between invocations: retained/predecoded
        // duration=2 would yield again instead of completing this call.
        objects.get_mut(owner).unwrap().base.hit_points = 1;
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 3),
            Ok(ControlStep::Ended)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 1)));
        assert_eq!(objects.get(owner).unwrap().base.wait_timer, 0);
    }

    #[test]
    fn callback_return_yields_to_batch_not_interrupted_main_program() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![
            vec![Statement::Control(ControlCommand::End)],
            vec![
                health(ByteOperation::Assign(ByteOperand::Literal(7)), cursor(1, 1)),
                Statement::Control(ControlCommand::Return),
            ],
        ])
        .unwrap();
        runtime
            .add_trigger(
                &mut objects,
                owner,
                Trigger {
                    path: cursor(1, 0),
                    kind: TriggerKind::Always,
                    timer: 0,
                },
            )
            .unwrap();
        assert!(runtime.begin_callbacks(&objects, owner).unwrap());
        assert_eq!(
            runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
            Ok(CallbackStep::Run(cursor(1, 0)))
        );
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 2),
            Ok(ControlStep::ResumeCallbacks)
        );
        assert_eq!(objects.get(owner).unwrap().base.hit_points, 7);
        assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
        assert_eq!(
            runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
            Ok(CallbackStep::Complete)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 0)));
    }

    #[test]
    fn immediate_resume_does_not_reselect_player() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![Statement::Control(ControlCommand::WaitOne {
            next: cursor(0, 0),
        })]])
        .unwrap();
        runtime.enter(&objects, owner).unwrap();
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = PlayerTarget::Secondary;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Ok(ControlStep::Movement)
        );
        assert_eq!(runtime.selected_player(), PlayerTarget::Primary);
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Ok(ControlStep::Movement)
        );
        assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
    }

    #[test]
    fn missing_statements_and_runaway_paths_never_become_successful_ticks() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![
            health(ByteOperation::Increment, cursor(0, 1)),
            Statement::Control(ControlCommand::Jump {
                target: cursor(0, 0),
            }),
        ]])
        .unwrap();
        objects.get_mut(owner).unwrap().base.hit_points = 0;
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 3),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0, 1),
                executed: 3
            })
        );
        assert_eq!(objects.get(owner).unwrap().base.hit_points, 2);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0, 1),
                executed: 0
            })
        );
        objects.get_mut(owner).unwrap().base.path = Some(cursor(1, 0));
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 5),
            Err(ProgramError::MissingStatement(cursor(1, 0)))
        );
    }
}
