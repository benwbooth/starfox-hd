//! Named actor operands for source path arithmetic. Program generation maps
//! authored operands to these fields; gameplay never resolves numeric fields
//! or source addresses. Assigning speed here is deliberately different from
//! SETVEL: ordinary variable writes do not regenerate velocity.

use super::{Angle, Object, Vector3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    fn get(self, value: Vector3) -> i16 {
        match self {
            Self::X => value.x,
            Self::Y => value.y,
            Self::Z => value.z,
        }
    }

    fn set(self, value: &mut Vector3, component: i16) {
        match self {
            Self::X => value.x = component,
            Self::Y => value.y = component,
            Self::Z => value.z = component,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordField {
    MotionPhase,
    Position(Axis),
    Velocity(Axis),
    RelativePosition(Axis),
}

impl WordField {
    pub fn read(self, actor: &Object) -> u16 {
        (match self {
            Self::MotionPhase => actor.extension.path_state.motion_phase as i16,
            Self::Position(axis) => axis.get(actor.base.position),
            Self::Velocity(axis) => axis.get(actor.base.velocity),
            Self::RelativePosition(axis) => axis.get(actor.extension.relative_position),
        }) as u16
    }

    pub fn write(self, actor: &mut Object, value: u16) {
        match self {
            Self::MotionPhase => actor.extension.path_state.motion_phase = value,
            Self::Position(axis) => axis.set(&mut actor.base.position, value as i16),
            Self::Velocity(axis) => axis.set(&mut actor.base.velocity, value as i16),
            Self::RelativePosition(axis) => {
                axis.set(&mut actor.extension.relative_position, value as i16)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BytePart {
    Low,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteField {
    Rotation(Axis),
    RelativeRotation(Axis),
    Speed,
    TargetSpeed,
    Acceleration,
    WaitTimer,
    RepeatCounter,
    Part,
    Health,
    AttackPower,
    HitFlags,
    WordPart { field: WordField, part: BytePart },
}

impl ByteField {
    pub fn read(self, actor: &Object) -> u8 {
        match self {
            Self::Rotation(axis) => match axis {
                Axis::X => actor.base.pitch,
                Axis::Y => actor.base.yaw,
                Axis::Z => actor.base.roll,
            }
            .units(),
            Self::RelativeRotation(axis) => match axis {
                Axis::X => actor.extension.relative_rotation.pitch,
                Axis::Y => actor.extension.relative_rotation.yaw,
                Axis::Z => actor.extension.relative_rotation.roll,
            }
            .units(),
            Self::Speed => actor.base.speed,
            Self::TargetSpeed => actor.base.target_speed,
            Self::Acceleration => actor.base.acceleration,
            Self::WaitTimer => actor.base.wait_timer,
            Self::RepeatCounter => actor.extension.path_state.repeat_counter,
            Self::Part => actor.extension.path_state.part,
            Self::Health => actor.base.hit_points,
            Self::AttackPower => actor.base.attack_power,
            Self::HitFlags => actor.base.hit_flags,
            Self::WordPart { field, part } => {
                let bytes = field.read(actor).to_le_bytes();
                match part {
                    BytePart::Low => bytes[0],
                    BytePart::High => bytes[1],
                }
            }
        }
    }

    pub fn write(self, actor: &mut Object, value: u8) {
        match self {
            Self::Rotation(axis) => {
                let value = Angle::from_units(value);
                match axis {
                    Axis::X => actor.base.pitch = value,
                    Axis::Y => actor.base.yaw = value,
                    Axis::Z => actor.base.roll = value,
                }
            }
            Self::RelativeRotation(axis) => {
                let value = Angle::from_units(value);
                match axis {
                    Axis::X => actor.extension.relative_rotation.pitch = value,
                    Axis::Y => actor.extension.relative_rotation.yaw = value,
                    Axis::Z => actor.extension.relative_rotation.roll = value,
                }
            }
            Self::Speed => actor.base.speed = value,
            Self::TargetSpeed => actor.base.target_speed = value,
            Self::Acceleration => actor.base.acceleration = value,
            Self::WaitTimer => actor.base.wait_timer = value,
            Self::RepeatCounter => actor.extension.path_state.repeat_counter = value,
            Self::Part => actor.extension.path_state.part = value,
            Self::Health => actor.base.hit_points = value,
            Self::AttackPower => actor.base.attack_power = value,
            Self::HitFlags => actor.base.hit_flags = value,
            Self::WordPart { field, part } => {
                let mut bytes = field.read(actor).to_le_bytes();
                match part {
                    BytePart::Low => bytes[0] = value,
                    BytePart::High => bytes[1] = value,
                }
                field.write(actor, u16::from_le_bytes(bytes));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOperand {
    Literal(u8),
    Actor(ByteField),
    LowWord(WordField),
}

impl ByteOperand {
    pub fn read(self, actor: &Object) -> u8 {
        match self {
            Self::Literal(value) => value,
            Self::Actor(field) => field.read(actor),
            Self::LowWord(field) => field.read(actor) as u8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordOperand {
    Literal(u16),
    Actor(WordField),
    /// Byte-to-word copies and adds sign-extend (`$7F:8976`, `$7F:8925`).
    SignedByte(ByteOperand),
}

impl WordOperand {
    pub fn read(self, actor: &Object) -> u16 {
        match self {
            Self::Literal(value) => value,
            Self::Actor(field) => field.read(actor),
            Self::SignedByte(value) => value.read(actor) as i8 as i16 as u16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOperation {
    Assign(ByteOperand),
    Add(ByteOperand),
    Increment,
    Decrement,
    Negate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordOperation {
    Assign(WordOperand),
    Add(WordOperand),
    Increment,
    Decrement,
    Negate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutation {
    Byte {
        field: ByteField,
        operation: ByteOperation,
    },
    Word {
        field: WordField,
        operation: WordOperation,
    },
}

impl Mutation {
    pub fn apply(self, actor: &mut Object) {
        match self {
            Self::Byte { field, operation } => {
                let old = field.read(actor);
                let value = match operation {
                    ByteOperation::Assign(value) => value.read(actor),
                    ByteOperation::Add(value) => old.wrapping_add(value.read(actor)),
                    ByteOperation::Increment => old.wrapping_add(1),
                    ByteOperation::Decrement => old.wrapping_sub(1),
                    ByteOperation::Negate => old.wrapping_neg(),
                };
                field.write(actor, value);
            }
            Self::Word { field, operation } => {
                let old = field.read(actor);
                let value = match operation {
                    WordOperation::Assign(value) => value.read(actor),
                    WordOperation::Add(value) => old.wrapping_add(value.read(actor)),
                    WordOperation::Increment => old.wrapping_add(1),
                    WordOperation::Decrement => old.wrapping_sub(1),
                    WordOperation::Negate => old.wrapping_neg(),
                };
                field.write(actor, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, ObjectKind, ShapeId};

    fn actor() -> Object {
        Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath)
    }

    #[test]
    fn scalar_fields_preserve_full_bytes_and_relative_angles_are_independent() {
        let mut actor = actor();
        let fields = [
            ByteField::Rotation(Axis::X),
            ByteField::Rotation(Axis::Y),
            ByteField::Rotation(Axis::Z),
            ByteField::RelativeRotation(Axis::X),
            ByteField::RelativeRotation(Axis::Y),
            ByteField::RelativeRotation(Axis::Z),
            ByteField::Speed,
            ByteField::TargetSpeed,
            ByteField::Acceleration,
            ByteField::WaitTimer,
            ByteField::RepeatCounter,
            ByteField::Part,
            ByteField::Health,
            ByteField::AttackPower,
            ByteField::HitFlags,
        ];
        for (index, field) in fields.into_iter().enumerate() {
            for value in 0..=u8::MAX {
                field.write(&mut actor, value);
                assert_eq!(field.read(&actor), value);
            }
            field.write(&mut actor, index as u8);
        }
        for (index, field) in fields.into_iter().enumerate() {
            assert_eq!(field.read(&actor), index as u8);
        }
    }

    #[test]
    fn byte_word_views_alias_typed_coordinates_without_affecting_other_components() {
        let mut actor = actor();
        for field in [
            WordField::Position(Axis::X),
            WordField::Velocity(Axis::Y),
            WordField::RelativePosition(Axis::Z),
        ] {
            field.write(&mut actor, 0xABCD);
            let low = ByteField::WordPart {
                field,
                part: BytePart::Low,
            };
            let high = ByteField::WordPart {
                field,
                part: BytePart::High,
            };
            assert_eq!((low.read(&actor), high.read(&actor)), (0xCD, 0xAB));
            low.write(&mut actor, 0x12);
            assert_eq!(field.read(&actor), 0xAB12);
            high.write(&mut actor, 0x34);
            assert_eq!(field.read(&actor), 0x3412);
        }
        assert_eq!(actor.base.position.y, 0);
        assert_eq!(actor.base.velocity.x, 0);
        assert_eq!(actor.extension.relative_position.x, 0);
    }

    #[test]
    fn copies_and_adds_sample_source_before_writing_and_sign_extend_bytes() {
        let mut actor = actor();
        let field = WordField::Position(Axis::X);
        for value in 0..=u8::MAX {
            ByteField::Speed.write(&mut actor, value);
            Mutation::Word {
                field,
                operation: WordOperation::Assign(WordOperand::SignedByte(ByteOperand::Actor(
                    ByteField::Speed,
                ))),
            }
            .apply(&mut actor);
            assert_eq!(actor.base.position.x, i16::from(value as i8));
            Mutation::Word {
                field,
                operation: WordOperation::Add(WordOperand::Actor(field)),
            }
            .apply(&mut actor);
            assert_eq!(actor.base.position.x, i16::from(value as i8) * 2);
        }
        field.write(&mut actor, 0xABCD);
        Mutation::Byte {
            field: ByteField::WordPart {
                field,
                part: BytePart::High,
            },
            operation: ByteOperation::Assign(ByteOperand::LowWord(field)),
        }
        .apply(&mut actor);
        assert_eq!(field.read(&actor), 0xCDCD);
    }

    #[test]
    fn arithmetic_wraps_at_each_destination_width_and_angle_writes_do_not_generate_velocity() {
        let mut actor = actor();
        for value in 0..=u8::MAX {
            for (operation, expected) in [
                (ByteOperation::Increment, value.wrapping_add(1)),
                (ByteOperation::Decrement, value.wrapping_sub(1)),
                (ByteOperation::Negate, value.wrapping_neg()),
                (
                    ByteOperation::Add(ByteOperand::Literal(127)),
                    value.wrapping_add(127),
                ),
            ] {
                ByteField::Rotation(Axis::Y).write(&mut actor, value);
                Mutation::Byte {
                    field: ByteField::Rotation(Axis::Y),
                    operation,
                }
                .apply(&mut actor);
                assert_eq!(actor.base.yaw.units(), expected);
            }
        }
        for value in 0..=u16::MAX {
            for (operation, expected) in [
                (WordOperation::Increment, value.wrapping_add(1)),
                (WordOperation::Decrement, value.wrapping_sub(1)),
                (WordOperation::Negate, value.wrapping_neg()),
            ] {
                WordField::Position(Axis::X).write(&mut actor, value);
                Mutation::Word {
                    field: WordField::Position(Axis::X),
                    operation,
                }
                .apply(&mut actor);
                assert_eq!(actor.base.position.x as u16, expected);
            }
        }
        Mutation::Byte {
            field: ByteField::Speed,
            operation: ByteOperation::Assign(ByteOperand::Literal(100)),
        }
        .apply(&mut actor);
        assert_eq!(actor.base.velocity, Vector3::default());
    }
}
