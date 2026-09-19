//! Typed program-resource ownership and source allocation pressure.
//!
//! `$7F:1737..19D5` uses first-fit free-list order, high-end splitting, and
//! adjacent coalescing. Only capacity partitions are retained here: payloads
//! are Rust values, never source bytes, and handles are not source addresses.

use super::ObjectId;

/// Source capacity after its free-list head (`$7F:174A`).
pub const PROGRAM_CAPACITY: u16 = 18_430;
const WORD_COST: u16 = 2;
const MIN_BLOCK_COST: u16 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramResourceId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationFailure {
    ZeroRoundedSize,
    NoContiguousFit,
    /// The source returns its nonzero requested cost when the free list is
    /// empty (`$7F:18D1..18D4`), not a valid resource. Surface that invalid
    /// source state rather than manufacturing an address-shaped handle.
    EmptyFreeList {
        returned_cost: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationError<T> {
    pub reason: AllocationFailure,
    pub value: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementError<T> {
    MissingOwnedResource(T),
    Allocation(AllocationError<T>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Resource<T> {
    owner: Option<ObjectId>,
    value: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Partition<T> {
    id: ProgramResourceId,
    cost: u16,
    resource: Option<Resource<T>>,
}

/// All actor-owned and shared program allocations compete for this one pool.
/// Partition order records adjacency; free order records first-fit priority.
/// Neither order can be replaced by aggregate available capacity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramResources<T> {
    partitions: Vec<Partition<T>>,
    free_order: Vec<ProgramResourceId>,
    next_id: u64,
}

impl<T> Default for ProgramResources<T> {
    fn default() -> Self {
        Self {
            partitions: vec![Partition {
                id: ProgramResourceId(0),
                cost: PROGRAM_CAPACITY,
                resource: None,
            }],
            free_order: vec![ProgramResourceId(0)],
            next_id: 1,
        }
    }
}

/// Word-rounded requested cost plus the allocation header. Source arithmetic
/// rejects zero after rounding, but wraps before applying its minimum size.
fn block_cost(payload_cost: u16) -> Option<u16> {
    let rounded = payload_cost.wrapping_add(1) & !1;
    (rounded != 0).then(|| rounded.wrapping_add(WORD_COST).max(MIN_BLOCK_COST))
}

impl<T> ProgramResources<T> {
    pub fn available_capacity(&self) -> u16 {
        self.partitions
            .iter()
            .filter(|partition| partition.resource.is_none())
            .map(|partition| partition.cost)
            .sum()
    }

    pub fn owner_count(&self, owner: ObjectId) -> usize {
        self.partitions
            .iter()
            .filter(|partition| {
                partition
                    .resource
                    .as_ref()
                    .is_some_and(|r| r.owner == Some(owner))
            })
            .count()
    }

    pub fn get(&self, id: ProgramResourceId) -> Option<&T> {
        self.partitions
            .iter()
            .find(|p| p.id == id)?
            .resource
            .as_ref()
            .map(|r| &r.value)
    }

    pub fn get_mut(&mut self, id: ProgramResourceId) -> Option<&mut T> {
        self.partitions
            .iter_mut()
            .find(|p| p.id == id)?
            .resource
            .as_mut()
            .map(|r| &mut r.value)
    }

    pub fn get_owned(&self, owner: ObjectId, id: ProgramResourceId) -> Option<&T> {
        let resource = self
            .partitions
            .iter()
            .find(|p| p.id == id)?
            .resource
            .as_ref()?;
        (resource.owner == Some(owner)).then_some(&resource.value)
    }

    pub fn get_owned_mut(&mut self, owner: ObjectId, id: ProgramResourceId) -> Option<&mut T> {
        let resource = self
            .partitions
            .iter_mut()
            .find(|p| p.id == id)?
            .resource
            .as_mut()?;
        (resource.owner == Some(owner)).then_some(&mut resource.value)
    }

    /// Allocation failure returns the unconsumed typed value. `payload_cost`
    /// is the authored record cost, not `size_of::<T>()`.
    pub fn allocate_shared(
        &mut self,
        payload_cost: u16,
        value: T,
    ) -> Result<ProgramResourceId, AllocationError<T>> {
        self.allocate(payload_cost, None, value)
    }

    /// Actor ownership adds one link word before the common allocator rounds
    /// the request (`$7F:194E`). New resources precede older owned resources.
    pub fn allocate_owned(
        &mut self,
        owner: ObjectId,
        payload_cost: u16,
        value: T,
    ) -> Result<ProgramResourceId, AllocationError<T>> {
        self.allocate(payload_cost.wrapping_add(WORD_COST), Some(owner), value)
    }

    fn allocate(
        &mut self,
        payload_cost: u16,
        owner: Option<ObjectId>,
        value: T,
    ) -> Result<ProgramResourceId, AllocationError<T>> {
        let Some(cost) = block_cost(payload_cost) else {
            return Err(AllocationError {
                reason: AllocationFailure::ZeroRoundedSize,
                value,
            });
        };
        if self.free_order.is_empty() {
            return Err(AllocationError {
                reason: AllocationFailure::EmptyFreeList {
                    returned_cost: cost,
                },
                value,
            });
        }
        let Some((rank, index)) = self.free_order.iter().enumerate().find_map(|(rank, id)| {
            self.partitions
                .iter()
                .position(|p| p.id == *id && p.cost >= cost)
                .map(|index| (rank, index))
        }) else {
            return Err(AllocationError {
                reason: AllocationFailure::NoContiguousFit,
                value,
            });
        };
        let id = ProgramResourceId(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("program resource identity exhausted");
        let remainder = self.partitions[index].cost - cost;
        let resource = Some(Resource { owner, value });
        if remainder > MIN_BLOCK_COST {
            self.partitions[index].cost = remainder;
            self.partitions
                .insert(index + 1, Partition { id, cost, resource });
        } else {
            // Equality also consumes the whole block (`$7F:18F2..18F7`).
            self.free_order.remove(rank);
            self.partitions[index].id = id;
            self.partitions[index].resource = resource;
        }
        Ok(id)
    }

    /// Remove one shared allocation. Owned allocations require their owner.
    pub fn release_shared(&mut self, id: ProgramResourceId) -> Option<T> {
        self.release(id, None)
    }

    pub fn release_owned(&mut self, owner: ObjectId, id: ProgramResourceId) -> Option<T> {
        self.release(id, Some(owner))
    }

    /// Resize/replace an owned record (`$7F:1B00..1B62`). Both allocations
    /// remain charged until replacement succeeds; the old resource cannot
    /// fund its own growth. The domain caller constructs `replacement` by
    /// preserving the meaningful fields of its typed record, not copying
    /// storage bytes or treating allocation padding as gameplay data.
    pub fn replace_owned(
        &mut self,
        owner: ObjectId,
        old: ProgramResourceId,
        payload_cost: u16,
        replacement: T,
    ) -> Result<ProgramResourceId, ReplacementError<T>> {
        if !self
            .partitions
            .iter()
            .any(|p| p.id == old && p.resource.as_ref().is_some_and(|r| r.owner == Some(owner)))
        {
            return Err(ReplacementError::MissingOwnedResource(replacement));
        }
        let new = self
            .allocate_owned(owner, payload_cost, replacement)
            .map_err(ReplacementError::Allocation)?;
        self.release_owned(owner, old)
            .expect("replacement owner was validated");
        Ok(new)
    }

    /// Retire the ownership chain newest-first (`$7F:19B3..19C5`). The caller
    /// subsequently clears its auxiliary table, loop, and callback handles.
    pub fn release_owner(&mut self, owner: ObjectId) -> Vec<T> {
        let mut ids: Vec<_> = self
            .partitions
            .iter()
            .filter_map(|p| {
                p.resource
                    .as_ref()
                    .filter(|r| r.owner == Some(owner))
                    .map(|_| p.id)
            })
            .collect();
        ids.sort_unstable_by_key(|id| std::cmp::Reverse(id.0));
        ids.into_iter()
            .map(|id| {
                self.release_owned(owner, id)
                    .expect("owned resource remains allocated")
            })
            .collect()
    }

    fn release(&mut self, id: ProgramResourceId, owner: Option<ObjectId>) -> Option<T> {
        let index = self.partitions.iter().position(|p| p.id == id)?;
        if self.partitions[index].resource.as_ref()?.owner != owner {
            return None;
        }
        let value = self.partitions[index].resource.take()?.value;
        let left_free = index > 0 && self.partitions[index - 1].resource.is_none();
        let right_free = self
            .partitions
            .get(index + 1)
            .is_some_and(|p| p.resource.is_none());
        if left_free {
            // A lower adjacent free block keeps its place in the free list,
            // even when the higher neighbor was encountered first.
            let released = self.partitions.remove(index);
            self.partitions[index - 1].cost += released.cost;
            if right_free {
                let right = self.partitions.remove(index);
                self.partitions[index - 1].cost += right.cost;
                self.free_order.retain(|id| *id != right.id);
            }
        } else if right_free {
            let released = self.partitions.remove(index);
            self.partitions[index].cost += released.cost;
        } else {
            self.free_order.insert(0, id);
        }
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind, ObjectStore, ShapeId};

    fn owners() -> [ObjectId; 2] {
        let mut objects = ObjectStore::new();
        std::array::from_fn(|_| {
            objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Effect,
                ))
                .unwrap()
        })
    }

    #[test]
    fn source_rounding_minimum_and_wrapping_costs() {
        assert_eq!(block_cost(0), None);
        assert_eq!(block_cost(u16::MAX), None);
        assert_eq!(block_cost(u16::MAX - 1), Some(MIN_BLOCK_COST));
        for request in 1..=4 {
            assert_eq!(block_cost(request), Some(MIN_BLOCK_COST));
        }
        assert_eq!(block_cost(5), Some(8));
        assert_eq!(block_cost(6), Some(8));
        assert_eq!(block_cost(7), Some(10));
    }

    #[test]
    fn whole_block_threshold_and_failed_allocation_are_exact() {
        for remainder in [0, 2, 4, 6, 8] {
            let mut pool = ProgramResources::default();
            let id = pool
                .allocate_shared(PROGRAM_CAPACITY - WORD_COST - remainder, 7)
                .unwrap();
            assert_eq!(
                pool.available_capacity(),
                if remainder > MIN_BLOCK_COST {
                    remainder
                } else {
                    0
                }
            );
            let error = pool.allocate_shared(PROGRAM_CAPACITY, 8).unwrap_err();
            assert_eq!(error.value, 8);
            assert_eq!(
                error.reason,
                if remainder > MIN_BLOCK_COST {
                    AllocationFailure::NoContiguousFit
                } else {
                    AllocationFailure::EmptyFreeList {
                        returned_cost: PROGRAM_CAPACITY + WORD_COST,
                    }
                }
            );
            assert_eq!(pool.release_shared(id), Some(7));
            assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY);
            assert_eq!(pool.release_shared(id), None);
        }
    }

    #[test]
    fn high_end_splits_and_freed_blocks_precede_older_free_space() {
        let mut pool = ProgramResources::default();
        let high = pool.allocate_shared(8, "high").unwrap();
        let barrier = pool.allocate_shared(8, "barrier").unwrap();
        pool.release_shared(high).unwrap();
        let replacement = pool.allocate_shared(4, "replacement").unwrap();
        assert_eq!(pool.partitions.last().unwrap().id, replacement);
        assert_eq!(pool.partitions.last().unwrap().cost, 10);
        assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY - 20);
        assert_ne!(high, replacement);
        assert_eq!(pool.get(high), None);
        assert_eq!(pool.get(barrier), Some(&"barrier"));
    }

    #[test]
    fn fragment_total_does_not_make_a_large_request_succeed() {
        let mut pool = ProgramResources::default();
        let high = pool.allocate_shared(6_000, 1).unwrap();
        let middle = pool.allocate_shared(6_000, 2).unwrap();
        let low = pool.allocate_shared(6_000, 3).unwrap();
        pool.release_shared(high).unwrap();
        pool.release_shared(low).unwrap();
        assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY - 6_002);
        assert_eq!(
            pool.allocate_shared(7_000, 4),
            Err(AllocationError {
                reason: AllocationFailure::NoContiguousFit,
                value: 4
            })
        );
        pool.release_shared(middle).unwrap();
        assert_eq!(pool.partitions.len(), 1);
        assert_eq!(pool.free_order.len(), 1);
        assert!(pool.allocate_shared(7_000, 4).is_ok());
    }

    #[test]
    fn every_release_order_coalesces_without_losing_capacity() {
        for a in 0..4 {
            for b in 0..4 {
                for c in 0..4 {
                    for d in 0..4 {
                        let order = [a, b, c, d];
                        if (0..4).any(|i| order.iter().filter(|&&item| item == i).count() != 1) {
                            continue;
                        }
                        let mut pool = ProgramResources::default();
                        let ids: [_; 4] =
                            std::array::from_fn(|i| pool.allocate_shared(100, i).unwrap());
                        for (n, i) in order.into_iter().enumerate() {
                            assert_eq!(pool.release_shared(ids[i]), Some(i));
                            assert_eq!(
                                pool.available_capacity(),
                                PROGRAM_CAPACITY - (3 - n) as u16 * 102
                            );
                            assert_eq!(
                                pool.free_order.len(),
                                pool.partitions
                                    .iter()
                                    .filter(|p| p.resource.is_none())
                                    .count()
                            );
                            assert!(pool
                                .partitions
                                .windows(2)
                                .all(|p| p[0].resource.is_some() || p[1].resource.is_some()));
                        }
                        assert_eq!(pool.partitions.len(), 1);
                    }
                }
            }
        }
    }

    #[test]
    fn actor_chain_is_newest_first_and_cannot_release_another_owner() {
        let [owner, other] = owners();
        let mut pool = ProgramResources::default();
        let first = pool.allocate_owned(owner, 4, 1).unwrap();
        let shared = pool.allocate_shared(4, 2).unwrap();
        let second = pool.allocate_owned(owner, 4, 3).unwrap();
        let foreign = pool.allocate_owned(other, 4, 4).unwrap();
        assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY - 30);
        assert_eq!(pool.owner_count(owner), 2);
        assert_eq!(pool.release_shared(first), None);
        assert_eq!(pool.release_owned(other, second), None);
        *pool.get_mut(second).unwrap() = 5;
        assert_eq!(pool.release_owner(owner), [5, 1]);
        assert_eq!(pool.owner_count(owner), 0);
        assert_eq!(pool.get(shared), Some(&2));
        assert_eq!(pool.get(foreign), Some(&4));
        assert_eq!(pool.release_owner(owner), []);
        pool.release_shared(shared).unwrap();
        pool.release_owner(other);
        assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY);
    }

    #[test]
    fn replacement_allocates_before_release_and_becomes_chain_head() {
        let [owner, other] = owners();
        let mut pool = ProgramResources::default();
        let first = pool.allocate_owned(owner, 100, vec![1]).unwrap();
        let later = pool.allocate_owned(owner, 100, vec![2]).unwrap();
        assert_eq!(
            pool.replace_owned(other, first, 200, vec![3]),
            Err(ReplacementError::MissingOwnedResource(vec![3]))
        );
        let new = pool.replace_owned(owner, first, 200, vec![1, 3]).unwrap();
        assert_ne!(new, first);
        assert_eq!(pool.get(first), None);
        assert_eq!(pool.get(later), Some(&vec![2]));
        assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY - 308);
        assert_eq!(pool.release_owner(owner), [vec![1, 3], vec![2]]);
        assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY);

        let old = pool.allocate_owned(owner, 10_000, vec![4]).unwrap();
        let before = pool.available_capacity();
        assert_eq!(
            pool.replace_owned(owner, old, 12_000, vec![4, 5]),
            Err(ReplacementError::Allocation(AllocationError {
                reason: AllocationFailure::NoContiguousFit,
                value: vec![4, 5],
            }))
        );
        assert_eq!(pool.available_capacity(), before);
        assert_eq!(pool.owner_count(owner), 1);
        assert_eq!(pool.get(old), Some(&vec![4]));
    }
}
