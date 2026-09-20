//! Trigger predicates (`$7F:9B0F..9D66`). Inputs are fresh world observations
//! for ONE candidate: earlier callbacks can change every later predicate.
//! Outputs explicitly request changes to the shared selected-player context.

use super::path_control::{PlayerCrossing, PlayerTarget};
use super::path_triggers::{TriggerKind, TriggerRunner};
use super::ObjectId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerPartTarget {
    /// Player auxiliary state high nibble equals the authored part-target mode.
    pub part_target_mode: bool,
    pub actor: Option<ObjectId>,
    pub part: u8,
    /// Source player flag 24 bit 02.
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ControlledAuxFlags {
    /// Source controlled actor auxiliary flags bit 10.
    pub high: bool,
    /// Source controlled actor auxiliary flags bit 08.
    pub low: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerInputs {
    pub owner: ObjectId,
    pub part: u8,
    pub strategy_tick: u8,
    /// New-contact latch, not generic collided/pending-hit state (22 bit 02).
    pub new_contact: bool,
    /// Source attribution flags 26 bits 02 and 04.
    pub contacted_players: [bool; 2],
    pub attached: bool,
    pub health: u8,
    pub player_parts: [Option<PlayerPartTarget>; 2],
    /// Fresh source-exact forward-plane projections; absent players are None.
    pub player_projections: [Option<i16>; 2],
    pub controlled_aux: ControlledAuxFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerActorState {
    /// Consumed only by the matching predicate (source flag 23 bit 08).
    pub hit_event_pending: bool,
    pub crossing: PlayerCrossing,
    /// Source actor flag 24 bit 80; separate from shared selection context.
    pub selected_player: PlayerTarget,
}

impl Default for TriggerActorState {
    fn default() -> Self {
        Self {
            hit_event_pending: false,
            crossing: PlayerCrossing::default(),
            selected_player: PlayerTarget::Primary,
        }
    }
}

#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerDecision {
    Skip,
    Run,
    /// Apply this to the world's selected player before entering the callback.
    SelectAndRun(PlayerTarget),
}

fn part_target(inputs: &TriggerInputs) -> Option<PlayerTarget> {
    let eligible =
        |player: &PlayerPartTarget| player.part_target_mode && player.actor == Some(inputs.owner);
    if let Some(player) = inputs.player_parts[0] {
        if eligible(&player) && player.part == inputs.part && player.enabled {
            return Some(PlayerTarget::Primary);
        }
    }
    if let Some(player) = inputs.player_parts[1] {
        if eligible(&player) {
            // Actual asymmetric branches at 9BF0 and 9BFA: secondary mismatch
            // selects secondary without checking enabled; secondary match
            // plus enabled selects PRIMARY, not secondary.
            if player.part != inputs.part {
                return Some(PlayerTarget::Secondary);
            }
            if player.enabled {
                return Some(PlayerTarget::Primary);
            }
        }
    }
    None
}

pub fn evaluate(
    kind: TriggerKind,
    inputs: &TriggerInputs,
    state: &mut TriggerActorState,
    runner: &TriggerRunner,
) -> TriggerDecision {
    let selection = match kind {
        TriggerKind::PlayerContact => {
            if inputs.contacted_players[0] {
                Some(PlayerTarget::Primary)
            } else if inputs.contacted_players[1] {
                Some(PlayerTarget::Secondary)
            } else {
                None
            }
        }
        TriggerKind::PlayerCrossing => state.crossing.sample(inputs.player_projections),
        TriggerKind::PlayerPartTarget => part_target(inputs),
        _ => {
            let run = match kind {
                TriggerKind::Always => true,
                TriggerKind::Periodic(period) => period.due(inputs.strategy_tick),
                TriggerKind::NewContact => inputs.new_contact,
                TriggerKind::ConsumeHitEvent => std::mem::take(&mut state.hit_event_pending),
                TriggerKind::Detached => !inputs.attached,
                TriggerKind::ZeroHealth => inputs.health == 0,
                TriggerKind::ControlledAuxFlagHigh => inputs.controlled_aux.high,
                TriggerKind::ControlledAuxFlagLow => inputs.controlled_aux.low,
                TriggerKind::TimerPenultimate => runner.penultimate_timer(),
                TriggerKind::PlayerContact
                | TriggerKind::PlayerCrossing
                | TriggerKind::PlayerPartTarget => {
                    unreachable!("selection predicates handled above")
                }
            };
            return if run {
                TriggerDecision::Run
            } else {
                TriggerDecision::Skip
            };
        }
    };
    match selection {
        Some(player) => {
            state.selected_player = player;
            TriggerDecision::SelectAndRun(player)
        }
        None => TriggerDecision::Skip,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_control::TriggerPeriod;
    use crate::{Behavior, Object, ObjectKind, ObjectStore, ShapeId};

    fn inputs() -> TriggerInputs {
        let owner = ObjectStore::new()
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Effect,
            ))
            .unwrap();
        TriggerInputs {
            owner,
            part: 3,
            strategy_tick: 0,
            new_contact: false,
            contacted_players: [false; 2],
            attached: true,
            health: 1,
            player_parts: [None; 2],
            player_projections: [None; 2],
            controlled_aux: ControlledAuxFlags::default(),
        }
    }

    #[test]
    fn periodic_predicates_cover_each_clock_byte_without_spawn_age() {
        let mut inputs = inputs();
        let mut state = TriggerActorState::default();
        let runner = TriggerRunner::default();
        let periods = [
            TriggerPeriod::Two,
            TriggerPeriod::Four,
            TriggerPeriod::Eight,
            TriggerPeriod::Sixteen,
            TriggerPeriod::ThirtyTwo,
            TriggerPeriod::SixtyFour,
            TriggerPeriod::OneTwentyEight,
        ];
        for (index, period) in periods.into_iter().enumerate() {
            let divisor = 1_u16 << (index + 1);
            for tick in 0..=u8::MAX {
                inputs.strategy_tick = tick;
                assert_eq!(
                    evaluate(TriggerKind::Periodic(period), &inputs, &mut state, &runner),
                    if u16::from(tick) % divisor == 0 {
                        TriggerDecision::Run
                    } else {
                        TriggerDecision::Skip
                    }
                );
            }
        }
    }

    #[test]
    fn simple_predicates_use_distinct_flags_and_only_hit_event_is_consumed() {
        let mut inputs = inputs();
        let mut state = TriggerActorState::default();
        let runner = TriggerRunner::default();
        assert_eq!(
            evaluate(TriggerKind::Always, &inputs, &mut state, &runner),
            TriggerDecision::Run
        );
        for kind in [
            TriggerKind::NewContact,
            TriggerKind::ConsumeHitEvent,
            TriggerKind::Detached,
            TriggerKind::ZeroHealth,
            TriggerKind::ControlledAuxFlagHigh,
            TriggerKind::ControlledAuxFlagLow,
            TriggerKind::TimerPenultimate,
        ] {
            assert_eq!(
                evaluate(kind, &inputs, &mut state, &runner),
                TriggerDecision::Skip
            );
        }
        inputs.new_contact = true;
        inputs.attached = false;
        inputs.health = 0;
        inputs.controlled_aux = ControlledAuxFlags {
            high: true,
            low: true,
        };
        state.hit_event_pending = true;
        for kind in [
            TriggerKind::NewContact,
            TriggerKind::ConsumeHitEvent,
            TriggerKind::Detached,
            TriggerKind::ZeroHealth,
            TriggerKind::ControlledAuxFlagHigh,
            TriggerKind::ControlledAuxFlagLow,
        ] {
            assert_eq!(
                evaluate(kind, &inputs, &mut state, &runner),
                TriggerDecision::Run
            );
        }
        assert_eq!(
            evaluate(TriggerKind::ConsumeHitEvent, &inputs, &mut state, &runner),
            TriggerDecision::Skip
        );
        assert_eq!(
            evaluate(TriggerKind::NewContact, &inputs, &mut state, &runner),
            TriggerDecision::Run
        );
    }

    #[test]
    fn player_contact_selection_prioritizes_primary_and_is_not_consumed() {
        let mut inputs = inputs();
        let mut state = TriggerActorState::default();
        let runner = TriggerRunner::default();
        for primary in [false, true] {
            for secondary in [false, true] {
                inputs.contacted_players = [primary, secondary];
                let expected = if primary {
                    TriggerDecision::SelectAndRun(PlayerTarget::Primary)
                } else if secondary {
                    TriggerDecision::SelectAndRun(PlayerTarget::Secondary)
                } else {
                    TriggerDecision::Skip
                };
                assert_eq!(
                    evaluate(TriggerKind::PlayerContact, &inputs, &mut state, &runner),
                    expected
                );
                assert_eq!(
                    evaluate(TriggerKind::PlayerContact, &inputs, &mut state, &runner),
                    expected
                );
            }
        }
    }

    #[test]
    fn part_target_preserves_the_secondary_players_asymmetric_branches() {
        let mut inputs = inputs();
        let mut state = TriggerActorState::default();
        let runner = TriggerRunner::default();
        for player_index in 0..2 {
            for mode in [false, true] {
                for target_matches in [false, true] {
                    for part_matches in [false, true] {
                        for enabled in [false, true] {
                            inputs.player_parts = [None; 2];
                            inputs.player_parts[player_index] = Some(PlayerPartTarget {
                                part_target_mode: mode,
                                actor: target_matches.then_some(inputs.owner),
                                part: if part_matches {
                                    inputs.part
                                } else {
                                    inputs.part + 1
                                },
                                enabled,
                            });
                            let expected = if !mode || !target_matches {
                                TriggerDecision::Skip
                            } else if player_index == 1 && !part_matches {
                                TriggerDecision::SelectAndRun(PlayerTarget::Secondary)
                            } else if part_matches && enabled {
                                TriggerDecision::SelectAndRun(PlayerTarget::Primary)
                            } else {
                                TriggerDecision::Skip
                            };
                            assert_eq!(
                                evaluate(
                                    TriggerKind::PlayerPartTarget,
                                    &inputs,
                                    &mut state,
                                    &runner
                                ),
                                expected
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn crossing_updates_selection_only_when_the_live_projection_changes_sign() {
        let mut inputs = inputs();
        let mut state = TriggerActorState::default();
        let runner = TriggerRunner::default();
        inputs.player_projections = [Some(1), Some(1)];
        assert_eq!(
            evaluate(TriggerKind::PlayerCrossing, &inputs, &mut state, &runner),
            TriggerDecision::Skip
        );
        inputs.player_projections[1] = Some(-1);
        assert_eq!(
            evaluate(TriggerKind::PlayerCrossing, &inputs, &mut state, &runner),
            TriggerDecision::SelectAndRun(PlayerTarget::Secondary)
        );
        assert_eq!(state.selected_player, PlayerTarget::Secondary);
        assert_eq!(
            evaluate(TriggerKind::PlayerCrossing, &inputs, &mut state, &runner),
            TriggerDecision::Skip
        );
        assert_eq!(state.selected_player, PlayerTarget::Secondary);
        inputs.player_projections[0] = Some(-1);
        assert_eq!(
            evaluate(TriggerKind::PlayerCrossing, &inputs, &mut state, &runner),
            TriggerDecision::SelectAndRun(PlayerTarget::Primary)
        );
    }

    #[test]
    fn callback_batch_composes_timers_consumed_events_redirects_and_parent_stack() {
        use crate::path_calls::{PathCalls, PathReturn, RedirectEffects};
        use crate::path_triggers::{Trigger, TriggerList, TriggerStep};
        use crate::program_resources::ProgramResources;
        use crate::program_state::{LoopRepeat, PathStack};
        use crate::{PathCursor, PathId};

        let inputs = inputs();
        let cursor = |command_index| PathCursor {
            path: PathId::from_catalog_index(0),
            command_index,
        };
        let mut resources = ProgramResources::default();
        let mut stack = PathStack::default();
        let mut list = TriggerList::default();
        let mut runner = TriggerRunner::default();
        let mut calls = PathCalls::default();
        let mut state = TriggerActorState {
            hit_event_pending: true,
            ..Default::default()
        };
        stack
            .begin(&mut resources, inputs.owner, cursor(80), 2)
            .unwrap();
        for (index, kind, timer) in [
            (1, TriggerKind::ConsumeHitEvent, 0),
            (2, TriggerKind::ConsumeHitEvent, 0),
            (3, TriggerKind::TimerPenultimate, 2),
        ] {
            list.add(
                &mut resources,
                inputs.owner,
                Trigger {
                    path: cursor(index),
                    kind,
                    timer,
                },
            )
            .unwrap();
        }
        runner.begin(&list, &resources, inputs.owner).unwrap();
        calls
            .begin_callbacks(inputs.owner, cursor(81), true)
            .unwrap();
        let mut invoked = Vec::new();
        loop {
            match runner.step(&mut list, &mut resources).unwrap() {
                TriggerStep::Candidate(trigger) => {
                    let decision = evaluate(trigger.kind, &inputs, &mut state, &runner);
                    if decision == TriggerDecision::Skip {
                        continue;
                    }
                    assert_eq!(decision, TriggerDecision::Run);
                    calls.enter_callback().unwrap();
                    invoked.push(trigger.path.command_index);
                    if trigger.path == cursor(1) {
                        assert_eq!(
                            calls.force_after_callbacks(cursor(90)),
                            RedirectEffects::RestartPathStrategyAndClearWaitAndRepeat
                        );
                    }
                    assert_eq!(
                        calls.return_from(&mut stack, &mut resources),
                        Ok(PathReturn::CallbackComplete)
                    );
                }
                TriggerStep::Expired => panic!("none of these timers expires on the first pass"),
                TriggerStep::Complete => break,
            }
        }
        assert_eq!(invoked, [1, 3]);
        assert!(!state.hit_event_pending);
        assert_eq!(
            calls.finish_callbacks(&mut stack, &mut resources),
            Ok(Some(cursor(90)))
        );
        assert_eq!(
            stack.next(&mut resources),
            Ok(LoopRepeat::Repeat {
                continuation: cursor(80)
            })
        );
    }
}
