//! One source path-strategy invocation, through its actual movement footer.
//!
//! Common entry ($7F:7E53), immediate dispatch, ordinary/death-tail movement
//! ($7F:9DDE/9E70), mutable callback traversal and the attachment/carry footer
//! are distinct phases. The strategy scheduler, not this driver, owns clocks,
//! pause/death/hit routing, actor traversal and eventual retirement.

use super::path_commands::ControlStep;
use super::path_control::PlayerTarget;
use super::path_motion::PlayerDisplacement;
use super::path_program::{PathCatalog, PathWorld, ProgramError};
use super::path_runtime::{CallbackStep, PathRuntime, PathRuntimeError, TriggerWorldInputs};
use super::platform_carry::CarriedPlayer;
use super::{ObjectId, ObjectStore};

/// Live world ownership. No method has an assumed-success/default service.
/// Returned path borrows last for one statement only: callbacks and immediate
/// NEXT can select a different player before the next statement executes.
pub trait InvocationWorld {
    type Error;

    /// Resolve actor-linked and firing inputs using the actual current
    /// program actor, not the original invoker or the selected player.
    fn path_world(
        &mut self,
        objects: &ObjectStore,
        actor: ObjectId,
        selected: PlayerTarget,
    ) -> Result<PathWorld<'_>, Self::Error>;

    fn displacement(&mut self, selected: PlayerTarget) -> Result<PlayerDisplacement, Self::Error>;

    /// Called anew for every callback candidate, after all earlier callbacks.
    /// Derive projections from these live objects, not an epoch-entry snapshot.
    fn trigger_inputs(
        &mut self,
        objects: &ObjectStore,
        owner: ObjectId,
        selected: PlayerTarget,
    ) -> Result<TriggerWorldInputs, Self::Error>;

    /// The authoritative auxiliary records, not copies of player transforms.
    fn carried_players(&mut self) -> Result<&mut [Option<CarriedPlayer>; 2], Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationEntry {
    /// Initializer (if pending), then common path entry.
    Program,
    /// PATHHOLD's installed movement strategy does not re-enter the path or
    /// refresh selection from this actor. Preserve the shared selected slot.
    Movement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Fault {
    Runtime(PathRuntimeError),
    UnexpectedExit { actor: ObjectId, step: ControlStep },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    Entry(ObjectId),
    Program {
        actor: ObjectId,
        callback_owner: Option<ObjectId>,
    },
    Movement {
        actor: ObjectId,
        tail_only: bool,
    },
    Callbacks(ObjectId),
    Finish(ObjectId),
    Faulted(Fault),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvocationError<E> {
    NotActive,
    /// A diagnostic boundary, never a new game/movement tick. Resume the same
    /// invocation; phases already completed must not be executed again.
    BudgetExceeded {
        completed_steps: usize,
    },
    Program(ProgramError),
    Runtime(PathRuntimeError),
    World(E),
    UnexpectedExit {
        actor: ObjectId,
        step: ControlStep,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvocationAlreadyActive;

/// Shared runtime plus at most one in-flight strategy invocation. A missing
/// world input or diagnostic budget retains its exact phase and actual actor,
/// including temporary actor borrowing. Invalid runtime-state errors latch:
/// they cannot be retried as a fresh movement after a partially changed state.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PathInvocation {
    pub runtime: PathRuntime,
    phase: Option<Phase>,
}

impl PathInvocation {
    pub fn is_active(&self) -> bool {
        self.phase.is_some()
    }

    pub fn begin(
        &mut self,
        actor: ObjectId,
        entry: InvocationEntry,
    ) -> Result<(), InvocationAlreadyActive> {
        if self.is_active() {
            return Err(InvocationAlreadyActive);
        }
        self.phase = Some(match entry {
            InvocationEntry::Program => Phase::Entry(actor),
            InvocationEntry::Movement => Phase::Movement {
                actor,
                tail_only: false,
            },
        });
        Ok(())
    }

    /// Complete one invocation, not one presentation frame. Each immediate
    /// statement and each phase/callback visit consumes one diagnostic step.
    /// The return is the actual actor at the common exit, which need not be the
    /// original caller. END merely marks it; this driver never recycles slots.
    pub fn resume<W: InvocationWorld>(
        &mut self,
        catalog: &PathCatalog,
        objects: &mut ObjectStore,
        world: &mut W,
        budget: usize,
    ) -> Result<ObjectId, InvocationError<W::Error>> {
        if self.phase.is_none() {
            return Err(InvocationError::NotActive);
        }
        for _ in 0..budget {
            let phase = self.phase.clone().expect("active invocation");
            match phase {
                Phase::Entry(actor) => {
                    let result = self
                        .runtime
                        .initialize_path_strategy(objects, actor)
                        .and_then(|()| self.runtime.enter(objects, actor).map(|_| ()));
                    self.check_runtime(result)?;
                    self.phase = Some(Phase::Program {
                        actor,
                        callback_owner: None,
                    });
                }
                Phase::Program {
                    actor,
                    callback_owner,
                } => {
                    let mut inputs = world
                        .path_world(objects, actor, self.runtime.selected_player())
                        .map_err(InvocationError::World)?;
                    let result = self
                        .runtime
                        .step_program(catalog, objects, actor, &mut inputs);
                    let current = self.runtime.program_actor().unwrap_or(actor);
                    self.phase = Some(Phase::Program {
                        actor: current,
                        callback_owner,
                    });
                    let exit = result.map_err(InvocationError::Program)?;
                    self.phase = Some(match (callback_owner, exit.step) {
                        (_, ControlStep::Continue) => Phase::Program {
                            actor: exit.actor,
                            callback_owner,
                        },
                        (Some(owner), ControlStep::ResumeCallbacks) => Phase::Callbacks(owner),
                        (None, ControlStep::Movement) => Phase::Movement {
                            actor: exit.actor,
                            tail_only: false,
                        },
                        (None, ControlStep::MovementTail) => Phase::Movement {
                            actor: exit.actor,
                            tail_only: true,
                        },
                        (None, ControlStep::Ended) => {
                            self.phase = None;
                            return Ok(exit.actor);
                        }
                        _ => {
                            let fault = Fault::UnexpectedExit {
                                actor: exit.actor,
                                step: exit.step,
                            };
                            self.phase = Some(Phase::Faulted(fault));
                            return Err(InvocationError::UnexpectedExit {
                                actor: exit.actor,
                                step: exit.step,
                            });
                        }
                    });
                }
                Phase::Movement { actor, tail_only } => {
                    let result = if tail_only {
                        self.runtime.begin_movement_tail(objects, actor)
                    } else {
                        let follows = objects
                            .get(actor)
                            .ok_or(InvocationError::Runtime(PathRuntimeError::MissingActor(
                                actor,
                            )))?
                            .extension
                            .path_state
                            .motion
                            .follow_player_displacement;
                        let displacement = if follows {
                            world
                                .displacement(self.runtime.selected_player())
                                .map_err(InvocationError::World)?
                        } else {
                            // Source gate bypasses this service entirely; these
                            // values are not read by before_callbacks.
                            PlayerDisplacement::default()
                        };
                        self.runtime.begin_movement(objects, actor, displacement)
                    };
                    let callbacks = self.check_runtime(result)?;
                    self.phase = Some(if callbacks {
                        Phase::Callbacks(actor)
                    } else {
                        Phase::Finish(actor)
                    });
                }
                Phase::Callbacks(actor) => {
                    let inputs = world
                        .trigger_inputs(objects, actor, self.runtime.selected_player())
                        .map_err(InvocationError::World)?;
                    let result = self.runtime.step_callbacks(objects, actor, inputs);
                    self.phase = Some(match self.check_runtime(result)? {
                        CallbackStep::Complete => Phase::Finish(actor),
                        CallbackStep::Skipped | CallbackStep::Expired => Phase::Callbacks(actor),
                        CallbackStep::Run(_) => Phase::Program {
                            actor,
                            callback_owner: Some(actor),
                        },
                    });
                }
                Phase::Finish(actor) => {
                    let carries = objects
                        .get(actor)
                        .ok_or(InvocationError::Runtime(PathRuntimeError::MissingActor(
                            actor,
                        )))?
                        .extension
                        .path_state
                        .motion
                        .carry_selected_player;
                    let result = if carries {
                        let players = world.carried_players().map_err(InvocationError::World)?;
                        self.runtime.finish_movement(objects, players)
                    } else {
                        // The carry gate is disabled; no player record is read.
                        self.runtime.finish_movement(objects, &mut [None; 2])
                    };
                    self.check_runtime(result)?;
                    self.phase = None;
                    return Ok(actor);
                }
                Phase::Faulted(fault) => {
                    return Err(match fault {
                        Fault::Runtime(error) => InvocationError::Runtime(error),
                        Fault::UnexpectedExit { actor, step } => {
                            InvocationError::UnexpectedExit { actor, step }
                        }
                    })
                }
            }
        }
        Err(InvocationError::BudgetExceeded {
            completed_steps: budget,
        })
    }

    fn check_runtime<T, E>(
        &mut self,
        result: Result<T, PathRuntimeError>,
    ) -> Result<T, InvocationError<E>> {
        result.map_err(|error| {
            self.phase = Some(Phase::Faulted(Fault::Runtime(error.clone())));
            InvocationError::Runtime(error)
        })
    }
}
