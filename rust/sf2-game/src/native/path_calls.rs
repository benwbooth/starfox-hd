//! Path calls and callback-batch continuation (`$7F:955E..95C7`,
//! `$7F:99A9`, `$7F:9D74..9DDD`, `$7F:BC5A`).
//!
//! Call depth belongs to the shared dispatcher, while continuations belong
//! to each actor's program stack. A callback-root return resumes the trigger
//! pass without consuming an ordinary call or loop entry.

use super::program_resources::ProgramResources;
use super::program_state::{PathStack, PathStackError, ProgramData};
use super::{ObjectId, PathCursor};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum PendingPath {
    #[default]
    Resume,
    Forced,
    Call(PathCursor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CallbackBatch {
    owner: ObjectId,
    interrupted: PathCursor,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathCalls {
    depth: u8,
    active: Option<CallbackBatch>,
    pending: PendingPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    ReentrantCallbacks,
    NoCallbackBatch,
    Stack(PathStackError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathReturn {
    Resume(PathCursor),
    CallbackComplete,
}

/// Immediate changes required by the redirect command. Its own path cursor
/// still advances normally; the requested destination is applied at batch end.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectEffects {
    AdvanceOnly,
    RestartPathStrategy,
    RestartPathStrategyAndClearWaitAndSteering,
}

impl PathCalls {
    /// Save the interrupted continuation only for a nonempty trigger list.
    /// Empty lists take the source's early exit, bypassing redirect handling.
    pub fn begin_callbacks(
        &mut self,
        owner: ObjectId,
        interrupted: PathCursor,
        has_triggers: bool,
    ) -> Result<bool, CallError> {
        if self.active.is_some() {
            return Err(CallError::ReentrantCallbacks);
        }
        if !has_triggers {
            return Ok(false);
        }
        self.active = Some(CallbackBatch { owner, interrupted });
        // The source leaves pending mode intact here. Only actual callback
        // entry resets it; a pass consisting entirely of skips retains it.
        Ok(true)
    }

    /// Invoke one eligible trigger path, not each candidate predicate.
    pub fn enter_callback(&mut self) -> Result<(), CallError> {
        if self.active.is_none() {
            return Err(CallError::NoCallbackBatch);
        }
        self.pending = PendingPath::Resume;
        self.depth = 1;
        Ok(())
    }

    /// Both direct and decoded table calls increment before storing their
    /// continuation. Source depth wraps; stack allocation faults are terminal.
    pub fn call(
        &mut self,
        stack: &mut PathStack,
        resources: &mut ProgramResources<ProgramData>,
        owner: ObjectId,
        continuation: PathCursor,
    ) -> Result<(), CallError> {
        self.depth = self.depth.wrapping_add(1);
        stack
            .push_call(resources, owner, continuation)
            .map_err(CallError::Stack)
    }

    pub fn return_from(
        &mut self,
        stack: &mut PathStack,
        resources: &mut ProgramResources<ProgramData>,
    ) -> Result<PathReturn, CallError> {
        if self.active.is_some() {
            self.depth = self.depth.wrapping_sub(1);
            if self.depth == 0 {
                return Ok(PathReturn::CallbackComplete);
            }
        }
        // Outside callbacks the shared depth is deliberately not decremented.
        stack
            .pop_call(resources)
            .map(PathReturn::Resume)
            .map_err(CallError::Stack)
    }

    /// Force replaces the interrupted path immediately in saved state. A
    /// later callback entry clears the mode but does not undo this replacement.
    pub fn force_after_callbacks(&mut self, destination: PathCursor) -> RedirectEffects {
        let Some(batch) = &mut self.active else {
            return RedirectEffects::AdvanceOnly;
        };
        batch.interrupted = destination;
        self.pending = PendingPath::Forced;
        RedirectEffects::RestartPathStrategyAndClearWaitAndSteering
    }

    /// Deferred call changes strategy but does not clear wait/steering state.
    pub fn call_after_callbacks(&mut self, destination: PathCursor) -> RedirectEffects {
        if self.active.is_none() {
            return RedirectEffects::AdvanceOnly;
        }
        self.pending = PendingPath::Call(destination);
        RedirectEffects::RestartPathStrategy
    }

    pub fn finish_callbacks(
        &mut self,
        stack: &mut PathStack,
        resources: &mut ProgramResources<ProgramData>,
    ) -> Result<PathCursor, CallError> {
        let batch = self.active.ok_or(CallError::NoCallbackBatch)?;
        let destination = match self.pending {
            // The forced-path hook in this source revision is a bare return.
            PendingPath::Resume | PendingPath::Forced => batch.interrupted,
            PendingPath::Call(destination) => {
                // The source subtracts the return handler's encoded width
                // before pushing; a decoded cursor stores the actual resume
                // instruction directly. This push does NOT increment depth.
                stack
                    .push_call(resources, batch.owner, batch.interrupted)
                    .map_err(CallError::Stack)?;
                destination
            }
        };
        self.active = None;
        Ok(destination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program_state::LoopRepeat;
    use crate::{Behavior, Object, ObjectKind, ObjectStore, PathId, ShapeId};

    fn owner() -> ObjectId {
        ObjectStore::new()
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Effect,
            ))
            .unwrap()
    }

    fn cursor(command_index: u16) -> PathCursor {
        PathCursor {
            path: PathId::from_catalog_index(0),
            command_index,
        }
    }

    #[test]
    fn callback_root_returns_without_consuming_parent_loop() {
        let owner = owner();
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let mut calls = PathCalls::default();
        stack.begin(&mut resources, owner, cursor(1), 2).unwrap();
        assert!(calls.begin_callbacks(owner, cursor(2), true).unwrap());
        calls.enter_callback().unwrap();
        calls
            .call(&mut stack, &mut resources, owner, cursor(10))
            .unwrap();
        assert_eq!(
            calls.return_from(&mut stack, &mut resources),
            Ok(PathReturn::Resume(cursor(10)))
        );
        assert_eq!(
            calls.return_from(&mut stack, &mut resources),
            Ok(PathReturn::CallbackComplete)
        );
        assert_eq!(
            calls.finish_callbacks(&mut stack, &mut resources),
            Ok(cursor(2))
        );
        assert_eq!(
            stack.next(&mut resources),
            Ok(LoopRepeat::Repeat {
                continuation: cursor(1)
            })
        );
    }

    #[test]
    fn normal_returns_do_not_decrement_shared_depth_and_callback_entry_resets_it() {
        let owner = owner();
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let mut calls = PathCalls::default();
        for index in 0..=u8::MAX {
            calls
                .call(&mut stack, &mut resources, owner, cursor(u16::from(index)))
                .unwrap();
            assert_eq!(
                calls.return_from(&mut stack, &mut resources),
                Ok(PathReturn::Resume(cursor(u16::from(index))))
            );
            assert_eq!(calls.depth, index.wrapping_add(1));
        }
        calls.begin_callbacks(owner, cursor(1), true).unwrap();
        calls.enter_callback().unwrap();
        assert_eq!(calls.depth, 1);
        assert_eq!(
            calls.return_from(&mut stack, &mut resources),
            Ok(PathReturn::CallbackComplete)
        );
    }

    #[test]
    fn later_callback_cancels_deferred_call_but_not_replaced_interrupted_path() {
        let owner = owner();
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let mut calls = PathCalls::default();
        calls.begin_callbacks(owner, cursor(1), true).unwrap();
        calls.enter_callback().unwrap();
        assert_eq!(
            calls.force_after_callbacks(cursor(2)),
            RedirectEffects::RestartPathStrategyAndClearWaitAndSteering
        );
        assert_eq!(
            calls.call_after_callbacks(cursor(3)),
            RedirectEffects::RestartPathStrategy
        );
        calls.return_from(&mut stack, &mut resources).unwrap();
        calls.enter_callback().unwrap();
        calls.return_from(&mut stack, &mut resources).unwrap();
        assert_eq!(
            calls.finish_callbacks(&mut stack, &mut resources),
            Ok(cursor(2))
        );
        assert_eq!(resources.owner_count(owner), 0);
    }

    #[test]
    fn deferred_call_pushes_exact_interrupted_continuation_without_depth_change() {
        let owner = owner();
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let mut calls = PathCalls::default();
        calls.begin_callbacks(owner, cursor(1), true).unwrap();
        calls.enter_callback().unwrap();
        assert_eq!(
            calls.force_after_callbacks(cursor(2)),
            RedirectEffects::RestartPathStrategyAndClearWaitAndSteering
        );
        assert_eq!(
            calls.call_after_callbacks(cursor(3)),
            RedirectEffects::RestartPathStrategy
        );
        calls.return_from(&mut stack, &mut resources).unwrap();
        assert_eq!(
            calls.finish_callbacks(&mut stack, &mut resources),
            Ok(cursor(3))
        );
        assert_eq!(calls.depth, 0);
        assert_eq!(
            calls.return_from(&mut stack, &mut resources),
            Ok(PathReturn::Resume(cursor(2)))
        );
    }

    #[test]
    fn pending_mode_survives_empty_and_all_skipped_passes_until_actual_entry() {
        let owner = owner();
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let mut calls = PathCalls::default();
        calls.begin_callbacks(owner, cursor(1), true).unwrap();
        calls.enter_callback().unwrap();
        assert_eq!(
            calls.call_after_callbacks(cursor(3)),
            RedirectEffects::RestartPathStrategy
        );
        calls.return_from(&mut stack, &mut resources).unwrap();
        assert_eq!(
            calls.finish_callbacks(&mut stack, &mut resources),
            Ok(cursor(3))
        );
        assert!(!calls.begin_callbacks(owner, cursor(4), false).unwrap());
        calls.begin_callbacks(owner, cursor(5), true).unwrap();
        assert_eq!(
            calls.finish_callbacks(&mut stack, &mut resources),
            Ok(cursor(3))
        );
        assert_eq!(stack.pop_call(&mut resources), Ok(cursor(5)));
        assert_eq!(stack.pop_call(&mut resources), Ok(cursor(1)));
        calls.begin_callbacks(owner, cursor(6), true).unwrap();
        calls.enter_callback().unwrap();
        calls.return_from(&mut stack, &mut resources).unwrap();
        assert_eq!(
            calls.finish_callbacks(&mut stack, &mut resources),
            Ok(cursor(6))
        );
    }

    #[test]
    fn rejects_reentry_and_redirects_outside_callbacks_only_advance() {
        let owner = owner();
        let mut calls = PathCalls::default();
        assert_eq!(calls.enter_callback(), Err(CallError::NoCallbackBatch));
        assert_eq!(
            calls.force_after_callbacks(cursor(1)),
            RedirectEffects::AdvanceOnly
        );
        assert_eq!(
            calls.call_after_callbacks(cursor(1)),
            RedirectEffects::AdvanceOnly
        );
        calls.begin_callbacks(owner, cursor(2), true).unwrap();
        assert_eq!(
            calls.begin_callbacks(owner, cursor(3), false),
            Err(CallError::ReentrantCallbacks)
        );
    }
}
