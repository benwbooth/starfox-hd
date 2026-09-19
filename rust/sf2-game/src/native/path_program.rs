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
use super::{Object, ObjectId, ObjectStore, PathCursor};

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
        selected: Option<ObjectId>,
        budget: usize,
    ) -> Result<ControlStep, ProgramError> {
        self.check_execution_owner(owner)?;
        self.enter(objects, owner)?;
        self.resume_program(catalog, objects, owner, selected, budget)
    }

    /// Run only immediate statements. Budget exhaustion retains the next live
    /// cursor and returns an error; it must never manufacture a movement tick.
    pub fn resume_program(
        &mut self,
        catalog: &PathCatalog,
        objects: &mut ObjectStore,
        owner: ObjectId,
        selected: Option<ObjectId>,
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
                } => self.execute_spatial_branch(objects, owner, selected, condition, taken, next),
            }?;
            if outcome != ControlStep::Continue {
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

    fn setup() -> (PathRuntime, ObjectStore, ObjectId) {
        let mut objects = ObjectStore::new();
        let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
        actor.base.path = Some(cursor(0, 0));
        let owner = objects.allocate(actor).unwrap();
        (PathRuntime::default(), objects, owner)
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
    fn immediate_loop_samples_mutations_and_preserves_wait_boundary() {
        let (mut runtime, mut objects, owner) = setup();
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
            runtime.enter_program(&catalog, &mut objects, owner, None, 8),
            Ok(ControlStep::Movement)
        );
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.hit_points, 0);
        assert_eq!(actor.base.path, Some(cursor(0, 4)));
        assert!(!actor.base.flags.remove_after_tick);
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, None, 1),
            Ok(ControlStep::Ended)
        );
    }

    #[test]
    fn called_path_waits_then_resumes_saved_caller() {
        let (mut runtime, mut objects, owner) = setup();
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
            runtime.enter_program(&catalog, &mut objects, owner, None, 2),
            Ok(ControlStep::Movement)
        );
        assert_eq!(objects.get(owner).unwrap().base.wait_timer, 1);
        // Change the live operand between invocations: retained/predecoded
        // duration=2 would yield again instead of completing this call.
        objects.get_mut(owner).unwrap().base.hit_points = 1;
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, None, 3),
            Ok(ControlStep::Ended)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 1)));
        assert_eq!(objects.get(owner).unwrap().base.wait_timer, 0);
    }

    #[test]
    fn callback_return_yields_to_batch_not_interrupted_main_program() {
        let (mut runtime, mut objects, owner) = setup();
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
            runtime.resume_program(&catalog, &mut objects, owner, None, 2),
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
        let (mut runtime, mut objects, owner) = setup();
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
            runtime.resume_program(&catalog, &mut objects, owner, None, 1),
            Ok(ControlStep::Movement)
        );
        assert_eq!(runtime.selected_player(), PlayerTarget::Primary);
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, None, 1),
            Ok(ControlStep::Movement)
        );
        assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
    }

    #[test]
    fn missing_statements_and_runaway_paths_never_become_successful_ticks() {
        let (mut runtime, mut objects, owner) = setup();
        let catalog = PathCatalog::new(vec![vec![
            health(ByteOperation::Increment, cursor(0, 1)),
            Statement::Control(ControlCommand::Jump {
                target: cursor(0, 0),
            }),
        ]])
        .unwrap();
        objects.get_mut(owner).unwrap().base.hit_points = 0;
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, None, 3),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0, 1),
                executed: 3
            })
        );
        assert_eq!(objects.get(owner).unwrap().base.hit_points, 2);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, None, 0),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0, 1),
                executed: 0
            })
        );
        objects.get_mut(owner).unwrap().base.path = Some(cursor(1, 0));
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, None, 5),
            Err(ProgramError::MissingStatement(cursor(1, 0)))
        );
    }
}
