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
    RunWhenPaused {
        enabled: bool,
        next: PathCursor,
    },
    /// Named source inline action: latch phase low byte to one if the primary
    /// player's view-side filter is enabled; otherwise leave it unchanged.
    LatchPrimaryViewFilter {
        next: PathCursor,
    },
    Sound {
        cue: super::path_sound::AuthoredCue,
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
    Mutate {
        mutation: Mutation,
        next: PathCursor,
    },
    Wait {
        duration: ByteOperand,
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
    MissingAudio,
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
                Statement::Mutate { mutation, next } => {
                    self.execute_mutation(objects, owner, mutation, next)
                }
                Statement::Wait { duration, next } => {
                    let duration = duration.read(actor);
                    self.execute_control(objects, owner, ControlCommand::Wait { duration, next })
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
                spawn_defaults: None,
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
    fn authored_alternate_exhaust_runs_complete_graph_with_two_movement_yields() {
        use super::super::{authored_paths, path_appearance, path_motion};
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::ALTERNATE_EXHAUST);
        objects.get_mut(owner).unwrap().base.velocity.x = 7;
        let catalog = authored_paths::catalog();
        assert_eq!(authored_paths::LOWERED_ROOT_COUNT, 9);
        assert_eq!(authored_paths::LOWERED_COMMAND_COUNT, 97);
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
                selected_auxiliary: None,
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
                        selected_auxiliary: None,
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
