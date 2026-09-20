//! Decoded source path control statements. Destinations are semantic cursors;
//! count operands are already values from typed actor/world fields. There is
//! no encoded operand reader, source address lookup or instruction emulator.

use super::path_calls::PathReturn;
use super::path_fields::{ByteField, WordField};
use super::path_runtime::{PathRuntime, PathRuntimeError};
use super::path_triggers::Trigger;
use super::program_state::LoopRepeat;
use super::{ObjectId, ObjectStore, PathCursor};

/// A source statement with all continuation edges explicit. This enum covers
/// control statements only; it does not stand in for unported world services.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCommand {
    End,
    Hold,
    /// `$7F:BC80`: finish this movement invocation, then skip future actor
    /// strategy visits. Unlike PATHHOLD, retain the assigned behavior.
    SuspendAndMove,
    Wait {
        duration: u8,
        next: PathCursor,
    },
    WaitOne {
        next: PathCursor,
    },
    Repeat {
        count: u8,
        target: PathCursor,
        next: PathCursor,
    },
    Goto {
        target: PathCursor,
    },
    Jump {
        target: PathCursor,
    },
    Call {
        target: PathCursor,
        next: PathCursor,
    },
    Return,
    BeginLoop {
        iterations: u16,
        next: PathCursor,
    },
    Next {
        immediate: bool,
        next: PathCursor,
    },
    Break {
        target: PathCursor,
    },
    PopStackPair {
        next: PathCursor,
    },
    Register {
        trigger: Trigger,
        next: PathCursor,
    },
    Cancel {
        path: PathCursor,
        next: PathCursor,
    },
    Clear {
        next: PathCursor,
    },
    ForceAfterCallbacks {
        target: PathCursor,
        next: PathCursor,
    },
    CallAfterCallbacks {
        target: PathCursor,
        next: PathCursor,
    },
}

/// The caller must service movement and callback continuations separately;
/// neither outcome means to tick every actor or advance presentation time.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlStep {
    Continue,
    Movement,
    ResumeCallbacks,
    /// END runs only exit-latch cleanup; retirement is the scheduler's job.
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackValueCommand {
    SaveByte(ByteField),
    SaveWord(WordField),
    RestoreByte(ByteField),
    RestoreWord(WordField),
}

impl PathRuntime {
    pub fn execute_stack_value(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        command: StackValueCommand,
        next: PathCursor,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = objects
            .get_mut(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        if actor.base.path.is_none() {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        match command {
            StackValueCommand::SaveByte(field) => {
                let value = field.read(actor);
                actor
                    .extension
                    .path_state
                    .stack
                    .save_byte(&mut self.resources, owner, value)
                    .map_err(PathRuntimeError::Stack)?;
            }
            StackValueCommand::SaveWord(field) => {
                let value = field.read(actor);
                actor
                    .extension
                    .path_state
                    .stack
                    .save_word(&mut self.resources, owner, value)
                    .map_err(PathRuntimeError::Stack)?;
            }
            StackValueCommand::RestoreByte(field) => {
                let value = actor
                    .extension
                    .path_state
                    .stack
                    .restore_byte(&mut self.resources)
                    .map_err(PathRuntimeError::Stack)?;
                field.write(actor, value);
            }
            StackValueCommand::RestoreWord(field) => {
                let value = actor
                    .extension
                    .path_state
                    .stack
                    .restore_word(&mut self.resources)
                    .map_err(PathRuntimeError::Stack)?;
                field.write(actor, value);
            }
        }
        actor.base.path = Some(next);
        Ok(ControlStep::Continue)
    }
}

/// Source motion configuration statements; every one continues immediately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionCommand {
    SetSpeed(u8),
    AccelerateTo { target: u8, amount: u8 },
    FollowPlayerDisplacement(bool),
    GenerateVelocityEachStep(bool),
    BankTurn(bool),
    QuadrupleVelocity(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchCommand {
    HitFlags {
        mask: u8,
        taken: PathCursor,
        next: PathCursor,
    },
    HitEvent {
        taken: PathCursor,
        next: PathCursor,
    },
    InvertNext {
        next: PathCursor,
    },
    Test {
        predicate: super::path_conditions::Predicate,
        taken: PathCursor,
        next: PathCursor,
    },
}

impl PathRuntime {
    pub fn execute_random(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        random: &mut super::RandomState,
        mutation: super::path_random::RandomMutation,
        next: PathCursor,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = objects
            .get_mut(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        if actor.base.path.is_none() {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        mutation.apply(actor, random);
        actor.base.path = Some(next);
        Ok(ControlStep::Continue)
    }

    /// Source `$7F:9A9F` sets the flag tested by the collision queue at
    /// `$7F:32CE`; no contact-latch clearing or retirement is implied.
    pub fn execute_disable_collision(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        next: PathCursor,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = objects
            .get_mut(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        if actor.base.path.is_none() {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        actor.base.flags.collision_disabled = true;
        actor.base.path = Some(next);
        Ok(ControlStep::Continue)
    }

    pub fn execute_animation(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        command: super::path_appearance::AnimationCommand,
        next: PathCursor,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = objects
            .get_mut(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        if actor.base.path.is_none() {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        actor.extension.path_state.animation.apply(command);
        actor.base.path = Some(next);
        Ok(ControlStep::Continue)
    }

    pub fn execute_sprite(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        color: u8,
        size: u8,
        next: PathCursor,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = objects
            .get_mut(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        if actor.base.path.is_none() {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        super::path_appearance::set_sprite(actor, color, size);
        actor.base.path = Some(next);
        Ok(ControlStep::Continue)
    }

    pub fn execute_mutation(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        mutation: super::path_fields::Mutation,
        next: PathCursor,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = objects
            .get_mut(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        if actor.base.path.is_none() {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        mutation.apply(actor);
        actor.base.path = Some(next);
        Ok(ControlStep::Continue)
    }

    pub fn execute_spatial_branch(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        selected: Option<ObjectId>,
        condition: super::path_conditions::SpatialCondition,
        taken: PathCursor,
        next: PathCursor,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let predicate = super::path_conditions::sample_spatial(objects, owner, selected, condition)
            .map_err(PathRuntimeError::Conditions)?;
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

    pub fn execute_branch(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        command: BranchCommand,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = objects
            .get_mut(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        if actor.base.path.is_none() {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        let next = match command {
            BranchCommand::HitFlags { mask, taken, next } => {
                // `$7F:9A79`: the operand is a literal mask, not a bit index.
                // Only a successful test clears the masked hit flags.
                if actor.base.hit_flags & mask != 0 {
                    actor.base.hit_flags &= !mask;
                    taken
                } else {
                    next
                }
            }
            BranchCommand::HitEvent { taken, next } => {
                // `$7F:9507`: shares the event latch with hit callbacks.
                if std::mem::take(&mut actor.extension.path_state.conditions.hit_event_pending) {
                    taken
                } else {
                    next
                }
            }
            BranchCommand::InvertNext { next } => {
                self.branch.invert_next_condition();
                next
            }
            BranchCommand::Test {
                predicate,
                taken,
                next,
            } => {
                if self.branch.test(predicate) {
                    taken
                } else {
                    next
                }
            }
        };
        actor.base.path = Some(next);
        Ok(ControlStep::Continue)
    }

    pub fn execute_yaw_orbit(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        target: super::path_steering::YawOrbitTarget,
        angle: super::Angle,
        next: PathCursor,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        if objects
            .get(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?
            .base
            .path
            .is_none()
        {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        super::path_steering::orbit_yaw(objects, owner, target, angle)
            .map_err(PathRuntimeError::Steering)?;
        objects
            .get_mut(owner)
            .expect("validated yaw orbit actor")
            .base
            .path = Some(next);
        Ok(ControlStep::Continue)
    }

    pub fn execute_radius(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        command: super::path_steering::RadiusCommand,
        selected: Option<ObjectId>,
        next: PathCursor,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        if objects
            .get(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?
            .base
            .path
            .is_none()
        {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        super::path_steering::contract_radius(objects, owner, command, selected)
            .map_err(PathRuntimeError::Steering)?;
        objects
            .get_mut(owner)
            .expect("validated radial movement actor")
            .base
            .path = Some(next);
        Ok(ControlStep::Continue)
    }

    pub fn execute_facing(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        command: super::path_steering::FacingCommand,
        targets: super::path_steering::FacingTargets,
        next: PathCursor,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        if objects
            .get(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?
            .base
            .path
            .is_none()
        {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        super::path_steering::face(objects, owner, command, targets, &mut self.steering)
            .map_err(PathRuntimeError::Steering)?;
        objects
            .get_mut(owner)
            .expect("validated facing actor")
            .base
            .path = Some(next);
        Ok(ControlStep::Continue)
    }

    pub fn execute_facing_offset(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        selected: Option<ObjectId>,
        offset: super::path_steering::AimOffset,
        next: PathCursor,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = objects
            .get(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        if actor.base.path.is_none() {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        super::path_steering::face_selected_offset(
            objects,
            owner,
            selected,
            offset,
            &mut self.steering,
        )
        .map_err(PathRuntimeError::Steering)?;
        objects
            .get_mut(owner)
            .expect("validated offset-facing actor")
            .base
            .path = Some(next);
        Ok(ControlStep::Continue)
    }

    pub fn execute_motion(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        command: MotionCommand,
        next: PathCursor,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = objects
            .get_mut(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        if actor.base.path.is_none() {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        match command {
            MotionCommand::SetSpeed(speed) => super::path_motion::set_speed(actor, owner, speed),
            MotionCommand::AccelerateTo { target, amount } => {
                actor.base.target_speed = target;
                actor.base.acceleration = amount;
            }
            MotionCommand::FollowPlayerDisplacement(enabled) => {
                actor.extension.path_state.motion.follow_player_displacement = enabled
            }
            MotionCommand::GenerateVelocityEachStep(enabled) => {
                actor
                    .extension
                    .path_state
                    .motion
                    .generate_velocity_each_step = enabled
            }
            MotionCommand::BankTurn(enabled) => {
                actor.extension.path_state.motion.bank_turn = enabled
            }
            MotionCommand::QuadrupleVelocity(enabled) => {
                actor.extension.path_state.motion.quadruple_velocity = enabled
            }
        }
        actor.base.path = Some(next);
        Ok(ControlStep::Continue)
    }

    pub fn execute_control(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        command: ControlCommand,
    ) -> Result<ControlStep, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = objects
            .get_mut(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        if actor.base.path.is_none() {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        let next = match command {
            // $7F:84FB compares before incrementing. A zero wait still needs
            // a full wrap when its retained elapsed byte starts nonzero.
            ControlCommand::Wait { duration, next } => {
                if actor.base.wait_timer != duration {
                    actor.base.wait_timer = actor.base.wait_timer.wrapping_add(1);
                    return Ok(ControlStep::Movement);
                }
                actor.base.wait_timer = 0;
                next
            }
            ControlCommand::End => {
                self.validate_terminal_command()?;
                actor.base.flags.remove_after_tick = true;
                super::path_motion::clear_exit_latches(actor);
                return Ok(ControlStep::Ended);
            }
            ControlCommand::Hold => {
                self.validate_terminal_command()?;
                actor.extension.path_state.hold_latched = true;
                actor.base.behavior = super::Behavior::PathMovement;
                return Ok(ControlStep::Movement);
            }
            ControlCommand::SuspendAndMove => {
                self.validate_terminal_command()?;
                actor.base.flags.strategy_suspended = true;
                return Ok(ControlStep::Movement);
            }
            ControlCommand::WaitOne { next } => {
                actor.base.path = Some(next);
                return Ok(ControlStep::Movement);
            }
            // $7F:860D: unlike DO/NEXT this counter increments toward a
            // byte operand and is not stacked, so nesting shares the counter.
            ControlCommand::Repeat {
                count,
                target,
                next,
            } => {
                let counter = &mut actor.extension.path_state.repeat_counter;
                if *counter != count {
                    *counter = counter.wrapping_add(1);
                    actor.base.path = Some(target);
                    return Ok(ControlStep::Movement);
                }
                *counter = 0;
                next
            }
            ControlCommand::Goto { target } => {
                actor.base.path = Some(target);
                return Ok(ControlStep::Movement);
            }
            ControlCommand::Jump { target } => target,
            ControlCommand::Call { target, next } => {
                self.call(objects, owner, target, next)?;
                return Ok(ControlStep::Continue);
            }
            ControlCommand::Return => {
                return Ok(match self.return_from(objects, owner)? {
                    PathReturn::Resume(_) => ControlStep::Continue,
                    PathReturn::CallbackComplete => ControlStep::ResumeCallbacks,
                });
            }
            ControlCommand::BeginLoop { iterations, next } => {
                actor
                    .extension
                    .path_state
                    .stack
                    .begin(&mut self.resources, owner, next, iterations)
                    .map_err(PathRuntimeError::Stack)?;
                next
            }
            ControlCommand::Next { immediate, next } => {
                match actor
                    .extension
                    .path_state
                    .stack
                    .next(&mut self.resources)
                    .map_err(PathRuntimeError::Stack)?
                {
                    LoopRepeat::Complete => next,
                    LoopRepeat::Repeat { continuation } => {
                        actor.base.path = Some(continuation);
                        if immediate {
                            // Immediate NEXT alone re-enters at $7F:7E53;
                            // ordinary advances/jumps enter at $7F:7E75.
                            self.enter(objects, owner)?;
                            return Ok(ControlStep::Continue);
                        }
                        return Ok(ControlStep::Movement);
                    }
                }
            }
            ControlCommand::Break { target } => {
                actor
                    .extension
                    .path_state
                    .stack
                    .discard(&mut self.resources)
                    .map_err(PathRuntimeError::Stack)?;
                target
            }
            ControlCommand::PopStackPair { next } => {
                actor
                    .extension
                    .path_state
                    .stack
                    .discard(&mut self.resources)
                    .map_err(PathRuntimeError::Stack)?;
                next
            }
            ControlCommand::Register { trigger, next } => {
                self.add_trigger(objects, owner, trigger)?;
                next
            }
            ControlCommand::Cancel { path, next } => {
                self.cancel_trigger(objects, owner, path)?;
                next
            }
            ControlCommand::Clear { next } => {
                self.clear_triggers(objects, owner)?;
                next
            }
            ControlCommand::ForceAfterCallbacks { target, next } => {
                self.redirect(objects, owner, target, next, true)?;
                return Ok(ControlStep::Continue);
            }
            ControlCommand::CallAfterCallbacks { target, next } => {
                self.redirect(objects, owner, target, next, false)?;
                return Ok(ControlStep::Continue);
            }
        };
        objects
            .get_mut(owner)
            .expect("validated actor remains live")
            .base
            .path = Some(next);
        Ok(ControlStep::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_control::PlayerTarget;
    use crate::path_runtime::{CallbackStep, TriggerWorldInputs};
    use crate::path_triggers::TriggerKind;
    use crate::{Behavior, Object, ObjectKind, PathId, ShapeId};

    fn cursor(command_index: u16) -> PathCursor {
        PathCursor {
            path: PathId::from_catalog_index(0),
            command_index,
        }
    }

    fn setup() -> (PathRuntime, ObjectStore, ObjectId) {
        let mut objects = ObjectStore::new();
        let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
        actor.base.path = Some(cursor(0));
        let owner = objects.allocate(actor).unwrap();
        (PathRuntime::default(), objects, owner)
    }

    #[test]
    fn facing_advances_without_motion_and_requires_a_live_path() {
        use super::super::path_steering::{FacingCommand, FacingTargets};
        let (mut runtime, mut objects, owner) = setup();
        let target = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        objects.get_mut(target).unwrap().base.position.x = 100;
        let targets = FacingTargets {
            selected: Some(target),
            ..FacingTargets::default()
        };
        assert_eq!(
            runtime
                .execute_facing(
                    &mut objects,
                    owner,
                    FacingCommand::SelectedImmediate,
                    targets,
                    cursor(8)
                )
                .unwrap(),
            ControlStep::Continue
        );
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.yaw.units(), 192);
        assert_eq!(actor.base.position, crate::Vector3::default());
        assert_eq!(actor.base.path, Some(cursor(8)));
        objects.get_mut(owner).unwrap().base.path = None;
        assert_eq!(
            runtime.execute_facing(
                &mut objects,
                owner,
                FacingCommand::SelectedImmediate,
                targets,
                cursor(9)
            ),
            Err(PathRuntimeError::MissingPath(owner))
        );
    }

    #[test]
    fn branch_latch_is_shared_across_actors_and_nonconsuming_predicates() {
        use super::super::path_conditions::Predicate;
        let (mut runtime, mut objects, owner) = setup();
        let other = objects
            .allocate(objects.get(owner).unwrap().clone())
            .unwrap();
        assert_eq!(
            runtime
                .execute_branch(
                    &mut objects,
                    owner,
                    BranchCommand::InvertNext { next: cursor(1) }
                )
                .unwrap(),
            ControlStep::Continue
        );
        runtime.enter(&objects, other).unwrap();
        assert_eq!(
            runtime
                .execute_branch(
                    &mut objects,
                    other,
                    BranchCommand::Test {
                        predicate: Predicate::NonzeroWord(1),
                        taken: cursor(5),
                        next: cursor(6),
                    },
                )
                .unwrap(),
            ControlStep::Continue
        );
        assert_eq!(objects.get(other).unwrap().base.path, Some(cursor(5)));
        assert!(runtime.branch.invert_next);
        assert_eq!(
            runtime
                .execute_branch(
                    &mut objects,
                    other,
                    BranchCommand::Test {
                        predicate: Predicate::EqualByte {
                            value: 7,
                            expected: 7,
                        },
                        taken: cursor(8),
                        next: cursor(9),
                    },
                )
                .unwrap(),
            ControlStep::Continue
        );
        assert_eq!(objects.get(other).unwrap().base.path, Some(cursor(9)));
        assert!(!runtime.branch.invert_next);
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(1)));
        objects.get_mut(other).unwrap().base.path = None;
        assert_eq!(
            runtime.execute_branch(
                &mut objects,
                other,
                BranchCommand::InvertNext { next: cursor(3) }
            ),
            Err(PathRuntimeError::MissingPath(other))
        );
        assert!(!runtime.branch.invert_next);
    }

    #[test]
    fn hit_branches_consume_only_their_own_latches_and_leave_inversion_pending() {
        let (mut runtime, mut objects, owner) = setup();
        runtime.branch.invert_next = true;
        objects.get_mut(owner).unwrap().base.hit_flags = 0b1011;
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .hit_event_pending = true;
        for (mask, expected_path, expected_flags) in
            [(0, 2, 11), (5, 1, 10), (5, 2, 10), (10, 1, 0)]
        {
            assert_eq!(
                runtime
                    .execute_branch(
                        &mut objects,
                        owner,
                        BranchCommand::HitFlags {
                            mask,
                            taken: cursor(1),
                            next: cursor(2),
                        }
                    )
                    .unwrap(),
                ControlStep::Continue
            );
            assert_eq!(
                objects.get(owner).unwrap().base.path,
                Some(cursor(expected_path))
            );
            assert_eq!(objects.get(owner).unwrap().base.hit_flags, expected_flags);
            assert!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .hit_event_pending
            );
        }
        for expected_path in [3, 4] {
            assert_eq!(
                runtime
                    .execute_branch(
                        &mut objects,
                        owner,
                        BranchCommand::HitEvent {
                            taken: cursor(3),
                            next: cursor(4),
                        }
                    )
                    .unwrap(),
                ControlStep::Continue
            );
            assert_eq!(
                objects.get(owner).unwrap().base.path,
                Some(cursor(expected_path))
            );
        }
        assert!(runtime.branch.invert_next);
    }

    #[test]
    fn spatial_branches_read_live_actors_and_do_not_replace_selection_with_link() {
        use super::super::path_conditions::SpatialCondition;
        let (mut runtime, mut objects, owner) = setup();
        let selected = objects
            .allocate(objects.get(owner).unwrap().clone())
            .unwrap();
        let linked = objects
            .allocate(objects.get(owner).unwrap().clone())
            .unwrap();
        objects.get_mut(owner).unwrap().base.attachment = Some(linked);
        objects.get_mut(linked).unwrap().base.position.x = 100;
        objects.get_mut(selected).unwrap().base.position.x = 1;
        for (condition, expected) in [
            (SpatialCondition::LinkedDistanceLess(50), 2),
            (SpatialCondition::SelectedDistanceLess(50), 1),
        ] {
            assert_eq!(
                runtime
                    .execute_spatial_branch(
                        &mut objects,
                        owner,
                        Some(selected),
                        condition,
                        cursor(1),
                        cursor(2)
                    )
                    .unwrap(),
                ControlStep::Continue
            );
            assert_eq!(
                objects.get(owner).unwrap().base.path,
                Some(cursor(expected))
            );
        }
        objects.get_mut(selected).unwrap().base.position.x = 100;
        assert_eq!(
            runtime
                .execute_spatial_branch(
                    &mut objects,
                    owner,
                    Some(selected),
                    SpatialCondition::SelectedDistanceLess(50),
                    cursor(1),
                    cursor(2)
                )
                .unwrap(),
            ControlStep::Continue
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(2)));
        objects.get_mut(owner).unwrap().base.attachment = None;
        runtime.branch.invert_next = true;
        assert_eq!(
            runtime
                .execute_spatial_branch(
                    &mut objects,
                    owner,
                    None,
                    SpatialCondition::LinkedDistanceLess(50),
                    cursor(1),
                    cursor(2)
                )
                .unwrap(),
            ControlStep::Continue
        );
        assert!(runtime.branch.invert_next);
        assert_eq!(
            runtime
                .execute_spatial_branch(
                    &mut objects,
                    owner,
                    None,
                    SpatialCondition::GroundThreshold(0),
                    cursor(1),
                    cursor(2)
                )
                .unwrap(),
            ControlStep::Continue
        );
        assert!(!runtime.branch.invert_next);
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(2)));
    }

    #[test]
    fn hold_installs_movement_only_and_end_clears_exit_latches_without_moving() {
        let (mut runtime, mut objects, owner) = setup();
        let original_path = objects.get(owner).unwrap().base.path;
        {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.wait_timer = 19;
            actor.base.position.x = 100;
            actor.base.velocity.x = 10;
            actor.base.contacts.new_contact_latched = true;
            actor.base.contacts.hit_by_primary = true;
            actor.extension.path_state.clear_on_path_exit_latch = true;
        }
        assert_eq!(
            runtime
                .execute_control(&mut objects, owner, ControlCommand::Hold)
                .unwrap(),
            ControlStep::Movement
        );
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.behavior, super::super::Behavior::PathMovement);
        assert!(actor.extension.path_state.hold_latched);
        assert!(actor.base.contacts.new_contact_latched);
        assert_eq!(
            (
                actor.base.path,
                actor.base.wait_timer,
                actor.base.position.x
            ),
            (original_path, 19, 100)
        );
        assert_eq!(
            runtime
                .execute_control(&mut objects, owner, ControlCommand::End)
                .unwrap(),
            ControlStep::Ended
        );
        let actor = objects.get(owner).unwrap();
        assert!(actor.base.flags.remove_after_tick);
        assert!(!actor.base.contacts.new_contact_latched);
        assert!(!actor.base.contacts.hit_by_primary);
        assert!(!actor.extension.path_state.clear_on_path_exit_latch);
        assert_eq!(
            (
                actor.base.path,
                actor.base.wait_timer,
                actor.base.position.x
            ),
            (original_path, 19, 100)
        );
    }

    #[test]
    fn terminal_commands_cannot_replace_callback_return() {
        let (mut runtime, mut objects, owner) = setup();
        runtime
            .add_trigger(
                &mut objects,
                owner,
                Trigger {
                    path: cursor(20),
                    kind: TriggerKind::Always,
                    timer: 0,
                },
            )
            .unwrap();
        runtime.begin_callbacks(&objects, owner).unwrap();
        assert_eq!(
            runtime
                .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                .unwrap(),
            CallbackStep::Run(cursor(20))
        );
        let before = objects.get(owner).unwrap().clone();
        for command in [
            ControlCommand::End,
            ControlCommand::Hold,
            ControlCommand::SuspendAndMove,
        ] {
            assert_eq!(
                runtime.execute_control(&mut objects, owner, command),
                Err(PathRuntimeError::InvalidTerminalCallback)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
        }
        assert_eq!(
            runtime
                .execute_control(&mut objects, owner, ControlCommand::Return)
                .unwrap(),
            ControlStep::ResumeCallbacks
        );
    }

    #[test]
    fn typed_field_mutations_advance_without_movement_or_speed_regeneration() {
        use super::super::path_fields::{
            Axis, ByteField, ByteOperand, ByteOperation, Mutation, WordField, WordOperand,
            WordOperation,
        };
        let (mut runtime, mut objects, owner) = setup();
        let speed = Mutation::Byte {
            field: ByteField::Speed,
            operation: ByteOperation::Assign(ByteOperand::Literal(200)),
        };
        assert_eq!(
            runtime
                .execute_mutation(&mut objects, owner, speed, cursor(2))
                .unwrap(),
            ControlStep::Continue
        );
        assert_eq!(objects.get(owner).unwrap().base.speed, 200);
        assert_eq!(
            objects.get(owner).unwrap().base.velocity,
            super::super::Vector3::default()
        );
        assert_eq!(
            runtime
                .execute_mutation(
                    &mut objects,
                    owner,
                    Mutation::Word {
                        field: WordField::Position(Axis::Y),
                        operation: WordOperation::Add(WordOperand::SignedByte(ByteOperand::Actor(
                            ByteField::Speed
                        ))),
                    },
                    cursor(3)
                )
                .unwrap(),
            ControlStep::Continue
        );
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.position.y, -56);
        assert_eq!(actor.base.path, Some(cursor(3)));
        objects.get_mut(owner).unwrap().base.path = None;
        assert_eq!(
            runtime.execute_mutation(&mut objects, owner, speed, cursor(4)),
            Err(PathRuntimeError::MissingPath(owner))
        );
    }

    #[test]
    fn wait_observes_all_byte_pairs_before_increment_and_success_clears_elapsed() {
        let (mut runtime, mut objects, owner) = setup();
        for elapsed in 0..=u8::MAX {
            for duration in 0..=u8::MAX {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.wait_timer = elapsed;
                actor.base.path = Some(cursor(0));
                let outcome = runtime
                    .execute_control(
                        &mut objects,
                        owner,
                        ControlCommand::Wait {
                            duration,
                            next: cursor(1),
                        },
                    )
                    .unwrap();
                let actor = objects.get(owner).unwrap();
                if elapsed == duration {
                    assert_eq!(
                        (outcome, actor.base.path, actor.base.wait_timer),
                        (ControlStep::Continue, Some(cursor(1)), 0)
                    );
                } else {
                    assert_eq!(
                        (outcome, actor.base.path, actor.base.wait_timer),
                        (
                            ControlStep::Movement,
                            Some(cursor(0)),
                            elapsed.wrapping_add(1)
                        )
                    );
                }
            }
        }
    }

    #[test]
    fn goto_wait_one_and_jump_preserve_elapsed_but_only_first_two_yield() {
        let (mut runtime, mut objects, owner) = setup();
        objects.get_mut(owner).unwrap().base.wait_timer = 253;
        for (command, expected) in [
            (
                ControlCommand::Goto { target: cursor(4) },
                ControlStep::Movement,
            ),
            (
                ControlCommand::WaitOne { next: cursor(4) },
                ControlStep::Movement,
            ),
            (
                ControlCommand::Jump { target: cursor(4) },
                ControlStep::Continue,
            ),
        ] {
            assert_eq!(
                runtime
                    .execute_control(&mut objects, owner, command)
                    .unwrap(),
                expected
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(
                (actor.base.path, actor.base.wait_timer),
                (Some(cursor(4)), 253)
            );
        }
    }

    #[test]
    fn byte_repeat_compares_before_increment_and_zero_count_is_not_a_word_loop() {
        let (mut runtime, mut objects, owner) = setup();
        for elapsed in 0..=u8::MAX {
            for count in 0..=u8::MAX {
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .repeat_counter = elapsed;
                let outcome = runtime
                    .execute_control(
                        &mut objects,
                        owner,
                        ControlCommand::Repeat {
                            count,
                            target: cursor(2),
                            next: cursor(3),
                        },
                    )
                    .unwrap();
                let actor = objects.get(owner).unwrap();
                if elapsed == count {
                    assert_eq!(
                        (
                            outcome,
                            actor.base.path,
                            actor.extension.path_state.repeat_counter
                        ),
                        (ControlStep::Continue, Some(cursor(3)), 0)
                    );
                } else {
                    assert_eq!(
                        (
                            outcome,
                            actor.base.path,
                            actor.extension.path_state.repeat_counter
                        ),
                        (
                            ControlStep::Movement,
                            Some(cursor(2)),
                            elapsed.wrapping_add(1)
                        )
                    );
                }
            }
        }
    }

    #[test]
    fn loop_repeat_yields_or_refreshes_selection_and_completion_never_yields() {
        for immediate in [false, true] {
            let (mut runtime, mut objects, owner) = setup();
            assert_eq!(
                runtime
                    .execute_control(
                        &mut objects,
                        owner,
                        ControlCommand::BeginLoop {
                            iterations: 2,
                            next: cursor(1)
                        }
                    )
                    .unwrap(),
                ControlStep::Continue
            );
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .conditions
                .selected_player = PlayerTarget::Secondary;
            assert_eq!(
                runtime
                    .execute_control(
                        &mut objects,
                        owner,
                        ControlCommand::Next {
                            immediate,
                            next: cursor(4)
                        }
                    )
                    .unwrap(),
                if immediate {
                    ControlStep::Continue
                } else {
                    ControlStep::Movement
                }
            );
            assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(1)));
            assert_eq!(
                runtime.selected_player(),
                if immediate {
                    PlayerTarget::Secondary
                } else {
                    PlayerTarget::Primary
                }
            );
            assert_eq!(
                runtime
                    .execute_control(
                        &mut objects,
                        owner,
                        ControlCommand::Next {
                            immediate,
                            next: cursor(4)
                        }
                    )
                    .unwrap(),
                ControlStep::Continue
            );
            assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(4)));
        }
    }

    #[test]
    fn break_and_pop_discard_only_the_inner_loop_and_normal_call_retains_parent() {
        for pop in [false, true] {
            let (mut runtime, mut objects, owner) = setup();
            for next in [cursor(1), cursor(2)] {
                assert_eq!(
                    runtime
                        .execute_control(
                            &mut objects,
                            owner,
                            ControlCommand::BeginLoop {
                                iterations: 2,
                                next
                            }
                        )
                        .unwrap(),
                    ControlStep::Continue
                );
            }
            let command = if pop {
                ControlCommand::PopStackPair { next: cursor(3) }
            } else {
                ControlCommand::Break { target: cursor(3) }
            };
            assert_eq!(
                runtime
                    .execute_control(&mut objects, owner, command)
                    .unwrap(),
                ControlStep::Continue
            );
            assert_eq!(
                runtime
                    .execute_control(
                        &mut objects,
                        owner,
                        ControlCommand::Call {
                            target: cursor(8),
                            next: cursor(4)
                        }
                    )
                    .unwrap(),
                ControlStep::Continue
            );
            assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(8)));
            assert_eq!(
                runtime
                    .execute_control(&mut objects, owner, ControlCommand::Return)
                    .unwrap(),
                ControlStep::Continue
            );
            assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(4)));
            assert_eq!(
                runtime
                    .execute_control(
                        &mut objects,
                        owner,
                        ControlCommand::Next {
                            immediate: false,
                            next: cursor(5)
                        }
                    )
                    .unwrap(),
                ControlStep::Movement
            );
            assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(1)));
        }
    }

    #[test]
    fn registration_callback_redirection_and_return_use_actual_control_dispatch() {
        let (mut runtime, mut objects, owner) = setup();
        let trigger = Trigger {
            path: cursor(20),
            kind: TriggerKind::Always,
            timer: 0,
        };
        for next in [cursor(1), cursor(2)] {
            assert_eq!(
                runtime
                    .execute_control(
                        &mut objects,
                        owner,
                        ControlCommand::Register { trigger, next }
                    )
                    .unwrap(),
                ControlStep::Continue
            );
        }
        assert_eq!(
            runtime
                .execute_control(
                    &mut objects,
                    owner,
                    ControlCommand::Cancel {
                        path: trigger.path,
                        next: cursor(3)
                    }
                )
                .unwrap(),
            ControlStep::Continue
        );
        assert_eq!(
            objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .triggers
                .entries(&runtime.resources, owner)
                .unwrap()
                .len(),
            1
        );
        runtime.begin_callbacks(&objects, owner).unwrap();
        assert_eq!(
            runtime
                .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                .unwrap(),
            CallbackStep::Run(cursor(20))
        );
        assert_eq!(
            runtime
                .execute_control(
                    &mut objects,
                    owner,
                    ControlCommand::ForceAfterCallbacks {
                        target: cursor(40),
                        next: cursor(21)
                    }
                )
                .unwrap(),
            ControlStep::Continue
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(21)));
        assert_eq!(
            runtime
                .execute_control(&mut objects, owner, ControlCommand::Return)
                .unwrap(),
            ControlStep::ResumeCallbacks
        );
        assert_eq!(
            runtime
                .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                .unwrap(),
            CallbackStep::Complete
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(40)));
        assert_eq!(
            runtime
                .execute_control(
                    &mut objects,
                    owner,
                    ControlCommand::Clear { next: cursor(41) }
                )
                .unwrap(),
            ControlStep::Continue
        );
        assert!(!runtime.begin_callbacks(&objects, owner).unwrap());
    }

    #[test]
    fn contact_callback_parameter_aliases_the_actual_speed_target() {
        let (_, mut objects, owner) = setup();
        for target in 0..=u8::MAX {
            objects.get_mut(owner).unwrap().base.target_speed = target;
            assert_eq!(
                crate::hit_response::HitActor::from_object(objects.get(owner).unwrap())
                    .contact_parameter,
                target
            );
        }
    }

    #[test]
    fn motion_configuration_continues_without_eager_acceleration_or_flag_regeneration() {
        let (mut runtime, mut objects, owner) = setup();
        let retained = crate::Vector3 { x: 1, y: 2, z: 3 };
        objects.get_mut(owner).unwrap().base.velocity = retained;
        for command in [
            MotionCommand::GenerateVelocityEachStep(true),
            MotionCommand::SetSpeed(31),
            MotionCommand::AccelerateTo {
                target: 40,
                amount: 3,
            },
            MotionCommand::FollowPlayerDisplacement(true),
            MotionCommand::BankTurn(true),
            MotionCommand::QuadrupleVelocity(true),
        ] {
            assert_eq!(
                runtime
                    .execute_motion(&mut objects, owner, command, cursor(1))
                    .unwrap(),
                ControlStep::Continue
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.path, Some(cursor(1)));
            assert_eq!(actor.base.velocity, retained);
        }
        let actor = objects.get(owner).unwrap();
        assert_eq!(
            (
                actor.base.speed,
                actor.base.target_speed,
                actor.base.acceleration
            ),
            (31, 40, 3)
        );
        assert_eq!(
            crate::hit_response::HitActor::from_object(actor).contact_parameter,
            40
        );
        assert_eq!(
            runtime
                .execute_motion(
                    &mut objects,
                    owner,
                    MotionCommand::GenerateVelocityEachStep(false),
                    cursor(2)
                )
                .unwrap(),
            ControlStep::Continue
        );
        assert_eq!(
            runtime
                .execute_motion(&mut objects, owner, MotionCommand::SetSpeed(50), cursor(3))
                .unwrap(),
            ControlStep::Continue
        );
        assert_eq!(
            objects.get(owner).unwrap().base.velocity,
            super::super::path_motion::direction_velocity(
                crate::Angle::ZERO,
                crate::Angle::ZERO,
                50,
                4
            )
        );
    }
}
