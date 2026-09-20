//! Actor-owned path state decoded into typed records.
//!
//! Counted loops use two four-unit entries per nesting level. The source
//! reallocates on each eighth pushed entry, regardless of earlier capacity,
//! and retains empty stack storage until actor/program cleanup.

use super::path_control::CountedLoop;
use super::program_resources::{
    AllocationFailure, ProgramResourceId, ProgramResources, ReplacementError,
};
use super::{ObjectId, PathCursor};

const ENTRY_COST: u16 = 4;
const ENTRY_GROWTH: u16 = 8;
const COUNT_COST: u16 = 1;
const INITIAL_STACK_COST: u16 = COUNT_COST + ENTRY_GROWTH * ENTRY_COST;

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathEntry {
    CallReturn(PathCursor),
    Continuation(PathCursor),
    Counter(CountedLoop),
    SavedByte(u8),
    SavedWord(u16),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathEntries {
    entries: Vec<PathEntry>,
}

/// The shared allocation pool stores semantic records, not encoded records.
/// Additional program record families belong in this sum type as ported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramData {
    PathStack(PathEntries),
    PathTriggers(super::path_triggers::TriggerRecords),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathStackError {
    MissingStorage,
    MissingLoop,
    IncompleteLoop,
    IncompatibleSavedValue,
    EntryCountOverflow,
    StorageStillOwned,
    Allocation(AllocationFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopRepeat {
    /// Ordinary NEXT yields; NEXT-immediate resumes path dispatch immediately.
    Repeat {
        continuation: PathCursor,
    },
    Complete,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PathStack {
    storage: Option<ProgramResourceId>,
}

impl PathStack {
    /// Begin any decoded byte/word/immediate/variable count variant
    /// (`$7F:95CA..96AB`). The caller supplies the decoded body continuation.
    /// Errors denote source fault paths and must terminate path processing;
    /// the first push can remain present if the second push fails.
    pub fn begin(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
        owner: ObjectId,
        continuation: PathCursor,
        count: u16,
    ) -> Result<(), PathStackError> {
        self.push(resources, owner, PathEntry::Continuation(continuation))?;
        self.push(
            resources,
            owner,
            PathEntry::Counter(CountedLoop::new(count)),
        )
    }

    fn push(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
        owner: ObjectId,
        entry: PathEntry,
    ) -> Result<(), PathStackError> {
        let Some(old) = self.storage else {
            let data = ProgramData::PathStack(PathEntries {
                entries: vec![entry],
            });
            self.storage = Some(
                resources
                    .allocate_owned(owner, INITIAL_STACK_COST, data)
                    .map_err(|error| PathStackError::Allocation(error.reason))?,
            );
            return Ok(());
        };
        let ProgramData::PathStack(data) = resources
            .get_owned(owner, old)
            .ok_or(PathStackError::MissingStorage)?
        else {
            return Err(PathStackError::MissingStorage);
        };
        let next_count = data.entries.len() + 1;
        // A wrapping source count writes before its first entry and corrupts
        // ownership metadata; it is not a valid typed loop continuation.
        if next_count > usize::from(u8::MAX) {
            return Err(PathStackError::EntryCountOverflow);
        }
        if next_count % usize::from(ENTRY_GROWTH) == 0 {
            let mut replacement = data.clone();
            replacement.entries.push(entry);
            let cost = (next_count as u16 + ENTRY_GROWTH) * ENTRY_COST + COUNT_COST;
            self.storage = Some(
                resources
                    .replace_owned(owner, old, cost, ProgramData::PathStack(replacement))
                    .map_err(|error| match error {
                        ReplacementError::MissingOwnedResource(_) => PathStackError::MissingStorage,
                        ReplacementError::Allocation(error) => {
                            PathStackError::Allocation(error.reason)
                        }
                    })?,
            );
        } else {
            let ProgramData::PathStack(data) = resources
                .get_owned_mut(owner, old)
                .ok_or(PathStackError::MissingStorage)?
            else {
                return Err(PathStackError::MissingStorage);
            };
            data.entries.push(entry);
        }
        Ok(())
    }

    fn entries_mut<'a>(
        &self,
        resources: &'a mut ProgramResources<ProgramData>,
    ) -> Result<&'a mut Vec<PathEntry>, PathStackError> {
        let id = self.storage.ok_or(PathStackError::MissingLoop)?;
        let ProgramData::PathStack(data) = resources
            .get_mut(id)
            .ok_or(PathStackError::MissingStorage)?
        else {
            return Err(PathStackError::MissingStorage);
        };
        if data.entries.is_empty() {
            return Err(PathStackError::MissingLoop);
        }
        Ok(&mut data.entries)
    }

    /// Subroutine calls share the same stack (`$7F:956F`, `$7F:9592`). The
    /// caller owns the invocation's shared call-depth byte and decoded jump.
    pub fn push_call(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
        owner: ObjectId,
        continuation: PathCursor,
    ) -> Result<(), PathStackError> {
        self.push(resources, owner, PathEntry::CallReturn(continuation))
    }

    /// Normal return pop (`$7F:95B4..95C7`). Callback-root returns must be
    /// intercepted by their invocation owner before this operation.
    pub fn pop_call(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
    ) -> Result<PathCursor, PathStackError> {
        let entries = self.entries_mut(resources)?;
        let Some(PathEntry::CallReturn(continuation)) = entries.last().cloned() else {
            return Err(PathStackError::IncompleteLoop);
        };
        entries.pop();
        Ok(continuation)
    }

    /// Variable saves use the same one-entry allocation as a call, not a
    /// separate value stack (`$7F:A752..A78D`). Byte saves define only their
    /// low byte; retaining an invented high byte would leak machine scratch
    /// state into the native program model.
    pub fn save_byte(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
        owner: ObjectId,
        value: u8,
    ) -> Result<(), PathStackError> {
        self.push(resources, owner, PathEntry::SavedByte(value))
    }

    pub fn save_word(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
        owner: ObjectId,
        value: u16,
    ) -> Result<(), PathStackError> {
        self.push(resources, owner, PathEntry::SavedWord(value))
    }

    /// Byte restore also accepts the low byte of a saved word. It cannot
    /// reinterpret typed call/loop continuations as numeric source addresses.
    pub fn restore_byte(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
    ) -> Result<u8, PathStackError> {
        let entries = self.entries_mut(resources)?;
        let value = match entries.last() {
            Some(PathEntry::SavedByte(value)) => *value,
            Some(PathEntry::SavedWord(value)) => *value as u8,
            _ => return Err(PathStackError::IncompatibleSavedValue),
        };
        entries.pop();
        Ok(value)
    }

    /// A word restore requires a fully defined word. Widening an unmatched
    /// byte save depends on stale source-machine temporaries and is outside
    /// this typed contract; reject without consuming or fabricating a value.
    pub fn restore_word(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
    ) -> Result<u16, PathStackError> {
        let entries = self.entries_mut(resources)?;
        let Some(PathEntry::SavedWord(value)) = entries.last().cloned() else {
            return Err(PathStackError::IncompatibleSavedValue);
        };
        entries.pop();
        Ok(value)
    }

    /// `$7F:96AE` and `$7F:9708` share this decrement/branch operation.
    /// The two handlers differ only in whether their caller yields movement
    /// after a repeat. Completion pops both entries without freeing storage.
    pub fn next(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
    ) -> Result<LoopRepeat, PathStackError> {
        let entries = self.entries_mut(resources)?;
        let length = entries.len();
        if length < 2 {
            return Err(PathStackError::IncompleteLoop);
        }
        let PathEntry::Continuation(continuation) = entries[length - 2] else {
            return Err(PathStackError::IncompleteLoop);
        };
        let PathEntry::Counter(counter) = &mut entries[length - 1] else {
            return Err(PathStackError::IncompleteLoop);
        };
        if counter.repeat() {
            Ok(LoopRepeat::Repeat { continuation })
        } else {
            entries.truncate(length - 2);
            Ok(LoopRepeat::Complete)
        }
    }

    /// Explicit pair discard (`$7F:9735`, `$7F:9760`). Neither source handler
    /// inspects entry contents: a pair of call continuations is discarded as
    /// readily as a loop. The caller chooses jump versus ordinary advance.
    pub fn discard(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
    ) -> Result<(), PathStackError> {
        let entries = self.entries_mut(resources)?;
        if entries.len() < 2 {
            return Err(PathStackError::IncompleteLoop);
        }
        entries.truncate(entries.len() - 2);
        Ok(())
    }

    /// Clear after the containing actor's ownership chain has been released.
    pub fn clear_released(
        &mut self,
        resources: &ProgramResources<ProgramData>,
    ) -> Result<(), PathStackError> {
        if self.storage.is_some_and(|id| resources.get(id).is_some()) {
            return Err(PathStackError::StorageStillOwned);
        }
        self.storage = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program_resources::PROGRAM_CAPACITY;
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

    fn cursor(index: u16) -> PathCursor {
        PathCursor {
            path: PathId::from_catalog_index(0),
            command_index: index,
        }
    }

    #[test]
    fn saved_values_preserve_width_and_all_word_bits_without_machine_temporaries() {
        let owner = owner();
        let mut stack = PathStack::default();
        let mut resources = ProgramResources::default();
        for value in 0..=u16::MAX {
            stack.save_word(&mut resources, owner, value).unwrap();
            assert_eq!(stack.restore_word(&mut resources), Ok(value));
            stack.save_word(&mut resources, owner, value).unwrap();
            assert_eq!(stack.restore_byte(&mut resources), Ok(value as u8));
        }
        for value in 0..=u8::MAX {
            stack.save_byte(&mut resources, owner, value).unwrap();
            let before = (stack.clone(), resources.clone());
            assert_eq!(
                stack.restore_word(&mut resources),
                Err(PathStackError::IncompatibleSavedValue)
            );
            assert_eq!((&stack, &resources), (&before.0, &before.1));
            assert_eq!(stack.restore_byte(&mut resources), Ok(value));
        }
        assert_eq!(resources.available_capacity(), PROGRAM_CAPACITY - 38);
        assert_eq!(resources.owner_count(owner), 1);
        assert_eq!(
            stack.restore_byte(&mut resources),
            Err(PathStackError::MissingLoop)
        );
        resources.release_owner(owner);
        stack.clear_released(&resources).unwrap();
        assert_eq!(resources.available_capacity(), PROGRAM_CAPACITY);
    }

    #[test]
    fn saved_values_calls_and_loops_share_order_and_pair_discard() {
        let owner = owner();
        let mut stack = PathStack::default();
        let mut resources = ProgramResources::default();
        stack.begin(&mut resources, owner, cursor(1), 2).unwrap();
        stack.save_word(&mut resources, owner, 513).unwrap();
        stack.push_call(&mut resources, owner, cursor(2)).unwrap();
        stack.save_byte(&mut resources, owner, 255).unwrap();
        let before = (stack.clone(), resources.clone());
        assert_eq!(
            stack.next(&mut resources),
            Err(PathStackError::IncompleteLoop)
        );
        assert_eq!(
            stack.pop_call(&mut resources),
            Err(PathStackError::IncompleteLoop)
        );
        assert_eq!((&stack, &resources), (&before.0, &before.1));
        assert_eq!(stack.restore_byte(&mut resources), Ok(255));
        let before = (stack.clone(), resources.clone());
        assert_eq!(
            stack.restore_word(&mut resources),
            Err(PathStackError::IncompatibleSavedValue)
        );
        assert_eq!((&stack, &resources), (&before.0, &before.1));
        assert_eq!(stack.pop_call(&mut resources), Ok(cursor(2)));
        assert_eq!(stack.restore_word(&mut resources), Ok(513));
        assert_eq!(
            stack.next(&mut resources),
            Ok(LoopRepeat::Repeat {
                continuation: cursor(1)
            })
        );
        stack.save_byte(&mut resources, owner, 7).unwrap();
        stack.push_call(&mut resources, owner, cursor(3)).unwrap();
        stack.discard(&mut resources).unwrap();
        assert_eq!(stack.next(&mut resources), Ok(LoopRepeat::Complete));
    }

    #[test]
    fn value_saves_have_identical_eighth_entry_growth_and_failure_pressure() {
        let owner = owner();
        let mut stack = PathStack::default();
        let mut resources = ProgramResources::default();
        for value in 0..7 {
            stack.save_byte(&mut resources, owner, value).unwrap();
        }
        let remaining = resources.available_capacity();
        let occupied = resources
            .allocate_shared(
                remaining - 12,
                ProgramData::PathStack(PathEntries::default()),
            )
            .unwrap();
        let before = (stack.clone(), resources.clone());
        assert_eq!(
            stack.save_word(&mut resources, owner, 900),
            Err(PathStackError::Allocation(
                AllocationFailure::NoContiguousFit
            ))
        );
        assert_eq!((&stack, &resources), (&before.0, &before.1));
        resources.release_shared(occupied).unwrap();
        let before_growth = stack.storage;
        stack.save_word(&mut resources, owner, 900).unwrap();
        assert_ne!(stack.storage, before_growth);
        assert_eq!(resources.available_capacity(), PROGRAM_CAPACITY - 70);
        assert_eq!(stack.restore_word(&mut resources), Ok(900));
        for value in (0..7).rev() {
            assert_eq!(stack.restore_byte(&mut resources), Ok(value));
        }
        assert_eq!(resources.available_capacity(), PROGRAM_CAPACITY - 70);
    }

    #[test]
    fn nested_loops_decrement_before_repeat_and_retain_empty_storage() {
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let owner = owner();
        stack.begin(&mut resources, owner, cursor(1), 2).unwrap();
        stack.begin(&mut resources, owner, cursor(2), 1).unwrap();
        assert_eq!(stack.next(&mut resources), Ok(LoopRepeat::Complete));
        assert_eq!(
            stack.next(&mut resources),
            Ok(LoopRepeat::Repeat {
                continuation: cursor(1)
            })
        );
        assert_eq!(stack.next(&mut resources), Ok(LoopRepeat::Complete));
        assert_eq!(stack.next(&mut resources), Err(PathStackError::MissingLoop));
        assert_eq!(resources.available_capacity(), PROGRAM_CAPACITY - 38);
        assert_eq!(resources.owner_count(owner), 1);
        assert_eq!(
            stack.clear_released(&resources),
            Err(PathStackError::StorageStillOwned)
        );
        resources.release_owner(owner);
        stack.clear_released(&resources).unwrap();
        assert_eq!(resources.available_capacity(), PROGRAM_CAPACITY);
    }

    #[test]
    fn zero_count_wraps_and_explicit_exit_exposes_outer_loop() {
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let owner = owner();
        stack.begin(&mut resources, owner, cursor(1), 1).unwrap();
        stack.begin(&mut resources, owner, cursor(2), 0).unwrap();
        for _ in 0..2 {
            assert_eq!(
                stack.next(&mut resources),
                Ok(LoopRepeat::Repeat {
                    continuation: cursor(2)
                })
            );
        }
        stack.discard(&mut resources).unwrap();
        assert_eq!(stack.next(&mut resources), Ok(LoopRepeat::Complete));
        assert_eq!(
            stack.discard(&mut resources),
            Err(PathStackError::MissingLoop)
        );
    }

    #[test]
    fn pair_discard_does_not_require_a_loop_frame() {
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let owner = owner();
        stack.begin(&mut resources, owner, cursor(1), 1).unwrap();
        stack.push_call(&mut resources, owner, cursor(2)).unwrap();
        stack.push_call(&mut resources, owner, cursor(3)).unwrap();
        stack.discard(&mut resources).unwrap();
        assert_eq!(stack.next(&mut resources), Ok(LoopRepeat::Complete));
    }

    #[test]
    fn every_eighth_entry_reallocates_even_after_previous_larger_stack() {
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let owner = owner();
        for i in 0..8 {
            stack.begin(&mut resources, owner, cursor(i), 1).unwrap();
            assert_eq!(
                resources.available_capacity(),
                PROGRAM_CAPACITY
                    - if i < 3 {
                        38
                    } else if i < 7 {
                        70
                    } else {
                        102
                    }
            );
        }
        let larger = stack.storage;
        for _ in 0..8 {
            stack.discard(&mut resources).unwrap();
        }
        assert_eq!(stack.storage, larger);
        for i in 0..4 {
            stack.begin(&mut resources, owner, cursor(i), 1).unwrap();
        }
        assert_ne!(stack.storage, larger);
        assert_eq!(resources.available_capacity(), PROGRAM_CAPACITY - 70);
        assert_eq!(resources.owner_count(owner), 1);
    }

    #[test]
    fn growth_failure_keeps_old_storage_and_prior_continuation_push() {
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let owner = owner();
        for i in 0..3 {
            stack.begin(&mut resources, owner, cursor(i), 1).unwrap();
        }
        let old = stack.storage;
        // Retain a small free block so failure is ordinary first-fit failure,
        // not the separate source empty-free-list anomaly.
        let remaining = resources.available_capacity();
        let occupied = resources
            .allocate_shared(
                remaining - 12,
                ProgramData::PathStack(PathEntries::default()),
            )
            .unwrap();
        assert_eq!(
            stack.begin(&mut resources, owner, cursor(4), 1),
            Err(PathStackError::Allocation(
                AllocationFailure::NoContiguousFit
            ))
        );
        assert_eq!(stack.storage, old);
        assert_eq!(resources.owner_count(owner), 1);
        assert_eq!(
            stack.next(&mut resources),
            Err(PathStackError::IncompleteLoop)
        );
        resources.release_shared(occupied).unwrap();
        resources.release_owner(owner);
        stack.clear_released(&resources).unwrap();
        assert_eq!(resources.available_capacity(), PROGRAM_CAPACITY);
    }

    #[test]
    fn loops_and_subroutines_share_storage_without_even_depth_assumptions() {
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let owner = owner();
        stack.push_call(&mut resources, owner, cursor(1)).unwrap();
        stack.begin(&mut resources, owner, cursor(2), 2).unwrap();
        stack.push_call(&mut resources, owner, cursor(3)).unwrap();
        stack.begin(&mut resources, owner, cursor(4), 1).unwrap();
        assert_eq!(stack.next(&mut resources), Ok(LoopRepeat::Complete));
        assert_eq!(stack.pop_call(&mut resources), Ok(cursor(3)));
        assert_eq!(
            stack.next(&mut resources),
            Ok(LoopRepeat::Repeat {
                continuation: cursor(2)
            })
        );
        assert_eq!(
            stack.pop_call(&mut resources),
            Err(PathStackError::IncompleteLoop)
        );
        assert_eq!(stack.next(&mut resources), Ok(LoopRepeat::Complete));
        assert_eq!(
            stack.next(&mut resources),
            Err(PathStackError::IncompleteLoop)
        );
        assert_eq!(stack.pop_call(&mut resources), Ok(cursor(1)));
        assert_eq!(resources.available_capacity(), PROGRAM_CAPACITY - 38);
    }

    #[test]
    fn wrapping_entry_count_reports_corrupt_source_state() {
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let owner = owner();
        for i in 0..u16::from(u8::MAX) {
            stack.push_call(&mut resources, owner, cursor(i)).unwrap();
        }
        assert_eq!(
            stack.push_call(&mut resources, owner, cursor(0)),
            Err(PathStackError::EntryCountOverflow)
        );
        for i in (0..u16::from(u8::MAX)).rev() {
            assert_eq!(stack.pop_call(&mut resources), Ok(cursor(i)));
        }
    }
}
