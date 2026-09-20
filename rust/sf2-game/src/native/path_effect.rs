//! Short-lived impact actor (`$09:AFE4..B04F`). The path installs this
//! strategy; initialization occurs on its next strategy visit, not inline.

use super::{Behavior, Object};

const INITIAL_DELAY: u8 = 7;
const INITIAL_CHILD_PARAMETER: u8 = 5;
const INITIAL_REPEAT_PARAMETER: u8 = 30;
const INITIAL_LIFETIME: u8 = 1;
const RETIRE_LIFETIME: i8 = -10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactBurstPhase {
    Initialize,
    Decay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrongImpactBehavior;

/// Performs one strategy visit only. This strategy neither runs ordinary
/// path movement nor advances registered callbacks after the handoff visit.
pub fn step(actor: &mut Object) -> Result<(), WrongImpactBehavior> {
    match actor.base.behavior {
        Behavior::ImpactBurst(ImpactBurstPhase::Initialize) => {
            actor.base.flags.reclaim_on_pool_pressure = true;
            actor.base.wait_timer = INITIAL_DELAY;
            actor.base.child_number = INITIAL_CHILD_PARAMETER;
            actor.extension.path_state.repeat_counter = INITIAL_REPEAT_PARAMETER;
            actor.base.flags.casts_shadow = false;
            actor.base.contacts.latch_new_contact = false;
            actor.base.contacts.run_when_paused = true;
            actor.base.flags.exclude_from_shape_footprint_search = true;
            actor.base.flags.maximum_draw_distance = true;
            actor.base.behavior = Behavior::ImpactBurst(ImpactBurstPhase::Decay);
            actor.base.target_speed = INITIAL_LIFETIME;
        }
        Behavior::ImpactBurst(ImpactBurstPhase::Decay) => {}
        _ => return Err(WrongImpactBehavior),
    }
    actor.base.target_speed = actor.base.target_speed.wrapping_sub(1);
    let remaining = actor.base.target_speed as i8;
    if remaining >= 0 {
        return Ok(());
    }
    if remaining == RETIRE_LIFETIME {
        actor.base.flags.remove_after_tick = true;
    } else {
        actor.base.flags.reclaim_on_pool_pressure = true;
        actor.base.wait_timer = 0;
        actor.base.child_number = 0;
        actor.extension.path_state.repeat_counter = 0;
        actor.base.flags.casts_shadow = false;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Angle, ObjectKind, ShapeId, Vector3};

    #[test]
    fn impact_initialization_and_all_eleven_visits_preserve_pose_and_unrelated_fields() {
        let mut actual = Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::ImpactBurst(ImpactBurstPhase::Initialize),
        );
        actual.base.position = Vector3 {
            x: 32767,
            y: -20,
            z: -32768,
        };
        actual.base.velocity = Vector3 {
            x: 300,
            y: -100,
            z: 45,
        };
        actual.base.pitch = Angle::from_units(77);
        actual.base.yaw = Angle::from_units(88);
        actual.base.roll = Angle::from_units(99);
        actual.base.hit_points = 10;
        actual.base.attack_power = 10;
        actual.base.target_speed = 193;
        actual.base.contacts.new_contact_latched = true;
        actual.base.flags.collision_disabled = true;
        let mut expected = actual.clone();
        expected.base.flags.reclaim_on_pool_pressure = true;
        expected.base.wait_timer = 7;
        expected.base.child_number = 5;
        expected.extension.path_state.repeat_counter = 30;
        expected.base.flags.casts_shadow = false;
        expected.base.contacts.latch_new_contact = false;
        expected.base.contacts.run_when_paused = true;
        expected.base.flags.exclude_from_shape_footprint_search = true;
        expected.base.flags.maximum_draw_distance = true;
        expected.base.behavior = Behavior::ImpactBurst(ImpactBurstPhase::Decay);
        for visit in 0..=10u8 {
            expected.base.target_speed = visit.wrapping_neg();
            if visit > 0 {
                expected.base.wait_timer = 0;
                expected.base.child_number = 0;
                expected.extension.path_state.repeat_counter = 0;
            }
            expected.base.flags.remove_after_tick = visit == 10;
            step(&mut actual).unwrap();
            assert_eq!(actual, expected, "visit {visit}");
        }
    }

    #[test]
    fn decay_uses_wrapping_signed_byte_and_retirement_preserves_parameters_on_its_own_branch() {
        for value in 0..=u8::MAX {
            let mut actual = Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::ImpactBurst(ImpactBurstPhase::Decay),
            );
            actual.base.target_speed = value;
            actual.base.wait_timer = 61;
            actual.base.child_number = 47;
            actual.extension.path_state.repeat_counter = 91;
            actual.base.flags.casts_shadow = true;
            let mut expected = actual.clone();
            expected.base.target_speed = value.wrapping_sub(1);
            if expected.base.target_speed == 246 {
                expected.base.flags.remove_after_tick = true;
            } else if expected.base.target_speed >= 128 {
                expected.base.flags.reclaim_on_pool_pressure = true;
                expected.base.wait_timer = 0;
                expected.base.child_number = 0;
                expected.extension.path_state.repeat_counter = 0;
                expected.base.flags.casts_shadow = false;
            }
            step(&mut actual).unwrap();
            assert_eq!(actual, expected);
        }
        let mut actual = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
        let before = actual.clone();
        assert_eq!(step(&mut actual), Err(WrongImpactBehavior));
        assert_eq!(actual, before);
    }
}
