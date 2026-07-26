//! Native articulated-body behavior for Mirage Dragon.
//!
//! The retail path keeps one stable predecessor link per part. Each active
//! part faces that predecessor, then occupies its authored depth along the
//! predecessor's orientation. This module retains those gameplay concepts
//! directly; source-machine object fields and path instructions remain
//! confined to the disassembly and oracle tooling.

use super::{Angle, MirageDragonSegmentPhase, Object, ObjectActivity, Vector3};

pub(super) const FIRST_PART_DEPTH: i8 = -45;
pub(super) const LATER_PART_DEPTH: i8 = -100;
pub(super) const BODY_ENTRANCE_RETAIL_FRAME: u16 = 64;
pub(super) const TAIL_ENTRANCE_RETAIL_FRAME: u16 = 68;
pub(super) const FOLLOWING_START_RETAIL_FRAME: u16 = 76;
#[cfg(test)]
pub(super) const LAST_PART_DEPARTURE_RETAIL_FRAME: u16 = 632;
pub(super) const PROGRESSIVE_ENTRANCE_BODY_START_INDEX: usize = 6;

const RETAIL_POSITION_SCALE: u32 = 3;
const DEPARTURE_SPEED: u8 = 216;
const DEPARTURE_PITCH_STEP: i8 = 32;
const DEPARTURE_YAW_STEP: i8 = 16;
const DEPARTURE_ROLL_STEP: i8 = 8;
const FIRST_PART_INITIAL_PITCH: Angle = Angle::from_units(253);
const FIRST_PART_INITIAL_YAW: Angle = Angle::from_units(219);
const LATER_PART_INITIAL_PITCH: Angle = Angle::from_units(63);
const LATER_PART_INITIAL_YAW: Angle = Angle::from_units(221);
const MIRAGE_DRAGON_PART_COUNT: usize = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DepartureProfile {
    velocity: Vector3,
    initial_pitch: Angle,
    initial_yaw: Angle,
    initial_roll: Angle,
}

const DEPARTURE_PROFILES: [DepartureProfile; MIRAGE_DRAGON_PART_COUNT] = [
    DepartureProfile {
        velocity: Vector3 {
            x: -52,
            y: -144,
            z: -24,
        },
        initial_pitch: Angle::from_units(165),
        initial_yaw: Angle::from_units(141),
        initial_roll: Angle::from_units(129),
    },
    DepartureProfile {
        velocity: Vector3 {
            x: -12,
            y: -156,
            z: -12,
        },
        initial_pitch: Angle::from_units(202),
        initial_yaw: Angle::from_units(188),
        initial_roll: Angle::from_units(161),
    },
    DepartureProfile {
        velocity: Vector3 {
            x: -4,
            y: -156,
            z: 0,
        },
        initial_pitch: Angle::from_units(39),
        initial_yaw: Angle::from_units(4),
        initial_roll: Angle::from_units(232),
    },
    DepartureProfile {
        velocity: Vector3 {
            x: 0,
            y: -152,
            z: 32,
        },
        initial_pitch: Angle::from_units(7),
        initial_yaw: Angle::from_units(56),
        initial_roll: Angle::from_units(168),
    },
    DepartureProfile {
        velocity: Vector3 {
            x: 4,
            y: -140,
            z: 68,
        },
        initial_pitch: Angle::from_units(204),
        initial_yaw: Angle::from_units(190),
        initial_roll: Angle::from_units(71),
    },
    DepartureProfile {
        velocity: Vector3 {
            x: 8,
            y: -96,
            z: 120,
        },
        initial_pitch: Angle::from_units(247),
        initial_yaw: Angle::from_units(165),
        initial_roll: Angle::from_units(143),
    },
    DepartureProfile {
        velocity: Vector3 {
            x: 12,
            y: 28,
            z: 148,
        },
        initial_pitch: Angle::from_units(109),
        initial_yaw: Angle::from_units(206),
        initial_roll: Angle::from_units(72),
    },
    DepartureProfile {
        velocity: Vector3 {
            x: 8,
            y: 120,
            z: 96,
        },
        initial_pitch: Angle::from_units(41),
        initial_yaw: Angle::from_units(39),
        initial_roll: Angle::from_units(199),
    },
    DepartureProfile {
        velocity: Vector3 {
            x: 4,
            y: 144,
            z: 60,
        },
        initial_pitch: Angle::from_units(238),
        initial_yaw: Angle::from_units(191),
        initial_roll: Angle::from_units(11),
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PartAnchor {
    pub position: Vector3,
    pub pitch: Angle,
    pub yaw: Angle,
}

impl PartAnchor {
    pub(super) const fn from_object(object: &Object) -> Self {
        Self {
            position: object.base.position,
            pitch: object.base.pitch,
            yaw: object.base.yaw,
        }
    }
}

pub(super) fn follow_predecessor(object: &mut Object, predecessor: PartAnchor, depth: i8) {
    face_predecessor_pitch(object, predecessor);
    face_predecessor_yaw(object, predecessor);
    follow_predecessor_position(object, predecessor, depth);
}

fn follow_predecessor_position(object: &mut Object, predecessor: PartAnchor, depth: i8) {
    let (offset_y, depth_after_pitch) =
        sf_core::snes_trig::rotate_16yz(predecessor.pitch.units(), 0, i16::from(depth));
    let (offset_x, offset_z) =
        sf_core::snes_trig::rotate_16xz(predecessor.yaw.units(), 0, depth_after_pitch);
    object.base.position = Vector3 {
        x: predecessor
            .position
            .x
            .wrapping_add(offset_x.wrapping_shl(RETAIL_POSITION_SCALE)),
        y: predecessor
            .position
            .y
            .wrapping_add(offset_y.wrapping_shl(RETAIL_POSITION_SCALE)),
        z: predecessor
            .position
            .z
            .wrapping_add(offset_z.wrapping_shl(RETAIL_POSITION_SCALE)),
    };
}

fn face_predecessor_pitch(object: &mut Object, predecessor: PartAnchor) {
    let delta_x = predecessor.position.x.wrapping_sub(object.base.position.x);
    let delta_y = predecessor.position.y.wrapping_sub(object.base.position.y);
    let delta_z = predecessor.position.z.wrapping_sub(object.base.position.z);
    object.base.pitch = Angle::from_units(sf_core::aim_angle::sf2_pitch_to_target(
        delta_y,
        sf_core::aim_angle::sf2_xz_angle_distance(delta_x, delta_z),
    ));
}

fn face_predecessor_yaw(object: &mut Object, predecessor: PartAnchor) {
    let delta_x = predecessor.position.x.wrapping_sub(object.base.position.x);
    let delta_z = predecessor.position.z.wrapping_sub(object.base.position.z);
    object.base.yaw = Angle::from_units(sf_core::aim_angle::sf2_yaw_to_target(delta_x, delta_z));
}

pub(super) fn begin_progressive_entrance(object: &mut Object, body_index: usize) {
    let ObjectActivity::MirageDragonSegment(mut state) = object.extension.activity else {
        return;
    };
    object.base.flags.active = true;
    object.base.flags.visible = true;
    object.base.flags.collision_disabled = false;
    if body_index == PROGRESSIVE_ENTRANCE_BODY_START_INDEX {
        object.base.pitch = LATER_PART_INITIAL_PITCH;
        object.base.yaw = LATER_PART_INITIAL_YAW;
    }
    state.phase = MirageDragonSegmentPhase::Entering;
    object.extension.activity = ObjectActivity::MirageDragonSegment(state);
}

pub(super) fn activate_from_head(object: &mut Object, head: PartAnchor) {
    let ObjectActivity::MirageDragonSegment(mut state) = object.extension.activity else {
        return;
    };
    follow_predecessor(object, head, state.authored_depth);
    if state.ordinal == 1 {
        object.base.pitch = FIRST_PART_INITIAL_PITCH;
        object.base.yaw = FIRST_PART_INITIAL_YAW;
    } else {
        object.base.pitch = LATER_PART_INITIAL_PITCH;
        object.base.yaw = LATER_PART_INITIAL_YAW;
    }
    object.base.roll = Angle::ZERO;
    object.base.speed = 0;
    object.base.velocity = Vector3::default();
    object.base.flags.active = true;
    object.base.flags.visible = true;
    object.base.flags.collision_disabled = false;
    state.phase = MirageDragonSegmentPhase::Following;
    object.extension.activity = ObjectActivity::MirageDragonSegment(state);
}

pub(super) fn face_linked_part_pitch(object: &mut Object, predecessor: PartAnchor) {
    let ObjectActivity::MirageDragonSegment(state) = object.extension.activity else {
        return;
    };
    if state.phase != MirageDragonSegmentPhase::Following {
        return;
    }
    face_predecessor_pitch(object, predecessor);
}

pub(super) fn face_linked_part_yaw(object: &mut Object, predecessor: PartAnchor) {
    let ObjectActivity::MirageDragonSegment(state) = object.extension.activity else {
        return;
    };
    if state.phase != MirageDragonSegmentPhase::Following {
        return;
    }
    face_predecessor_yaw(object, predecessor);
}

pub(super) fn follow_linked_part_position(object: &mut Object, predecessor: PartAnchor) {
    let ObjectActivity::MirageDragonSegment(state) = object.extension.activity else {
        return;
    };
    if state.phase != MirageDragonSegmentPhase::Following {
        return;
    }
    follow_predecessor_position(object, predecessor, state.authored_depth);
}

pub(super) fn begin_departure(object: &mut Object) {
    let ObjectActivity::MirageDragonSegment(mut state) = object.extension.activity else {
        return;
    };
    let profile = DEPARTURE_PROFILES[usize::from(state.ordinal - 1)];
    state.phase = MirageDragonSegmentPhase::Departing;
    object.base.velocity = profile.velocity;
    object.base.pitch = profile.initial_pitch;
    object.base.yaw = profile.initial_yaw;
    object.base.roll = profile.initial_roll;
    object.base.speed = DEPARTURE_SPEED;
    advance_departure_position(object);
    object.extension.activity = ObjectActivity::MirageDragonSegment(state);
}

pub(super) fn advance_departure_position(object: &mut Object) {
    object.base.position.x = object.base.position.x.wrapping_add(object.base.velocity.x);
    object.base.position.y = object.base.position.y.wrapping_add(object.base.velocity.y);
    object.base.position.z = object.base.position.z.wrapping_add(object.base.velocity.z);
}

pub(super) fn advance_departure_pitch(object: &mut Object) {
    object.base.pitch = object.base.pitch.wrapping_add(DEPARTURE_PITCH_STEP);
}

pub(super) fn advance_departure_yaw(object: &mut Object) {
    object.base.yaw = object.base.yaw.wrapping_add(DEPARTURE_YAW_STEP);
}

pub(super) fn advance_departure_roll(object: &mut Object) {
    object.base.roll = object.base.roll.wrapping_add(DEPARTURE_ROLL_STEP);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, ObjectKind, ShapeId};

    fn part(position: Vector3, pitch: u8, yaw: u8) -> Object {
        let mut object = Object::new(
            ObjectKind::Enemy,
            ShapeId::MIRAGE_DRAGON_BODY,
            Behavior::EnemyFlight,
        );
        object.base.position = position;
        object.base.pitch = Angle::from_units(pitch);
        object.base.yaw = Angle::from_units(yaw);
        object
    }

    #[test]
    fn retail_predecessor_transform_reproduces_first_live_chain_slice() {
        let mut body = [
            part(
                Vector3 {
                    x: -248,
                    y: 1_644,
                    z: -188,
                },
                253,
                219,
            ),
            part(
                Vector3 {
                    x: -584,
                    y: 1_660,
                    z: -452,
                },
                63,
                221,
            ),
            part(
                Vector3 {
                    x: -584,
                    y: 1_660,
                    z: -452,
                },
                63,
                221,
            ),
            part(
                Vector3 {
                    x: -584,
                    y: 1_660,
                    z: -452,
                },
                63,
                221,
            ),
        ];
        let mut predecessor = PartAnchor {
            position: Vector3 {
                x: 48,
                y: 1_628,
                z: 40,
            },
            pitch: Angle::from_units(252),
            yaw: Angle::from_units(221),
        };

        for (index, object) in body.iter_mut().enumerate() {
            let depth = if index == 0 {
                FIRST_PART_DEPTH
            } else {
                LATER_PART_DEPTH
            };
            follow_predecessor(object, predecessor, depth);
            predecessor = PartAnchor::from_object(object);
        }

        assert_eq!(
            body[0].base.position,
            Vector3 {
                x: -216,
                y: 1_660,
                z: -184
            }
        );
        assert_eq!(body[0].base.pitch, Angle::from_units(253));
        assert_eq!(body[0].base.yaw, Angle::from_units(219));
        assert_eq!(
            body[1].base.position,
            Vector3 {
                x: -824,
                y: 1_716,
                z: -656
            }
        );
        assert_eq!(body[1].base.pitch, Angle::from_units(0));
        assert_eq!(body[1].base.yaw, Angle::from_units(218));
        assert_eq!(
            body[2].base.position,
            Vector3 {
                x: -1_448,
                y: 1_716,
                z: -1_120
            }
        );
        assert_eq!(body[2].base.pitch, Angle::from_units(10));
        assert_eq!(body[2].base.yaw, Angle::from_units(93));
        assert_eq!(
            body[3].base.position,
            Vector3 {
                x: -872,
                y: 1_532,
                z: -632
            }
        );
        assert_eq!(body[3].base.pitch, Angle::from_units(3));
        assert_eq!(body[3].base.yaw, Angle::from_units(91));
    }
}
