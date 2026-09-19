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
    pub selected: Option<ObjectId>,
    pub random: &'a mut RandomState,
    pub animation_clock: u8,
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
    use super::super::path_fields::{ByteField, ByteOperation};
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
            selected: None,
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
    fn authored_alternate_exhaust_runs_complete_graph_with_two_movement_yields() {
        use super::super::{authored_paths, path_appearance, path_motion};
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::ALTERNATE_EXHAUST);
        objects.get_mut(owner).unwrap().base.velocity.x = 7;
        let catalog = authored_paths::catalog();
        assert_eq!(authored_paths::LOWERED_ROOT_COUNT, 4);
        assert_eq!(authored_paths::LOWERED_COMMAND_COUNT, 37);
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
                selected: None,
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
                        selected: None,
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
