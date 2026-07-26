//! Native path behavior for Mirage Dragon's head.
//!
//! The encounter begins on the retail root path, follows the recovered
//! cooperative action schedule, then enters a hold path with one fixed
//! velocity and an independent pitch-turn trigger.

#[cfg(test)]
use super::MirageDragonHeadState;
use super::{flight_velocity, Angle, MirageDragonHeadPhase, Object, ObjectActivity, Vector3};

const INITIAL_POSITION: Vector3 = Vector3 {
    x: 24,
    y: 1_628,
    z: 20,
};
const INITIAL_PITCH: Angle = Angle::from_units(254);
const INITIAL_YAW: Angle = Angle::from_units(219);
const INITIAL_ROLL: Angle = Angle::ZERO;
const INITIAL_SPEED: u8 = 10;
const INITIAL_VELOCITY: Vector3 = Vector3 { x: 24, y: 0, z: 20 };

const DEPARTURE_SPEED: u8 = 40;
const DEPARTURE_POSITION_SCALE: i16 = 4;
const DEPARTURE_PITCH_STEP: i8 = 30;

pub(super) fn activate(object: &mut Object) {
    let ObjectActivity::MirageDragonHead(mut state) = object.extension.activity else {
        return;
    };
    if state.phase != MirageDragonHeadPhase::AwaitingEntrance {
        return;
    }
    object.base.position = INITIAL_POSITION;
    object.base.pitch = INITIAL_PITCH;
    object.base.yaw = INITIAL_YAW;
    object.base.roll = INITIAL_ROLL;
    object.base.speed = INITIAL_SPEED;
    object.base.velocity = INITIAL_VELOCITY;
    object.base.flags.active = true;
    object.base.flags.visible = true;
    object.base.flags.collision_disabled = false;
    state.phase = MirageDragonHeadPhase::Following;
    object.extension.activity = ObjectActivity::MirageDragonHead(state);
}

pub(super) fn apply_follow_pose(
    object: &mut Object,
    position: Vector3,
    pitch: Angle,
    yaw: Angle,
    velocity: Vector3,
) {
    let ObjectActivity::MirageDragonHead(state) = object.extension.activity else {
        return;
    };
    if state.phase != MirageDragonHeadPhase::Following {
        return;
    }
    object.base.position = position;
    object.base.pitch = pitch;
    object.base.yaw = yaw;
    object.base.velocity = velocity;
}

pub(super) fn begin_departure(
    object: &mut Object,
    target: Vector3,
    movement_updates: u8,
    pitch_updates: u8,
) {
    let ObjectActivity::MirageDragonHead(mut state) = object.extension.activity else {
        return;
    };
    if state.phase != MirageDragonHeadPhase::Following {
        return;
    }

    let delta_x = target.x.wrapping_sub(object.base.position.x);
    let delta_y = target.y.wrapping_sub(object.base.position.y);
    let delta_z = target.z.wrapping_sub(object.base.position.z);
    object.base.pitch = Angle::from_units(sf_core::aim_angle::sf2_pitch_to_target(
        delta_y,
        sf_core::aim_angle::sf2_xz_angle_distance(delta_x, delta_z),
    ));
    object.base.yaw = Angle::from_units(sf_core::aim_angle::sf2_yaw_to_target(delta_x, delta_z));
    object.base.speed = DEPARTURE_SPEED;
    object.base.velocity = flight_velocity(
        object.base.pitch,
        object.base.yaw,
        object.base.speed,
        DEPARTURE_POSITION_SCALE,
    );
    state.phase = MirageDragonHeadPhase::Departing;
    object.extension.activity = ObjectActivity::MirageDragonHead(state);
    advance_departure(object, movement_updates, pitch_updates);
}

pub(super) fn advance_departure(object: &mut Object, movement_updates: u8, pitch_updates: u8) {
    let ObjectActivity::MirageDragonHead(mut state) = object.extension.activity else {
        return;
    };
    if state.phase != MirageDragonHeadPhase::Departing {
        return;
    }
    for _ in 0..movement_updates {
        object.base.position.x = object.base.position.x.wrapping_add(object.base.velocity.x);
        object.base.position.y = object.base.position.y.wrapping_add(object.base.velocity.y);
        object.base.position.z = object.base.position.z.wrapping_add(object.base.velocity.z);
    }
    for _ in 0..pitch_updates {
        object.base.pitch = object.base.pitch.wrapping_add(DEPARTURE_PITCH_STEP);
    }
    state.departure_motion_updates = state
        .departure_motion_updates
        .saturating_add(u16::from(movement_updates));
    state.departure_turn_updates = state
        .departure_turn_updates
        .saturating_add(u16::from(pitch_updates));
    object.extension.activity = ObjectActivity::MirageDragonHead(state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, ObjectKind, ShapeId};

    const PRE_DEPARTURE_POSITION: Vector3 = Vector3 {
        x: 672,
        y: 1_044,
        z: 1_576,
    };
    const PRE_DEPARTURE_PITCH: Angle = Angle::from_units(72);
    const PRE_DEPARTURE_YAW: Angle = Angle::from_units(116);
    const RETAIL_VIEW_POSITION: Vector3 = Vector3 {
        x: 9_960,
        y: -2_861,
        z: 2_509,
    };
    const FIRST_DEPARTURE_POSITION: Vector3 = Vector3 {
        x: 796,
        y: 956,
        z: 1_584,
    };
    const DEPARTURE_VELOCITY: Vector3 = Vector3 {
        x: 124,
        y: -88,
        z: 8,
    };
    const FIRST_DEPARTURE_PITCH: Angle = Angle::from_units(5);
    const DEPARTURE_YAW: Angle = Angle::from_units(196);

    #[test]
    fn retail_departure_faces_the_view_anchor_and_enters_the_hold_velocity() {
        let mut object = Object::new(
            ObjectKind::Enemy,
            ShapeId::MIRAGE_DRAGON_HEAD,
            Behavior::EnemyFlight,
        );
        object.extension.activity = ObjectActivity::MirageDragonHead(MirageDragonHeadState {
            phase: MirageDragonHeadPhase::AwaitingEntrance,
            departure_motion_updates: 0,
            departure_turn_updates: 0,
        });
        activate(&mut object);
        object.base.position = PRE_DEPARTURE_POSITION;
        object.base.pitch = PRE_DEPARTURE_PITCH;
        object.base.yaw = PRE_DEPARTURE_YAW;
        begin_departure(&mut object, RETAIL_VIEW_POSITION, 1, 1);

        assert_eq!(object.base.pitch, FIRST_DEPARTURE_PITCH);
        assert_eq!(object.base.yaw, DEPARTURE_YAW);
        assert_eq!(object.base.speed, DEPARTURE_SPEED);
        assert_eq!(object.base.velocity, DEPARTURE_VELOCITY);
        assert_eq!(object.base.position, FIRST_DEPARTURE_POSITION);
    }
}
