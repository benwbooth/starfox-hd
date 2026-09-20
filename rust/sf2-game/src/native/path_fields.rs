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
    ScriptValue,
    Position(Axis),
    Velocity(Axis),
    RelativePosition(Axis),
}

impl WordField {
    pub fn read(self, actor: &Object) -> u16 {
        (match self {
            Self::MotionPhase => actor.extension.path_state.motion_phase as i16,
            Self::ScriptValue => actor.extension.path_state.script_value as i16,
            Self::Position(axis) => axis.get(actor.base.position),
            Self::Velocity(axis) => axis.get(actor.base.velocity),
            Self::RelativePosition(axis) => axis.get(actor.extension.relative_position),
        }) as u16
    }

    pub fn write(self, actor: &mut Object, value: u16) {
        match self {
            Self::MotionPhase => actor.extension.path_state.motion_phase = value,
            Self::ScriptValue => actor.extension.path_state.script_value = value,
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
    ChildNumber,
    /// Source texture-X channel, also used as scaled-sprite size.
    TextureScrollX,
    Rotation(Axis),
    RelativeRotation(Axis),
    Speed,
    TargetSpeed,
    Acceleration,
    WaitTimer,
    ScriptParameter,
    RepeatCounter,
    Part,
    Health,
    AttackPower,
    HitFlags,
    WordPart {
        field: WordField,
        part: BytePart,
    },
}

impl ByteField {
    pub fn read(self, actor: &Object) -> u8 {
        match self {
            Self::ChildNumber => actor.base.child_number,
            Self::TextureScrollX => actor.extension.texture_scroll_x,
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
            Self::ScriptParameter => actor.extension.path_state.script_parameter,
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
            Self::ChildNumber => actor.base.child_number = value,
            Self::TextureScrollX => actor.extension.texture_scroll_x = value,
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
            Self::ScriptParameter => actor.extension.path_state.script_parameter = value,
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
    /// Variable-byte loop counts are zero-extended (`$7F:95E9`).
    UnsignedByte(ByteOperand),
    /// One-based authored bit selector. Its byte-width doubling aliases
    /// selectors separated by 128. The complete decoded lookup also retains
    /// source results outside the conventional sixteen single-bit entries.
    IndexedBitMask {
        selector: ByteOperand,
        masks: &'static [u16; 128],
    },
}

impl WordOperand {
    pub fn read(self, actor: &Object) -> u16 {
        match self {
            Self::Literal(value) => value,
            Self::Actor(field) => field.read(actor),
            Self::SignedByte(value) => value.read(actor) as i8 as i16 as u16,
            Self::UnsignedByte(value) => u16::from(value.read(actor)),
            Self::IndexedBitMask { selector, masks } => {
                let index = selector.read(actor).wrapping_sub(1) & 0x7F;
                masks[usize::from(index)]
            }
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
    HalfTowardZero,
    LogicalHalf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordOperation {
    Assign(WordOperand),
    Add(WordOperand),
    SetBits(WordOperand),
    ClearBits(WordOperand),
    Increment,
    Decrement,
    Negate,
    HalfTowardZero,
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
                    ByteOperation::HalfTowardZero => ((old as i8) / 2) as u8,
                    ByteOperation::LogicalHalf => old >> 1,
                };
                field.write(actor, value);
            }
            Self::Word { field, operation } => {
                let old = field.read(actor);
                let value = match operation {
                    WordOperation::Assign(value) => value.read(actor),
                    WordOperation::Add(value) => old.wrapping_add(value.read(actor)),
                    WordOperation::SetBits(mask) => old | mask.read(actor),
                    WordOperation::ClearBits(mask) => old & !mask.read(actor),
                    WordOperation::Increment => old.wrapping_add(1),
                    WordOperation::Decrement => old.wrapping_sub(1),
                    WordOperation::Negate => old.wrapping_neg(),
                    WordOperation::HalfTowardZero => ((old as i16) / 2) as u16,
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

    const MASKS: [u16; 128] = {
        let mut masks = [0; 128];
        let mut index = 0;
        while index < masks.len() {
            // Distinct multi-bit masks make lookup and alias errors visible.
            masks[index] = (index as u16 * 257) ^ 0xA55A;
            index += 1;
        }
        masks
    };

    #[test]
    fn signed_halves_round_toward_zero_and_logical_half_keeps_unsigned_meaning() {
        let mut actor = actor();
        for old in 0..=u16::MAX {
            WordField::MotionPhase.write(&mut actor, old);
            let mut expected = actor.clone();
            // Source adds one only to negatives before an arithmetic shift.
            let adjusted = old.wrapping_add(u16::from(old & 0x8000 != 0));
            expected.extension.path_state.motion_phase = ((adjusted as i16) >> 1) as u16;
            Mutation::Word {
                field: WordField::MotionPhase,
                operation: WordOperation::HalfTowardZero,
            }
            .apply(&mut actor);
            assert_eq!(actor, expected);
        }
        for old in 0..=u8::MAX {
            for operation in [ByteOperation::HalfTowardZero, ByteOperation::LogicalHalf] {
                ByteField::Speed.write(&mut actor, old);
                let mut expected = actor.clone();
                expected.base.speed = match operation {
                    ByteOperation::HalfTowardZero => {
                        let adjusted = old.wrapping_add(u8::from(old & 0x80 != 0));
                        ((adjusted as i8) >> 1) as u8
                    }
                    _ => old >> 1,
                };
                Mutation::Byte {
                    field: ByteField::Speed,
                    operation,
                }
                .apply(&mut actor);
                assert_eq!(actor, expected);
            }
        }
    }

    #[test]
    fn indexed_masks_keep_one_based_byte_wrap_and_sample_before_aliasing_writes() {
        let field = WordField::MotionPhase;
        for selector in 0..=u8::MAX {
            // Independent transcription of source byte decrement/double,
            // followed by a little-endian word lookup.
            let byte_offset = selector.wrapping_sub(1).wrapping_mul(2);
            let mask = MASKS[usize::from(byte_offset) / 2];
            for high in [0, 127, 128, 255] {
                for part in [BytePart::Low, BytePart::High] {
                    let mut original = actor();
                    let bytes = match part {
                        BytePart::Low => [selector, high],
                        BytePart::High => [high, selector],
                    };
                    let old = u16::from_le_bytes(bytes);
                    field.write(&mut original, old);
                    let operand = WordOperand::IndexedBitMask {
                        selector: ByteOperand::Actor(ByteField::WordPart { field, part }),
                        masks: &MASKS,
                    };
                    assert_eq!(operand.read(&original), mask);
                    for (operation, expected) in [
                        (WordOperation::SetBits(operand), old | mask),
                        (WordOperation::ClearBits(operand), old & !mask),
                    ] {
                        let mut actual = original.clone();
                        Mutation::Word { field, operation }.apply(&mut actual);
                        let mut expected_actor = original.clone();
                        field.write(&mut expected_actor, expected);
                        assert_eq!(actual, expected_actor);
                    }
                }
            }
        }
    }

    #[test]
    fn scalar_fields_preserve_full_bytes_and_relative_angles_are_independent() {
        let mut actor = actor();
        let fields = [
            ByteField::ChildNumber,
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
            ByteField::ScriptParameter,
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
    fn working_word_full_width_and_byte_views_preserve_other_path_state() {
        let mut original = actor();
        original.extension.path_state.motion_phase = 0x1357;
        original.extension.path_state.script_parameter = 0xA5;
        original.extension.path_state.repeat_counter = 19;
        original.base.position.y = -123;
        for value in 0..=u16::MAX {
            let mut actual = original.clone();
            let mut expected = original.clone();
            expected.extension.path_state.script_value = value;
            WordField::ScriptValue.write(&mut actual, value);
            assert_eq!(actual, expected);
            assert_eq!(WordField::ScriptValue.read(&actual), value);
            for part in [BytePart::Low, BytePart::High] {
                let field = ByteField::WordPart {
                    field: WordField::ScriptValue,
                    part,
                };
                let byte = match part {
                    BytePart::Low => value as u8,
                    BytePart::High => (value >> 8) as u8,
                };
                assert_eq!(field.read(&actual), byte);
                let mut changed = actual.clone();
                field.write(&mut changed, byte ^ 0xFF);
                let mut expected_changed = expected.clone();
                expected_changed.extension.path_state.script_value = value
                    ^ match part {
                        BytePart::Low => 0x00FF,
                        BytePart::High => 0xFF00,
                    };
                assert_eq!(changed, expected_changed);
            }
        }
    }

    #[test]
    fn script_parameter_copies_values_without_aliasing_health_height_or_loop_count() {
        for value in 0..=u8::MAX {
            let mut actual = actor();
            actual.base.hit_points = value;
            actual.base.position.y = i16::from_le_bytes([value, 0xAB]);
            actual.base.wait_timer = 17;
            actual.extension.path_state.repeat_counter = 29;
            actual.extension.path_state.motion_phase = 0x5678;
            let original = actual.clone();
            let mut expected = original.clone();
            expected.extension.path_state.script_parameter = value;
            for operand in [
                ByteOperand::Actor(ByteField::Health),
                ByteOperand::LowWord(WordField::Position(Axis::Y)),
            ] {
                actual = original.clone();
                Mutation::Byte {
                    field: ByteField::ScriptParameter,
                    operation: ByteOperation::Assign(operand),
                }
                .apply(&mut actual);
                assert_eq!(actual, expected);
                assert_eq!(ByteField::ScriptParameter.read(&actual), value);
                Mutation::Byte {
                    field: ByteField::ScriptParameter,
                    operation: ByteOperation::Add(ByteOperand::Literal(255)),
                }
                .apply(&mut actual);
                let mut decremented = expected.clone();
                decremented.extension.path_state.script_parameter = value.wrapping_sub(1);
                assert_eq!(actual, decremented);
            }
        }
    }

    #[test]
    fn byte_word_views_alias_typed_coordinates_without_affecting_other_components() {
        let mut actor = actor();
        for field in [
            WordField::ScriptValue,
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
    fn variable_copies_and_adds_sample_overlapping_fields_before_writing_destination() {
        let word = WordField::MotionPhase;
        let low_field = ByteField::WordPart {
            field: word,
            part: BytePart::Low,
        };
        let high_field = ByteField::WordPart {
            field: word,
            part: BytePart::High,
        };
        for high in [0u8, 127, 128, 255] {
            for low in 0..=u8::MAX {
                let initial = u16::from_le_bytes([low, high]);
                let cases = [
                    (
                        Mutation::Word {
                            field: word,
                            operation: WordOperation::Assign(WordOperand::SignedByte(
                                ByteOperand::Actor(low_field),
                            )),
                        },
                        low as i8 as i16 as u16,
                    ),
                    (
                        Mutation::Word {
                            field: word,
                            operation: WordOperation::Add(WordOperand::SignedByte(
                                ByteOperand::Actor(high_field),
                            )),
                        },
                        initial.wrapping_add(high as i8 as i16 as u16),
                    ),
                    (
                        Mutation::Byte {
                            field: high_field,
                            operation: ByteOperation::Assign(ByteOperand::LowWord(word)),
                        },
                        u16::from_le_bytes([low, low]),
                    ),
                    (
                        Mutation::Byte {
                            field: high_field,
                            operation: ByteOperation::Add(ByteOperand::Actor(low_field)),
                        },
                        u16::from_le_bytes([low, high.wrapping_add(low)]),
                    ),
                ];
                for (mutation, expected_phase) in cases {
                    let mut actor = actor();
                    actor.extension.path_state.motion_phase = initial;
                    actor.base.velocity = Vector3 {
                        x: 123,
                        y: -456,
                        z: 789,
                    };
                    actor.base.wait_timer = 97;
                    let mut expected = actor.clone();
                    expected.extension.path_state.motion_phase = expected_phase;
                    mutation.apply(&mut actor);
                    assert_eq!(actor, expected);
                }
            }
        }
    }

    #[test]
    fn loop_counts_zero_extend_each_byte_without_changing_signed_arithmetic() {
        let mut actor = actor();
        for value in 0..=u8::MAX {
            actor.base.target_speed = value;
            let operand = ByteOperand::Actor(ByteField::TargetSpeed);
            assert_eq!(
                WordOperand::UnsignedByte(operand).read(&actor),
                u16::from(value)
            );
            assert_eq!(
                WordOperand::SignedByte(operand).read(&actor),
                value as i8 as i16 as u16
            );
        }
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
