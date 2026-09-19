//! Ordered actor-owned path triggers (`$7F:97A9..99A8`, `$7F:9AA8..9DDD`).
//!
//! A callback can edit its own trigger list during a pass. Storage changes
//! retain the active logical position, and additions do not extend the pass.

use super::path_control::TriggerPeriod;
use super::program_resources::{
    AllocationFailure, ProgramResourceId, ProgramResources, ReplacementError,
};
use super::program_state::ProgramData;
use super::{ObjectId, PathCursor};

const RECORD_COST: u16 = 4;
const COUNT_COST: u16 = 1;
const MAX_TRIGGERS: usize = (u8::MAX as usize - COUNT_COST as usize) / RECORD_COST as usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    Always,
    Periodic(TriggerPeriod),
    NewContact,
    PlayerContact,
    ConsumeHitEvent,
    Detached,
    ZeroHealth,
    PlayerCrossing,
    PlayerPartTarget,
    ControlledAuxFlagHigh,
    ControlledAuxFlagLow,
    TimerPenultimate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trigger {
    pub path: PathCursor,
    pub kind: TriggerKind,
    /// Zero is indefinite. Nonzero timers decrement before predicate testing;
    /// reaching zero removes the trigger without running its callback.
    pub timer: u8,
}

impl Trigger {
    /// Timed registration stores the authored duration plus one, as a byte.
    pub fn timed(path: PathCursor, kind: TriggerKind, duration: u8) -> Self {
        Self {
            path,
            kind,
            timer: duration.wrapping_add(1),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TriggerRecords {
    entries: Vec<Trigger>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerError {
    MissingStorage,
    InvalidPassPosition,
    ReentrantPass,
    NoActivePass,
    EntryCostOverflow,
    StorageStillOwned,
    Allocation(AllocationFailure),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TriggerList {
    storage: Option<ProgramResourceId>,
}

impl TriggerList {
    pub fn entries<'a>(
        &self,
        resources: &'a ProgramResources<ProgramData>,
        owner: ObjectId,
    ) -> Result<&'a [Trigger], TriggerError> {
        let Some(id) = self.storage else {
            return Ok(&[]);
        };
        match resources.get_owned(owner, id) {
            Some(ProgramData::PathTriggers(records)) => Ok(&records.entries),
            _ => Err(TriggerError::MissingStorage),
        }
    }

    /// Duplicates are deliberately retained. Every addition to nonempty
    /// storage allocates a new exact-size record before releasing the old one.
    pub fn add(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
        owner: ObjectId,
        trigger: Trigger,
    ) -> Result<(), TriggerError> {
        let mut entries = self.entries(resources, owner)?.to_vec();
        if entries.len() == MAX_TRIGGERS {
            // The source's byte-sized multiplication wraps at 64 records and
            // the subsequent copy corrupts the undersized allocation.
            return Err(TriggerError::EntryCostOverflow);
        }
        entries.push(trigger);
        let cost = COUNT_COST + RECORD_COST * entries.len() as u16;
        let data = ProgramData::PathTriggers(TriggerRecords { entries });
        self.storage = Some(match self.storage {
            None => resources
                .allocate_owned(owner, cost, data)
                .map_err(|e| TriggerError::Allocation(e.reason))?,
            Some(old) => resources
                .replace_owned(owner, old, cost, data)
                .map_err(|e| match e {
                    ReplacementError::MissingOwnedResource(_) => TriggerError::MissingStorage,
                    ReplacementError::Allocation(e) => TriggerError::Allocation(e.reason),
                })?,
        });
        Ok(())
    }

    /// Cancellation removes only the first matching path, not every duplicate.
    pub fn cancel(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
        owner: ObjectId,
        runner: &mut TriggerRunner,
        path: PathCursor,
    ) -> Result<bool, TriggerError> {
        let Some(index) = self
            .entries(resources, owner)?
            .iter()
            .position(|t| t.path == path)
        else {
            return Ok(false);
        };
        self.remove_at(resources, owner, runner, index)?;
        Ok(true)
    }

    fn remove_at(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
        owner: ObjectId,
        runner: &mut TriggerRunner,
        index: usize,
    ) -> Result<(), TriggerError> {
        let length = self.entries(resources, owner)?.len();
        if index >= length {
            return Err(TriggerError::InvalidPassPosition);
        }
        let id = self.storage.ok_or(TriggerError::MissingStorage)?;
        if length == 1 {
            resources
                .release_owned(owner, id)
                .ok_or(TriggerError::MissingStorage)?;
            self.storage = None;
            // The singleton source branch does not alter pass position/count.
            return Ok(());
        }
        if let Some(pass) = &mut runner.active {
            if pass.owner == owner {
                let removed = index as i16;
                let current = pass.position;
                if removed <= current {
                    pass.position -= 1;
                }
                if removed >= current {
                    // Includes removal of the current entry. The common
                    // advance still decrements again, which can leave the
                    // final original entry unvisited in this pass.
                    pass.remaining = pass.remaining.wrapping_sub(1);
                    if pass.remaining == 0 {
                        pass.remaining = 1;
                    }
                }
            }
        }
        let Some(ProgramData::PathTriggers(records)) = resources.get_owned_mut(owner, id) else {
            return Err(TriggerError::MissingStorage);
        };
        records.entries.remove(index);
        Ok(())
    }

    pub fn clear(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
        owner: ObjectId,
        runner: &mut TriggerRunner,
    ) -> Result<(), TriggerError> {
        if let Some(id) = self.storage {
            self.entries(resources, owner)?;
            resources
                .release_owned(owner, id)
                .ok_or(TriggerError::MissingStorage)?;
            self.storage = None;
        }
        if let Some(pass) = &mut runner.active {
            if pass.owner == owner {
                pass.remaining = 1;
            }
        }
        Ok(())
    }

    pub fn clear_released(
        &mut self,
        resources: &ProgramResources<ProgramData>,
    ) -> Result<(), TriggerError> {
        if self.storage.is_some_and(|id| resources.get(id).is_some()) {
            return Err(TriggerError::StorageStillOwned);
        }
        self.storage = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::program_resources::PROGRAM_CAPACITY;
    use super::super::program_state::PathStack;
    use super::*;
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

    fn trigger(index: u16, timer: u8) -> Trigger {
        Trigger {
            path: PathCursor {
                path: PathId::from_catalog_index(0),
                command_index: index,
            },
            kind: TriggerKind::Always,
            timer,
        }
    }

    fn candidate(
        runner: &mut TriggerRunner,
        list: &mut TriggerList,
        resources: &mut ProgramResources<ProgramData>,
    ) -> u16 {
        let TriggerStep::Candidate(t) = runner.step(list, resources).unwrap() else {
            panic!("expected callback candidate");
        };
        t.path.command_index
    }

    #[test]
    fn duplicates_cancel_first_and_retain_capacity_until_last_removed() {
        let owner = owner();
        let mut resources = ProgramResources::default();
        let mut list = TriggerList::default();
        let mut runner = TriggerRunner::default();
        list.add(&mut resources, owner, trigger(1, 0)).unwrap();
        list.add(&mut resources, owner, trigger(1, 5)).unwrap();
        list.add(&mut resources, owner, trigger(2, 0)).unwrap();
        let capacity = resources.available_capacity();
        assert_eq!(capacity, PROGRAM_CAPACITY - 18);
        assert!(list
            .cancel(&mut resources, owner, &mut runner, trigger(1, 0).path)
            .unwrap());
        assert_eq!(
            list.entries(&resources, owner).unwrap(),
            &[trigger(1, 5), trigger(2, 0)]
        );
        assert_eq!(resources.available_capacity(), capacity);
        assert!(!list
            .cancel(&mut resources, owner, &mut runner, trigger(3, 0).path)
            .unwrap());
        list.cancel(&mut resources, owner, &mut runner, trigger(1, 0).path)
            .unwrap();
        assert_eq!(resources.available_capacity(), capacity);
        list.cancel(&mut resources, owner, &mut runner, trigger(2, 0).path)
            .unwrap();
        assert_eq!(resources.available_capacity(), PROGRAM_CAPACITY);
    }

    #[test]
    fn timed_registration_wraps_and_expiry_removes_the_current_duplicate() {
        for duration in 0..=u8::MAX {
            assert_eq!(
                Trigger::timed(trigger(0, 0).path, TriggerKind::Always, duration).timer,
                duration.wrapping_add(1)
            );
        }
        let owner = owner();
        let mut resources = ProgramResources::default();
        let mut list = TriggerList::default();
        let mut runner = TriggerRunner::default();
        for t in [trigger(1, 0), trigger(1, 1), trigger(3, 0), trigger(4, 0)] {
            list.add(&mut resources, owner, t).unwrap();
        }
        runner.begin(&list, &resources, owner).unwrap();
        assert_eq!(candidate(&mut runner, &mut list, &mut resources), 1);
        assert_eq!(
            runner.step(&mut list, &mut resources),
            Ok(TriggerStep::Expired)
        );
        assert_eq!(candidate(&mut runner, &mut list, &mut resources), 3);
        // Current removal decrements the pass budget in addition to advance.
        assert_eq!(
            runner.step(&mut list, &mut resources),
            Ok(TriggerStep::Complete)
        );
        assert_eq!(
            list.entries(&resources, owner).unwrap(),
            &[trigger(1, 0), trigger(3, 0), trigger(4, 0)]
        );
    }

    #[test]
    fn cancellation_before_current_preserves_next_and_additions_do_not_extend_pass() {
        let owner = owner();
        let mut resources = ProgramResources::default();
        let mut list = TriggerList::default();
        let mut runner = TriggerRunner::default();
        for i in 1..=3 {
            list.add(&mut resources, owner, trigger(i, 0)).unwrap();
        }
        runner.begin(&list, &resources, owner).unwrap();
        assert_eq!(
            runner.begin(&list, &resources, owner),
            Err(TriggerError::ReentrantPass)
        );
        assert_eq!(candidate(&mut runner, &mut list, &mut resources), 1);
        assert_eq!(candidate(&mut runner, &mut list, &mut resources), 2);
        list.cancel(&mut resources, owner, &mut runner, trigger(1, 0).path)
            .unwrap();
        list.add(&mut resources, owner, trigger(4, 0)).unwrap();
        assert_eq!(candidate(&mut runner, &mut list, &mut resources), 3);
        assert_eq!(
            runner.step(&mut list, &mut resources),
            Ok(TriggerStep::Complete)
        );
    }

    #[test]
    fn current_and_future_cancellation_apply_source_pass_budget_rules() {
        for removed in 1..=3 {
            let owner = owner();
            let mut resources = ProgramResources::default();
            let mut list = TriggerList::default();
            let mut runner = TriggerRunner::default();
            for i in 1..=3 {
                list.add(&mut resources, owner, trigger(i, 0)).unwrap();
            }
            runner.begin(&list, &resources, owner).unwrap();
            assert_eq!(candidate(&mut runner, &mut list, &mut resources), 1);
            list.cancel(&mut resources, owner, &mut runner, trigger(removed, 0).path)
                .unwrap();
            assert_eq!(
                candidate(&mut runner, &mut list, &mut resources),
                if removed == 2 { 3 } else { 2 }
            );
            assert_eq!(
                runner.step(&mut list, &mut resources),
                Ok(TriggerStep::Complete)
            );
        }
    }

    #[test]
    fn clearing_during_callback_ends_pass_even_after_new_registration() {
        let owner = owner();
        let mut resources = ProgramResources::default();
        let mut list = TriggerList::default();
        let mut runner = TriggerRunner::default();
        for i in 1..=3 {
            list.add(&mut resources, owner, trigger(i, 0)).unwrap();
        }
        runner.begin(&list, &resources, owner).unwrap();
        candidate(&mut runner, &mut list, &mut resources);
        list.clear(&mut resources, owner, &mut runner).unwrap();
        list.add(&mut resources, owner, trigger(4, 0)).unwrap();
        assert_eq!(
            runner.step(&mut list, &mut resources),
            Ok(TriggerStep::Complete)
        );
        runner.begin(&list, &resources, owner).unwrap();
        assert_eq!(candidate(&mut runner, &mut list, &mut resources), 4);
    }

    #[test]
    fn timer_observation_survives_indefinite_entries_and_pass_boundaries() {
        let owner = owner();
        let mut resources = ProgramResources::default();
        let mut list = TriggerList::default();
        let mut runner = TriggerRunner::default();
        list.add(&mut resources, owner, trigger(1, 2)).unwrap();
        list.add(&mut resources, owner, trigger(2, 0)).unwrap();
        runner.begin(&list, &resources, owner).unwrap();
        assert!(!runner.penultimate_timer());
        candidate(&mut runner, &mut list, &mut resources);
        assert!(runner.penultimate_timer());
        candidate(&mut runner, &mut list, &mut resources);
        assert!(runner.penultimate_timer());
        assert_eq!(
            runner.step(&mut list, &mut resources),
            Ok(TriggerStep::Complete)
        );
        list.cancel(&mut resources, owner, &mut runner, trigger(1, 0).path)
            .unwrap();
        runner.begin(&list, &resources, owner).unwrap();
        candidate(&mut runner, &mut list, &mut resources);
        assert!(runner.penultimate_timer());
    }

    #[test]
    fn trigger_and_path_storage_compete_and_owned_release_invalidates_both() {
        let owner = owner();
        let mut resources = ProgramResources::default();
        let mut list = TriggerList::default();
        let mut stack = PathStack::default();
        stack
            .push_call(&mut resources, owner, trigger(0, 0).path)
            .unwrap();
        for i in 0..MAX_TRIGGERS {
            list.add(&mut resources, owner, trigger(i as u16, 0))
                .unwrap();
        }
        assert_eq!(resources.owner_count(owner), 2);
        assert_eq!(resources.available_capacity(), PROGRAM_CAPACITY - 38 - 258);
        assert_eq!(
            list.add(&mut resources, owner, trigger(64, 0)),
            Err(TriggerError::EntryCostOverflow)
        );
        assert_eq!(
            list.clear_released(&resources),
            Err(TriggerError::StorageStillOwned)
        );
        resources.release_owner(owner);
        list.clear_released(&resources).unwrap();
        stack.clear_released(&resources).unwrap();
        assert_eq!(resources.available_capacity(), PROGRAM_CAPACITY);
    }

    #[test]
    fn failed_replacement_preserves_old_trigger_and_its_allocation() {
        let owner = owner();
        let mut resources = ProgramResources::default();
        let mut list = TriggerList::default();
        list.add(&mut resources, owner, trigger(1, 0)).unwrap();
        resources
            .allocate_shared(
                PROGRAM_CAPACITY - 10 - 2,
                ProgramData::PathTriggers(TriggerRecords::default()),
            )
            .unwrap();
        assert_eq!(resources.available_capacity(), 0);
        assert!(matches!(
            list.add(&mut resources, owner, trigger(2, 0)),
            Err(TriggerError::Allocation(_))
        ));
        assert_eq!(list.entries(&resources, owner).unwrap(), &[trigger(1, 0)]);
        assert_eq!(resources.owner_count(owner), 1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActivePass {
    owner: ObjectId,
    position: i16,
    remaining: u8,
    needs_advance: bool,
}

/// Shared across actors, including the last nonzero-timer observation. The
/// source does not reset that observation for an indefinite trigger or a new
/// pass, so TimerPenultimate can observe an earlier trigger's timer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TriggerRunner {
    active: Option<ActivePass>,
    last_timer: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerStep {
    Candidate(Trigger),
    Expired,
    Complete,
}

impl TriggerRunner {
    pub fn begin(
        &mut self,
        list: &TriggerList,
        resources: &ProgramResources<ProgramData>,
        owner: ObjectId,
    ) -> Result<(), TriggerError> {
        if self.active.is_some() {
            return Err(TriggerError::ReentrantPass);
        }
        self.active = Some(ActivePass {
            owner,
            position: 0,
            remaining: list.entries(resources, owner)?.len() as u8,
            needs_advance: false,
        });
        Ok(())
    }

    pub fn penultimate_timer(&self) -> bool {
        self.last_timer == 1
    }

    /// Resume after the previous callback (or skipped/expired candidate).
    /// Predicates and path execution occur between calls, allowing list edits.
    pub fn step(
        &mut self,
        list: &mut TriggerList,
        resources: &mut ProgramResources<ProgramData>,
    ) -> Result<TriggerStep, TriggerError> {
        let pass = self.active.as_mut().ok_or(TriggerError::NoActivePass)?;
        if pass.needs_advance {
            pass.position += 1;
            pass.remaining = pass.remaining.wrapping_sub(1);
        }
        if pass.remaining == 0 {
            self.active = None;
            return Ok(TriggerStep::Complete);
        }
        pass.needs_advance = true;
        let owner = pass.owner;
        let index =
            usize::try_from(pass.position).map_err(|_| TriggerError::InvalidPassPosition)?;
        let id = list.storage.ok_or(TriggerError::MissingStorage)?;
        let Some(ProgramData::PathTriggers(records)) = resources.get_owned_mut(owner, id) else {
            return Err(TriggerError::MissingStorage);
        };
        let trigger = records
            .entries
            .get_mut(index)
            .ok_or(TriggerError::InvalidPassPosition)?;
        if trigger.timer != 0 {
            trigger.timer -= 1;
            self.last_timer = trigger.timer;
            if trigger.timer == 0 {
                list.remove_at(resources, owner, self, index)?;
                return Ok(TriggerStep::Expired);
            }
        }
        Ok(TriggerStep::Candidate(*trigger))
    }
}
