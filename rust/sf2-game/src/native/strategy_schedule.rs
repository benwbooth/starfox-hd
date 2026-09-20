//! Shared actor-pass scheduling (`$7F:34E7..367F`).
//!
//! A strategy epoch has one clock increment and one live-list traversal. The
//! first portion overlaps rendering; the second resumes at the saved actor
//! without incrementing the clock. Render readiness is supplied by the real
//! work owner, not inferred from a presentation frame or per-actor credits.

use super::{ObjectId, ObjectStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyAction {
    Skip,
    CommonDestruction,
    HitResponse,
    Assigned,
}

/// Domain flags sampled at strategy entry. "First visit" is the source
/// initializer's one-use zero-health exemption, not an object age counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategyInputs {
    pub health: u8,
    pub suspended: bool,
    pub excluded_actor: bool,
    pub first_visit: bool,
    pub hit_pending: bool,
    pub run_when_paused: bool,
    pub has_assigned_strategy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategyDecision {
    pub action: StrategyAction,
    pub clear_first_visit: bool,
    pub service_positional_sound: bool,
}

/// Source strategy-selection precedence, including the post-strategy sound
/// service. The assigned handler may be skipped by pause; destruction and
/// hit routing have their own earlier branches and cannot share that check.
pub fn select_strategy(input: StrategyInputs, paused: bool) -> StrategyDecision {
    if input.suspended {
        return StrategyDecision {
            action: StrategyAction::Skip,
            clear_first_visit: false,
            service_positional_sound: false,
        };
    }
    if input.excluded_actor {
        return StrategyDecision {
            action: StrategyAction::Skip,
            clear_first_visit: false,
            service_positional_sound: true,
        };
    }
    let ordinary_enabled = !paused || input.run_when_paused;
    if input.health == 0 && !input.first_visit && ordinary_enabled {
        return StrategyDecision {
            action: StrategyAction::CommonDestruction,
            clear_first_visit: false,
            service_positional_sound: true,
        };
    }
    let action = if input.hit_pending {
        StrategyAction::HitResponse
    } else if input.has_assigned_strategy && ordinary_enabled {
        StrategyAction::Assigned
    } else {
        StrategyAction::Skip
    };
    StrategyDecision {
        action,
        clear_first_visit: true,
        // With no assigned strategy, the source branches past the sound
        // service. Hit response is selected before that null-handler test.
        service_positional_sound: input.hit_pending || input.has_assigned_strategy,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyCompletion {
    Keep,
    /// Separate from the deferred remove-after-tick flag. The pass saves the
    /// actor's successor after its strategy, then calls the retirement owner.
    RetireNow,
}

/// Native world services needed by the pass. Retirement must perform the
/// complete object lifecycle, including attachment/contact cleanup. Strategy
/// code must request RetireNow instead of freeing its own identity early.
pub trait StrategyHost {
    type Error;

    fn objects(&self) -> &ObjectStore;
    /// Additional host-owned suspension. The schedule itself always honors
    /// the actor's authored strategy_suspended flag.
    fn strategy_suspended(&self, object: ObjectId) -> bool;
    fn run_strategy(
        &mut self,
        object: ObjectId,
        strategy_clock: u16,
    ) -> Result<StrategyCompletion, Self::Error>;
    fn retire_object(&mut self, object: ObjectId) -> Result<(), Self::Error>;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum PassPhase {
    #[default]
    Idle,
    Overlapping,
    Remainder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError<E> {
    EpochAlreadyActive,
    WrongPass,
    MissingActor(ObjectId),
    Host(E),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StrategySchedule {
    clock: u16,
    phase: PassPhase,
    next: Option<ObjectId>,
}

impl StrategySchedule {
    pub const fn clock(&self) -> u16 {
        self.clock
    }

    pub const fn pending_actor(&self) -> Option<ObjectId> {
        self.next
    }

    /// Called once after the owning frame service has started render work.
    /// An empty list still advances the shared word clock.
    pub fn begin<E>(&mut self, objects: &ObjectStore) -> Result<(), ScheduleError<E>> {
        if self.phase != PassPhase::Idle {
            return Err(ScheduleError::EpochAlreadyActive);
        }
        self.clock = self.clock.wrapping_add(1);
        self.next = objects.active_ids().first().copied();
        self.phase = PassPhase::Overlapping;
        Ok(())
    }

    /// Render readiness is tested before each actor, including suspended
    /// actors. Saving the cursor never consumes or partially executes one.
    pub fn run_overlapping<H: StrategyHost>(
        &mut self,
        host: &mut H,
        mut render_work_pending: impl FnMut() -> bool,
    ) -> Result<(), ScheduleError<H::Error>> {
        if self.phase != PassPhase::Overlapping {
            return Err(ScheduleError::WrongPass);
        }
        while self.next.is_some() && render_work_pending() {
            self.visit(host)?;
        }
        self.phase = PassPhase::Remainder;
        Ok(())
    }

    /// The caller invokes this after the intervening render/scene services.
    /// The source resumes at the saved identity, not at a recomputed head or
    /// a snapshot of actor IDs, so children added after the cursor are live.
    pub fn run_remainder<H: StrategyHost>(
        &mut self,
        host: &mut H,
    ) -> Result<(), ScheduleError<H::Error>> {
        if self.phase != PassPhase::Remainder {
            return Err(ScheduleError::WrongPass);
        }
        while self.next.is_some() {
            self.visit(host)?;
        }
        self.phase = PassPhase::Idle;
        Ok(())
    }

    fn visit<H: StrategyHost>(&mut self, host: &mut H) -> Result<(), ScheduleError<H::Error>> {
        let actor = self.next.expect("nonempty strategy cursor");
        let authored_suspension = host
            .objects()
            .get(actor)
            .ok_or(ScheduleError::MissingActor(actor))?
            .base
            .flags
            .strategy_suspended;
        let completion = if authored_suspension || host.strategy_suspended(actor) {
            StrategyCompletion::Keep
        } else {
            host.run_strategy(actor, self.clock)
                .map_err(ScheduleError::Host)?
        };
        let next = host
            .objects()
            .get(actor)
            .ok_or(ScheduleError::MissingActor(actor))?
            .base
            .next;
        if completion == StrategyCompletion::RetireNow {
            host.retire_object(actor).map_err(ScheduleError::Host)?;
        }
        self.next = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind, ShapeId};

    const ORDINARY: StrategyInputs = StrategyInputs {
        health: 10,
        suspended: false,
        excluded_actor: false,
        first_visit: false,
        hit_pending: false,
        run_when_paused: false,
        has_assigned_strategy: true,
    };

    #[test]
    fn suspension_and_exclusion_have_distinct_post_service_behavior() {
        let suspended = select_strategy(
            StrategyInputs {
                suspended: true,
                ..ORDINARY
            },
            false,
        );
        let excluded = select_strategy(
            StrategyInputs {
                excluded_actor: true,
                ..ORDINARY
            },
            false,
        );
        assert_eq!(suspended.action, StrategyAction::Skip);
        assert_eq!(excluded.action, StrategyAction::Skip);
        assert!(!suspended.clear_first_visit && !excluded.clear_first_visit);
        assert!(!suspended.service_positional_sound);
        assert!(excluded.service_positional_sound);
    }

    #[test]
    fn death_precedes_hit_but_first_visit_protects_zero_health_once() {
        let dead = StrategyInputs {
            health: 0,
            hit_pending: true,
            ..ORDINARY
        };
        let decision = select_strategy(dead, false);
        assert_eq!(decision.action, StrategyAction::CommonDestruction);
        assert!(!decision.clear_first_visit);
        let first = select_strategy(
            StrategyInputs {
                first_visit: true,
                ..dead
            },
            false,
        );
        assert_eq!(first.action, StrategyAction::HitResponse);
        assert!(first.clear_first_visit);
    }

    #[test]
    fn pause_suppresses_ordinary_work_but_not_hit_response_or_post_service() {
        let paused = select_strategy(ORDINARY, true);
        assert_eq!(paused.action, StrategyAction::Skip);
        assert!(paused.clear_first_visit && paused.service_positional_sound);
        let dead = StrategyInputs {
            health: 0,
            ..ORDINARY
        };
        assert_eq!(select_strategy(dead, true).action, StrategyAction::Skip);
        assert_eq!(
            select_strategy(
                StrategyInputs {
                    hit_pending: true,
                    ..dead
                },
                true
            )
            .action,
            StrategyAction::HitResponse
        );
        assert_eq!(
            select_strategy(
                StrategyInputs {
                    run_when_paused: true,
                    ..dead
                },
                true
            )
            .action,
            StrategyAction::CommonDestruction
        );
        let absent = select_strategy(
            StrategyInputs {
                has_assigned_strategy: false,
                ..ORDINARY
            },
            false,
        );
        assert_eq!(absent.action, StrategyAction::Skip);
        assert!(absent.clear_first_visit && !absent.service_positional_sound);
    }

    #[derive(Default)]
    struct TestWorld {
        objects: ObjectStore,
        visits: Vec<(ObjectId, u16)>,
        spawn_from: Option<ObjectId>,
        child: Option<ObjectId>,
        retire: Option<ObjectId>,
        suspended: Option<ObjectId>,
        suspend_during_visit: Option<ObjectId>,
    }

    fn actor() -> Object {
        Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath)
    }

    impl StrategyHost for TestWorld {
        type Error = &'static str;
        fn objects(&self) -> &ObjectStore {
            &self.objects
        }
        fn strategy_suspended(&self, object: ObjectId) -> bool {
            self.suspended == Some(object)
        }
        fn run_strategy(
            &mut self,
            object: ObjectId,
            clock: u16,
        ) -> Result<StrategyCompletion, Self::Error> {
            self.visits.push((object, clock));
            if self.suspend_during_visit == Some(object) {
                self.objects
                    .get_mut(object)
                    .unwrap()
                    .base
                    .flags
                    .strategy_suspended = true;
            }
            if self.spawn_from == Some(object) {
                self.spawn_from = None;
                self.child = Some(
                    self.objects
                        .allocate_after(Some(object), actor())
                        .ok_or("pool full")?,
                );
            }
            Ok(if self.retire == Some(object) {
                StrategyCompletion::RetireNow
            } else {
                StrategyCompletion::Keep
            })
        }
        fn retire_object(&mut self, object: ObjectId) -> Result<(), Self::Error> {
            self.objects
                .remove(object)
                .ok_or("missing retiring actor")?;
            Ok(())
        }
    }

    #[test]
    fn split_pass_keeps_shared_clock_and_reads_new_children_before_retirement() {
        let mut world = TestWorld::default();
        let tail = world.objects.allocate(actor()).unwrap();
        let parent = world.objects.allocate(actor()).unwrap();
        world.spawn_from = Some(parent);
        world.retire = Some(parent);
        let mut schedule = StrategySchedule::default();
        schedule.begin::<&str>(&world.objects).unwrap();
        let mut pending_checks = 0;
        schedule
            .run_overlapping(&mut world, || {
                pending_checks += 1;
                pending_checks == 1
            })
            .unwrap();
        let child = world.child.unwrap();
        assert_eq!(schedule.pending_actor(), Some(child));
        assert_eq!(world.visits, [(parent, 1)]);
        assert!(world.objects.get(parent).is_none());
        // An unrelated head inserted between passes is not retroactively
        // visited in the epoch already in progress.
        let later_head = world.objects.allocate(actor()).unwrap();
        schedule.run_remainder(&mut world).unwrap();
        assert_eq!(world.visits, [(parent, 1), (child, 1), (tail, 1)]);
        schedule.begin::<&str>(&world.objects).unwrap();
        schedule.run_overlapping(&mut world, || false).unwrap();
        schedule.run_remainder(&mut world).unwrap();
        assert_eq!(
            &world.visits[3..],
            &[(later_head, 2), (child, 2), (tail, 2)]
        );
    }

    #[test]
    fn every_split_boundary_has_identical_actor_order_and_shared_clock() {
        for boundary in 0..=6 {
            let mut world = TestWorld::default();
            for _ in 0..6 {
                world.objects.allocate(actor()).unwrap();
            }
            let order = world.objects.active_ids().to_vec();
            world.suspended = Some(order[2]);
            world
                .objects
                .get_mut(order[4])
                .unwrap()
                .base
                .flags
                .strategy_suspended = true;
            let mut schedule = StrategySchedule::default();
            schedule.begin::<&str>(&world.objects).unwrap();
            let mut checked = 0;
            schedule
                .run_overlapping(&mut world, || {
                    checked += 1;
                    checked <= boundary
                })
                .unwrap();
            schedule.run_remainder(&mut world).unwrap();
            let expected: Vec<_> = order
                .into_iter()
                .filter(|id| {
                    Some(*id) != world.suspended
                        && !world
                            .objects
                            .get(*id)
                            .unwrap()
                            .base
                            .flags
                            .strategy_suspended
                })
                .map(|id| (id, 1))
                .collect();
            assert_eq!(world.visits, expected);
        }
    }

    #[test]
    fn newly_suspended_actor_finishes_current_visit_and_keeps_live_successors() {
        for boundary in 0..=3 {
            let mut world = TestWorld::default();
            let tail = world.objects.allocate(actor()).unwrap();
            let owner = world.objects.allocate(actor()).unwrap();
            world.suspend_during_visit = Some(owner);
            world.spawn_from = Some(owner);
            let mut schedule = StrategySchedule::default();
            for epoch in 1..=3 {
                schedule.begin::<&str>(&world.objects).unwrap();
                let mut visits = 0;
                schedule
                    .run_overlapping(&mut world, || {
                        visits += 1;
                        visits <= boundary
                    })
                    .unwrap();
                schedule.run_remainder(&mut world).unwrap();
                assert_eq!(schedule.clock(), epoch);
            }
            let child = world.child.unwrap();
            assert_eq!(
                world.visits,
                [
                    (owner, 1),
                    (child, 1),
                    (tail, 1),
                    (child, 2),
                    (tail, 2),
                    (child, 3),
                    (tail, 3),
                ]
            );
            assert_eq!(world.objects.active_ids(), &[owner, child, tail]);
            assert!(
                world
                    .objects
                    .get(owner)
                    .unwrap()
                    .base
                    .flags
                    .strategy_suspended
            );
        }
    }

    #[test]
    fn empty_epochs_wrap_the_word_clock_and_reject_invalid_pass_order() {
        let mut world = TestWorld::default();
        let mut schedule = StrategySchedule {
            clock: u16::MAX,
            ..StrategySchedule::default()
        };
        assert_eq!(
            schedule.run_remainder(&mut world),
            Err(ScheduleError::WrongPass)
        );
        schedule.begin::<&str>(&world.objects).unwrap();
        assert_eq!(schedule.clock(), 0);
        assert_eq!(
            schedule.begin::<&str>(&world.objects),
            Err(ScheduleError::EpochAlreadyActive)
        );
        schedule
            .run_overlapping(&mut world, || panic!("empty list has no actor work"))
            .unwrap();
        schedule.run_remainder(&mut world).unwrap();
        assert!(world.visits.is_empty());
    }
}
