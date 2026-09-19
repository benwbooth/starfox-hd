//! Live actor integration for path calls and mutable callback batches.
//! Source entry selection is refreshed on every callback (`$7F:7E53`),
//! whereas predicates observe the previously selected world actor.

use super::path_calls::{CallError, PathCalls, PathReturn, RedirectEffects};
use super::path_control::PlayerTarget;
use super::path_trigger_conditions::{
    self, ControlledAuxFlags, PlayerPartTarget, TriggerActorState, TriggerDecision, TriggerInputs,
};
use super::path_triggers::{Trigger, TriggerError, TriggerList, TriggerRunner, TriggerStep};
use super::program_resources::ProgramResources;
use super::program_state::{PathStack, PathStackError, ProgramData};
use super::{Behavior, Object, ObjectId, ObjectStore, PathCursor};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActorPathState {
    pub animation: super::path_appearance::AnimationChannels,
    /// Retained motion phase word (source extension 1CE2). Some paths use its
    /// low byte as a phase counter; player motion also uses it as an angle.
    pub motion_phase: u16,
    /// PATHHOLD sets source actor flag 09 bit 08 and retains its cursor.
    pub hold_latched: bool,
    pub motion: super::path_motion::MotionSettings,
    pub platform_carry: super::platform_carry::PlatformCarryState,
    /// Source 21 bit 80 is cleared at the common path exit. Its producer's
    /// gameplay meaning is not yet established; it is not a ground-hit test.
    pub clear_on_path_exit_latch: bool,
    pub stack: PathStack,
    pub triggers: TriggerList,
    pub conditions: TriggerActorState,
    /// Byte-counted LOOP counter, cleared by forced-path redirection (15).
    /// Separate from word-counted DO/NEXT entries on the shared path stack.
    pub repeat_counter: u8,
    /// Authored actor part identifier (parallel actor field EA).
    pub part: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveCallbacks {
    owner: ObjectId,
    executing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathRuntimeError {
    MissingActor(ObjectId),
    MissingPath(ObjectId),
    WrongCallbackOwner,
    CallbackStillExecuting,
    NoCallbackExecuting,
    MovementAlreadyActive,
    NoMovementActive,
    CallbacksStillActive,
    InvalidTerminalCallback,
    Attachments(super::attachments::AttachmentError),
    Steering(super::path_steering::SteeringError),
    Conditions(super::path_conditions::ConditionError),
    Calls(CallError),
    Triggers(TriggerError),
    Stack(PathStackError),
}

#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackStep {
    /// Cursor has been restored or redirected on the live object.
    Complete,
    Expired,
    Skipped,
    /// Execute the live object's path until its callback-root return.
    Run(PathCursor),
}

/// Observations supplied afresh by the world for each candidate. Actor-local
/// fields are deliberately absent; the coordinator reads those live itself.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TriggerWorldInputs {
    pub strategy_tick: u8,
    pub player_parts: [Option<PlayerPartTarget>; 2],
    pub player_projections: [Option<i16>; 2],
    pub controlled_aux: ControlledAuxFlags,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathRuntime {
    pub resources: ProgramResources<ProgramData>,
    pub steering: super::path_steering::SteeringState,
    pub branch: super::path_conditions::BranchState,
    calls: PathCalls,
    runner: TriggerRunner,
    active: Option<ActiveCallbacks>,
    movement: Option<ObjectId>,
    selected: PlayerTarget,
}

impl Default for PathRuntime {
    fn default() -> Self {
        Self {
            resources: ProgramResources::default(),
            steering: super::path_steering::SteeringState::default(),
            branch: super::path_conditions::BranchState::default(),
            calls: PathCalls::default(),
            runner: TriggerRunner::default(),
            active: None,
            movement: None,
            selected: PlayerTarget::Primary,
        }
    }
}

fn actor_mut(objects: &mut ObjectStore, owner: ObjectId) -> Result<&mut Object, PathRuntimeError> {
    objects
        .get_mut(owner)
        .ok_or(PathRuntimeError::MissingActor(owner))
}

impl PathRuntime {
    pub(super) fn validate_terminal_command(&self) -> Result<(), PathRuntimeError> {
        // Callbacks enter by a jump and return through their own continuation.
        // END's main-invocation exit is not a valid callback-root return;
        // PATHHOLD would attempt a nested movement/callback invocation.
        if self.active.is_some() {
            return Err(PathRuntimeError::InvalidTerminalCallback);
        }
        Ok(())
    }

    pub fn selected_player(&self) -> PlayerTarget {
        self.selected
    }

    /// Start exactly one movement invocation. The returned flag tells the
    /// caller whether to service callbacks before calling finish_movement.
    /// Displacement must be sampled using the selection that exists now;
    /// callbacks may change that selection before the final carry service.
    pub fn begin_movement(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        player: super::path_motion::PlayerDisplacement,
    ) -> Result<bool, PathRuntimeError> {
        if self.movement.is_some() {
            return Err(PathRuntimeError::MovementAlreadyActive);
        }
        if self.active.is_some() {
            return Err(PathRuntimeError::Calls(CallError::ReentrantCallbacks));
        }
        let actor = actor_mut(objects, owner)?;
        if actor.base.path.is_none() {
            return Err(PathRuntimeError::MissingPath(owner));
        }
        super::path_motion::before_callbacks(actor, owner, player);
        self.movement = Some(owner);
        self.begin_callbacks(objects, owner)
    }

    /// Finish the active movement after all callbacks, including skips and
    /// expired registrations, have been visited. Resolve the final selected
    /// player's auxiliary state here instead of retaining the entry target.
    pub fn finish_movement(
        &mut self,
        objects: &mut ObjectStore,
        players: &mut [Option<super::platform_carry::CarriedPlayer>; 2],
    ) -> Result<(), PathRuntimeError> {
        let owner = self.movement.ok_or(PathRuntimeError::NoMovementActive)?;
        if self.active.is_some() {
            return Err(PathRuntimeError::CallbacksStillActive);
        }
        let selected = match self.selected {
            PlayerTarget::Primary => 0,
            PlayerTarget::Secondary => 1,
        };
        super::path_motion::after_callbacks(objects, owner, players[selected].as_mut())
            .map_err(PathRuntimeError::Attachments)?;
        self.movement = None;
        Ok(())
    }

    /// Common entry, including callback entry: selection follows this actor's
    /// saved side even when the predicate itself did not select a new player.
    pub fn enter(
        &mut self,
        objects: &ObjectStore,
        owner: ObjectId,
    ) -> Result<PathCursor, PathRuntimeError> {
        let actor = objects
            .get(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        let cursor = actor
            .base
            .path
            .ok_or(PathRuntimeError::MissingPath(owner))?;
        self.selected = actor.extension.path_state.conditions.selected_player;
        Ok(cursor)
    }

    pub fn add_trigger(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        trigger: Trigger,
    ) -> Result<(), PathRuntimeError> {
        actor_mut(objects, owner)?
            .extension
            .path_state
            .triggers
            .add(&mut self.resources, owner, trigger)
            .map_err(PathRuntimeError::Triggers)
    }

    pub fn cancel_trigger(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        path: PathCursor,
    ) -> Result<bool, PathRuntimeError> {
        actor_mut(objects, owner)?
            .extension
            .path_state
            .triggers
            .cancel(&mut self.resources, owner, &mut self.runner, path)
            .map_err(PathRuntimeError::Triggers)
    }

    pub fn clear_triggers(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
    ) -> Result<(), PathRuntimeError> {
        actor_mut(objects, owner)?
            .extension
            .path_state
            .triggers
            .clear(&mut self.resources, owner, &mut self.runner)
            .map_err(PathRuntimeError::Triggers)
    }

    pub fn begin_callbacks(
        &mut self,
        objects: &ObjectStore,
        owner: ObjectId,
    ) -> Result<bool, PathRuntimeError> {
        if self.active.is_some() {
            return Err(PathRuntimeError::Calls(CallError::ReentrantCallbacks));
        }
        let actor = objects
            .get(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        let interrupted = actor
            .base
            .path
            .ok_or(PathRuntimeError::MissingPath(owner))?;
        let list = &actor.extension.path_state.triggers;
        let has_triggers = !list
            .entries(&self.resources, owner)
            .map_err(PathRuntimeError::Triggers)?
            .is_empty();
        if !self
            .calls
            .begin_callbacks(owner, interrupted, has_triggers)
            .map_err(PathRuntimeError::Calls)?
        {
            return Ok(false);
        }
        self.runner
            .begin(list, &self.resources, owner)
            .map_err(PathRuntimeError::Triggers)?;
        self.active = Some(ActiveCallbacks {
            owner,
            executing: false,
        });
        Ok(true)
    }

    /// Inspect exactly one candidate with fresh world inputs. Actor-local
    /// observations are always read here from the live object, not the input
    /// snapshot: earlier callbacks may have changed health or contact flags.
    pub fn step_callbacks(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        world: TriggerWorldInputs,
    ) -> Result<CallbackStep, PathRuntimeError> {
        let active = self
            .active
            .ok_or(PathRuntimeError::Calls(CallError::NoCallbackBatch))?;
        if active.owner != owner {
            return Err(PathRuntimeError::WrongCallbackOwner);
        }
        if active.executing {
            return Err(PathRuntimeError::CallbackStillExecuting);
        }
        let actor = actor_mut(objects, active.owner)?;
        let inputs = TriggerInputs {
            owner,
            part: actor.extension.path_state.part,
            health: actor.base.hit_points,
            attached: actor.base.attachment.is_some(),
            new_contact: actor.base.contacts.new_contact_latched,
            contacted_players: [
                actor.base.contacts.hit_by_primary,
                actor.base.contacts.hit_by_secondary,
            ],
            strategy_tick: world.strategy_tick,
            player_parts: world.player_parts,
            player_projections: world.player_projections,
            controlled_aux: world.controlled_aux,
        };
        let state = &mut actor.extension.path_state;
        match self
            .runner
            .step(&mut state.triggers, &mut self.resources)
            .map_err(PathRuntimeError::Triggers)?
        {
            TriggerStep::Complete => {
                actor.base.path = Some(
                    self.calls
                        .finish_callbacks(&mut state.stack, &mut self.resources)
                        .map_err(PathRuntimeError::Calls)?,
                );
                self.active = None;
                Ok(CallbackStep::Complete)
            }
            TriggerStep::Expired => Ok(CallbackStep::Expired),
            TriggerStep::Candidate(trigger) => {
                match path_trigger_conditions::evaluate(
                    trigger.kind,
                    &inputs,
                    &mut state.conditions,
                    &self.runner,
                ) {
                    TriggerDecision::Skip => return Ok(CallbackStep::Skipped),
                    TriggerDecision::Run | TriggerDecision::SelectAndRun(_) => {}
                }
                self.calls
                    .enter_callback()
                    .map_err(PathRuntimeError::Calls)?;
                actor.base.path = Some(trigger.path);
                self.selected = state.conditions.selected_player;
                self.active
                    .as_mut()
                    .expect("active callback pass")
                    .executing = true;
                Ok(CallbackStep::Run(trigger.path))
            }
        }
    }

    pub fn call(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        destination: PathCursor,
        continuation: PathCursor,
    ) -> Result<(), PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = actor_mut(objects, owner)?;
        self.calls
            .call(
                &mut actor.extension.path_state.stack,
                &mut self.resources,
                owner,
                continuation,
            )
            .map_err(PathRuntimeError::Calls)?;
        actor.base.path = Some(destination);
        Ok(())
    }

    pub fn return_from(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
    ) -> Result<PathReturn, PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = actor_mut(objects, owner)?;
        let result = self
            .calls
            .return_from(&mut actor.extension.path_state.stack, &mut self.resources)
            .map_err(PathRuntimeError::Calls)?;
        match result {
            PathReturn::Resume(cursor) => actor.base.path = Some(cursor),
            PathReturn::CallbackComplete => {
                self.active.as_mut().expect("callback root").executing = false
            }
        }
        Ok(result)
    }

    pub(super) fn check_execution_owner(&self, owner: ObjectId) -> Result<(), PathRuntimeError> {
        if let Some(active) = self.active {
            if active.owner != owner {
                return Err(PathRuntimeError::WrongCallbackOwner);
            }
            if !active.executing {
                return Err(PathRuntimeError::NoCallbackExecuting);
            }
        }
        Ok(())
    }

    /// Both redirect commands advance their own callback cursor; the target
    /// is applied only at batch completion. Outside a batch it is ignored.
    pub fn redirect(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        destination: PathCursor,
        continuation: PathCursor,
        force: bool,
    ) -> Result<(), PathRuntimeError> {
        self.check_execution_owner(owner)?;
        let actor = actor_mut(objects, owner)?;
        let effects = if force {
            self.calls.force_after_callbacks(destination)
        } else {
            self.calls.call_after_callbacks(destination)
        };
        match effects {
            RedirectEffects::AdvanceOnly => {}
            RedirectEffects::RestartPathStrategy => actor.base.behavior = Behavior::FollowPath,
            RedirectEffects::RestartPathStrategyAndClearWaitAndRepeat => {
                actor.base.behavior = Behavior::FollowPath;
                actor.base.wait_timer = 0;
                actor.extension.path_state.repeat_counter = 0;
            }
        }
        actor.base.path = Some(continuation);
        Ok(())
    }

    /// Retirement calls this after contact separation and before slot reuse.
    /// Condition latches are actor state, not allocations, and remain intact.
    pub fn release_actor_programs(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
    ) -> Result<(), PathRuntimeError> {
        let actor = actor_mut(objects, owner)?;
        self.resources.release_owner(owner);
        let state = &mut actor.extension.path_state;
        state
            .stack
            .clear_released(&self.resources)
            .map_err(PathRuntimeError::Stack)?;
        state
            .triggers
            .clear_released(&self.resources)
            .map_err(PathRuntimeError::Triggers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision_contacts::{Contact, ContactHost, ContactId, ContactStore};
    use crate::hit_response::{
        self, HitActor, HitActorMut, HitCallback, HitContext, HitResponseHost, HitSide,
    };
    use crate::path_triggers::TriggerKind;
    use crate::program_resources::PROGRAM_CAPACITY;
    use crate::program_state::LoopRepeat;
    use crate::{ObjectKind, PathId, ShapeId};

    fn cursor(command_index: u16) -> PathCursor {
        PathCursor {
            path: PathId::from_catalog_index(0),
            command_index,
        }
    }

    fn actor() -> Object {
        let mut value = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
        value.base.path = Some(cursor(1));
        value.base.hit_points = 10;
        value
    }

    fn trigger(command_index: u16, kind: TriggerKind) -> Trigger {
        Trigger {
            path: cursor(command_index),
            kind,
            timer: 0,
        }
    }

    fn step(runtime: &mut PathRuntime, objects: &mut ObjectStore, owner: ObjectId) -> CallbackStep {
        runtime
            .step_callbacks(objects, owner, TriggerWorldInputs::default())
            .unwrap()
    }

    #[test]
    fn movement_reselects_carry_after_callback_and_preserves_phase_order() {
        use super::super::path_motion::PlayerDisplacement;
        use super::super::platform_carry::CarriedPlayer;
        use crate::Vector3;
        let mut objects = ObjectStore::new();
        let owner = objects.allocate(actor()).unwrap();
        let child = objects.allocate(actor()).unwrap();
        let parent = objects.get_mut(owner).unwrap();
        parent.base.velocity.x = 10;
        parent.base.first_child = Some(child);
        parent.base.contacts.new_contact_latched = true;
        parent.extension.path_state.motion.carry_selected_player = true;
        parent.extension.path_state.motion.refresh_child_chain = true;
        parent.extension.path_state.platform_carry.continuity = 1;
        objects.get_mut(child).unwrap().base.attachment = Some(owner);
        let mut runtime = PathRuntime::default();
        runtime
            .add_trigger(&mut objects, owner, trigger(20, TriggerKind::Always))
            .unwrap();
        runtime.enter(&objects, owner).unwrap();
        assert!(runtime
            .begin_movement(&mut objects, owner, PlayerDisplacement::default())
            .unwrap());
        assert_eq!(objects.get(owner).unwrap().base.position.x, 10);
        let player = CarriedPlayer {
            enabled: true,
            carrier: Some(owner),
            ..Default::default()
        };
        let mut players = [Some(player), Some(player)];
        assert_eq!(
            runtime.finish_movement(&mut objects, &mut players),
            Err(PathRuntimeError::CallbacksStillActive)
        );
        assert_eq!(
            runtime.begin_movement(&mut objects, owner, PlayerDisplacement::default()),
            Err(PathRuntimeError::MovementAlreadyActive)
        );
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = PlayerTarget::Secondary;
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Run(cursor(20))
        );
        let parent = objects.get_mut(owner).unwrap();
        assert!(parent.base.contacts.new_contact_latched);
        parent.base.position.x = 100;
        parent.base.velocity.x = 3;
        parent.extension.path_state.motion.relative_coordinates = true;
        assert_eq!(
            runtime.return_from(&mut objects, owner).unwrap(),
            PathReturn::CallbackComplete
        );
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Complete
        );
        runtime.finish_movement(&mut objects, &mut players).unwrap();
        assert_eq!(players[0], Some(player));
        assert_eq!(players[1].unwrap().origin, Vector3 { x: 100, y: 0, z: 0 });
        assert_eq!(objects.get(child).unwrap().base.position.x, 100);
        assert_eq!(objects.get(owner).unwrap().extension.relative_position.x, 3);
        assert!(
            !objects
                .get(owner)
                .unwrap()
                .base
                .contacts
                .new_contact_latched
        );
        assert_eq!(
            runtime.finish_movement(&mut objects, &mut players),
            Err(PathRuntimeError::NoMovementActive)
        );
    }

    #[test]
    fn empty_callback_movement_still_requires_post_phase_without_refreshing_selection() {
        use super::super::path_motion::PlayerDisplacement;
        let mut objects = ObjectStore::new();
        let owner = objects.allocate(actor()).unwrap();
        let other = objects.allocate(actor()).unwrap();
        let mut runtime = PathRuntime::default();
        objects
            .get_mut(other)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = PlayerTarget::Secondary;
        runtime.enter(&objects, other).unwrap();
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .clear_on_path_exit_latch = true;
        assert!(!runtime
            .begin_movement(&mut objects, owner, PlayerDisplacement::default())
            .unwrap());
        assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
        assert!(
            objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .clear_on_path_exit_latch
        );
        runtime
            .finish_movement(&mut objects, &mut [None, None])
            .unwrap();
        assert!(
            !objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .clear_on_path_exit_latch
        );
    }

    #[test]
    fn callbacks_mutate_live_cursor_preserve_parent_loop_and_refresh_entry_selection() {
        let mut objects = ObjectStore::new();
        let owner = objects.allocate(actor()).unwrap();
        let other = objects.allocate(actor()).unwrap();
        let mut runtime = PathRuntime::default();
        objects
            .get_mut(other)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = PlayerTarget::Secondary;
        runtime.enter(&objects, other).unwrap();
        assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
        runtime
            .add_trigger(&mut objects, owner, trigger(20, TriggerKind::Always))
            .unwrap();
        runtime
            .add_trigger(&mut objects, owner, trigger(40, TriggerKind::ZeroHealth))
            .unwrap();
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .stack
            .begin(&mut runtime.resources, owner, cursor(5), 2)
            .unwrap();
        assert!(runtime.begin_callbacks(&objects, owner).unwrap());
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Run(cursor(20))
        );
        assert_eq!(runtime.selected_player(), PlayerTarget::Primary);
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(20)));
        runtime
            .call(&mut objects, owner, cursor(30), cursor(21))
            .unwrap();
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(30)));
        assert_eq!(
            runtime.return_from(&mut objects, owner).unwrap(),
            PathReturn::Resume(cursor(21))
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(21)));
        objects.get_mut(owner).unwrap().base.hit_points = 0;
        assert_eq!(
            runtime.return_from(&mut objects, owner).unwrap(),
            PathReturn::CallbackComplete
        );
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Run(cursor(40))
        );
        runtime.return_from(&mut objects, owner).unwrap();
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Complete
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(1)));
        assert_eq!(
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .stack
                .next(&mut runtime.resources)
                .unwrap(),
            LoopRepeat::Repeat {
                continuation: cursor(5)
            }
        );
    }

    #[test]
    fn redirects_apply_immediate_actor_effects_but_defer_destination_and_can_be_overwritten() {
        let mut objects = ObjectStore::new();
        let owner = objects.allocate(actor()).unwrap();
        let mut runtime = PathRuntime::default();
        runtime
            .add_trigger(&mut objects, owner, trigger(20, TriggerKind::Always))
            .unwrap();
        runtime
            .add_trigger(&mut objects, owner, trigger(40, TriggerKind::Always))
            .unwrap();
        runtime.begin_callbacks(&objects, owner).unwrap();
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Run(cursor(20))
        );
        let actor = objects.get_mut(owner).unwrap();
        actor.base.behavior = Behavior::EnemyFlight;
        actor.base.wait_timer = 19;
        actor.extension.path_state.repeat_counter = 29;
        runtime
            .redirect(&mut objects, owner, cursor(100), cursor(21), true)
            .unwrap();
        let actor = objects.get(owner).unwrap();
        assert_eq!(
            (
                actor.base.behavior,
                actor.base.wait_timer,
                actor.extension.path_state.repeat_counter,
                actor.base.path
            ),
            (Behavior::FollowPath, 0, 0, Some(cursor(21)))
        );
        runtime.return_from(&mut objects, owner).unwrap();
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Run(cursor(40))
        );
        let actor = objects.get_mut(owner).unwrap();
        actor.base.wait_timer = 11;
        actor.extension.path_state.repeat_counter = 12;
        runtime
            .redirect(&mut objects, owner, cursor(200), cursor(41), false)
            .unwrap();
        runtime.return_from(&mut objects, owner).unwrap();
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Complete
        );
        let actor = objects.get(owner).unwrap();
        assert_eq!(
            (
                actor.base.wait_timer,
                actor.extension.path_state.repeat_counter,
                actor.base.path
            ),
            (11, 12, Some(cursor(200)))
        );
        assert_eq!(
            runtime.return_from(&mut objects, owner).unwrap(),
            PathReturn::Resume(cursor(100))
        );
        // Outside a callback the destination is ignored, with no actor reset.
        runtime
            .redirect(&mut objects, owner, cursor(300), cursor(101), true)
            .unwrap();
        let actor = objects.get(owner).unwrap();
        assert_eq!(
            (
                actor.base.wait_timer,
                actor.extension.path_state.repeat_counter,
                actor.base.path
            ),
            (11, 12, Some(cursor(101)))
        );
    }

    #[test]
    fn mutable_pass_clears_and_replaces_list_without_visiting_new_registration() {
        let mut objects = ObjectStore::new();
        let owner = objects.allocate(actor()).unwrap();
        let mut runtime = PathRuntime::default();
        for command_index in [20, 40, 60] {
            runtime
                .add_trigger(
                    &mut objects,
                    owner,
                    trigger(command_index, TriggerKind::Always),
                )
                .unwrap();
        }
        runtime.begin_callbacks(&objects, owner).unwrap();
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Run(cursor(20))
        );
        runtime.clear_triggers(&mut objects, owner).unwrap();
        runtime
            .add_trigger(&mut objects, owner, trigger(80, TriggerKind::Always))
            .unwrap();
        runtime.return_from(&mut objects, owner).unwrap();
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Complete
        );
        runtime.begin_callbacks(&objects, owner).unwrap();
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Run(cursor(80))
        );
        runtime.return_from(&mut objects, owner).unwrap();
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Complete
        );
        runtime
            .call(&mut objects, owner, cursor(100), cursor(1))
            .unwrap();
        assert_eq!(runtime.resources.owner_count(owner), 2);
        runtime.release_actor_programs(&mut objects, owner).unwrap();
        assert_eq!(runtime.resources.available_capacity(), PROGRAM_CAPACITY);
        assert!(!runtime.begin_callbacks(&objects, owner).unwrap());
        runtime
            .add_trigger(&mut objects, owner, trigger(120, TriggerKind::Always))
            .unwrap();
        assert_eq!(runtime.resources.owner_count(owner), 1);
    }

    #[test]
    fn invalid_host_order_is_rejected_without_advancing_the_live_callback() {
        let mut objects = ObjectStore::new();
        let owner = objects.allocate(actor()).unwrap();
        let other = objects.allocate(actor()).unwrap();
        let mut runtime = PathRuntime::default();
        runtime
            .add_trigger(&mut objects, owner, trigger(20, TriggerKind::Always))
            .unwrap();
        runtime.begin_callbacks(&objects, owner).unwrap();
        assert_eq!(
            runtime.step_callbacks(&mut objects, other, TriggerWorldInputs::default()),
            Err(PathRuntimeError::WrongCallbackOwner)
        );
        assert_eq!(
            runtime.return_from(&mut objects, owner),
            Err(PathRuntimeError::NoCallbackExecuting)
        );
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Run(cursor(20))
        );
        assert_eq!(
            runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
            Err(PathRuntimeError::CallbackStillExecuting)
        );
        assert_eq!(
            runtime.return_from(&mut objects, other),
            Err(PathRuntimeError::WrongCallbackOwner)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(20)));
        runtime.return_from(&mut objects, owner).unwrap();
        assert_eq!(
            step(&mut runtime, &mut objects, owner),
            CallbackStep::Complete
        );
    }

    struct ContactWorld {
        objects: ObjectStore,
        contacts: ContactStore,
        paths: PathRuntime,
        callback_steps: Vec<CallbackStep>,
    }

    impl ContactHost for ContactWorld {
        type Error = String;
        fn contacts(&self) -> &ContactStore {
            &self.contacts
        }
        fn contacts_mut(&mut self) -> &mut ContactStore {
            &mut self.contacts
        }
        fn on_separation(&mut self, _: ContactId, _: Contact) -> Result<(), Self::Error> {
            panic!("this test performs hit response, not separation")
        }
    }

    impl HitResponseHost for ContactWorld {
        fn hit_actor(&self, id: ObjectId) -> Option<HitActor> {
            self.objects.get(id).map(HitActor::from_object)
        }
        fn hit_actor_mut(&mut self, id: ObjectId) -> Option<HitActorMut<'_>> {
            self.objects.get_mut(id).map(HitActorMut::from_object)
        }
        fn has_hit_callback(&self, _: ObjectId, kind: HitCallback) -> bool {
            kind == HitCallback::NewContact
        }
        fn run_hit_callback(
            &mut self,
            owner: ObjectId,
            _: ObjectId,
            _: HitCallback,
            context: &mut HitContext,
        ) -> Result<(), Self::Error> {
            // Synthetic authored callback: health changes before final damage.
            self.objects.get_mut(owner).unwrap().base.hit_points = 3;
            context.damage = 3;
            Ok(())
        }
        fn strategies_paused(&self) -> bool {
            false
        }
        fn run_assigned_strategy(&mut self, owner: ObjectId) -> Result<(), Self::Error> {
            self.paths
                .begin_callbacks(&self.objects, owner)
                .map_err(|e| format!("{e:?}"))?;
            loop {
                let step = self
                    .paths
                    .step_callbacks(&mut self.objects, owner, TriggerWorldInputs::default())
                    .map_err(|e| format!("{e:?}"))?;
                self.callback_steps.push(step);
                match step {
                    CallbackStep::Run(_) => {
                        self.paths
                            .return_from(&mut self.objects, owner)
                            .map_err(|e| format!("{e:?}"))?;
                    }
                    CallbackStep::Complete => return Ok(()),
                    CallbackStep::Skipped | CallbackStep::Expired => {}
                }
            }
        }
    }

    #[test]
    fn damage_and_contact_predicates_share_actual_objects_not_copied_health_or_flags() {
        let mut world = ContactWorld {
            objects: ObjectStore::new(),
            contacts: ContactStore::default(),
            paths: PathRuntime::default(),
            callback_steps: Vec::new(),
        };
        let mut current = actor();
        current.base.contacts.latch_new_contact = true;
        let owner = world.objects.allocate(current).unwrap();
        let mut projectile = actor();
        projectile.base.attack_power = 7;
        projectile.base.contacts.mutually_non_damaging = true;
        projectile.base.contacts.credits_hit_side = true;
        projectile.base.contacts.hit_side = HitSide::Secondary;
        let other = world.objects.allocate(projectile).unwrap();
        world.contacts.record_pair(owner, other, [None; 2]).unwrap();
        for (command_index, kind) in [
            (20, TriggerKind::ZeroHealth),
            (40, TriggerKind::NewContact),
            (60, TriggerKind::PlayerContact),
        ] {
            world
                .paths
                .add_trigger(&mut world.objects, owner, trigger(command_index, kind))
                .unwrap();
        }
        hit_response::respond(&mut world, owner, &mut HitContext::default()).unwrap();
        assert_eq!(
            world.callback_steps,
            [
                CallbackStep::Run(cursor(20)),
                CallbackStep::Run(cursor(40)),
                CallbackStep::Run(cursor(60)),
                CallbackStep::Complete
            ]
        );
        let actor = world.objects.get(owner).unwrap();
        assert_eq!(actor.base.hit_points, 0);
        assert!(
            actor.base.contacts.hit_marked
                && actor.base.contacts.new_contact_latched
                && actor.base.contacts.hit_by_secondary
        );
        assert_eq!(
            actor.extension.path_state.conditions.selected_player,
            PlayerTarget::Secondary
        );
        assert_eq!(world.paths.selected_player(), PlayerTarget::Secondary);
        assert_eq!(actor.base.path, Some(cursor(1)));
    }
}
