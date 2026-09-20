//! Named actor operands for source path arithmetic. Program generation maps
//! authored operands to these fields; gameplay never resolves numeric fields
//! or source addresses. Assigning speed here is deliberately different from
//! SETVEL: ordinary variable writes do not regenerate velocity.

use super::path_appearance::{AnimationChannel, AnimationControl};
use super::{Angle, Object, Vector3};

const ARITHMETIC_CHASE_DIVISOR: i16 = 8;

fn chase_step(delta: i16) -> i16 {
    if delta == 0 {
        return 0;
    }
    let delta = if delta < 0 {
        delta.min(-ARITHMETIC_CHASE_DIVISOR)
    } else {
        delta.max(ARITHMETIC_CHASE_DIVISOR)
    };
    delta / ARITHMETIC_CHASE_DIVISOR
}

/// Source arithmetic chase ($7F:9FFF/$7F:A054). Subtraction wraps at the
/// authored field width before its sign is interpreted; minimum progress is
/// one unit, and division rounds toward zero.
pub fn chase_byte(current: u8, target: u8) -> u8 {
    current.wrapping_add(chase_step(i16::from(target.wrapping_sub(current) as i8)) as u8)
}

pub fn chase_word(current: u16, target: u16) -> u16 {
    current.wrapping_add(chase_step(target.wrapping_sub(current) as i16) as u16)
}

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
    /// Packed path control, not the renderer's resolved frame snapshot.
    Animation(AnimationChannel),
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
            Self::Animation(channel) => match channel {
                AnimationChannel::Shape => actor.extension.path_state.animation.shape,
                AnimationChannel::Color => actor.extension.path_state.animation.color,
            }
            .packed(),
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
            Self::Animation(channel) => {
                let control = AnimationControl::from_packed(value);
                match channel {
                    AnimationChannel::Shape => actor.extension.path_state.animation.shape = control,
                    AnimationChannel::Color => actor.extension.path_state.animation.color = control,
                }
            }
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
    /// Source constant table decoded offline over the complete byte index.
    Lookup {
        selector: ByteField,
        values: &'static [u8; 256],
    },
}

impl ByteOperand {
    pub fn read(self, actor: &Object) -> u8 {
        match self {
            Self::Literal(value) => value,
            Self::Actor(field) => field.read(actor),
            Self::LowWord(field) => field.read(actor) as u8,
            Self::Lookup { selector, values } => values[usize::from(selector.read(actor))],
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
    Chase(ByteOperand),
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
    Chase(WordOperand),
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
                    ByteOperation::Chase(value) => chase_byte(old, value.read(actor)),
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
                    WordOperation::Chase(value) => chase_word(old, value.read(actor)),
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

    #[test]
    fn byte_lookup_reads_live_unsigned_index_before_an_overlapping_write() {
        static VALUES: [u8; 256] = {
            let mut values = [0; 256];
            let mut index = 0;
            while index < values.len() {
                values[index] = (index as u8).wrapping_mul(53).wrapping_add(129);
                index += 1;
            }
            values
        };
        for selector in [
            ByteField::Part,
            ByteField::WordPart {
                field: WordField::MotionPhase,
                part: BytePart::High,
            },
            ByteField::Animation(AnimationChannel::Color),
        ] {
            for initial in 0..=u8::MAX {
                let mut actual = actor();
                actual.extension.path_state.motion_phase = 0xAC53;
                actual.extension.color_frame = 19;
                actual.base.wait_timer = 71;
                selector.write(&mut actual, initial);
                let mut expected = actual.clone();
                let operand = ByteOperand::Lookup {
                    selector,
                    values: &VALUES,
                };
                let mutation = Mutation::Byte {
                    field: selector,
                    operation: ByteOperation::Assign(operand),
                };
                for _ in 0..3 {
                    let old = selector.read(&expected);
                    let value = ((u16::from(old) * 53 + 129) & 255) as u8;
                    selector.write(&mut expected, value);
                    mutation.apply(&mut actual);
                    assert_eq!(actual, expected);
                }
            }
        }
    }

    #[test]
    fn packed_animation_fields_keep_both_modes_and_publish_only_at_render_boundary() {
        use super::super::path_appearance::publish_animation;
        for channel in [AnimationChannel::Shape, AnimationChannel::Color] {
            let field = ByteField::Animation(channel);
            for old in 0..=u8::MAX {
                for (operation, packed) in [
                    (ByteOperation::Assign(ByteOperand::Literal(0)), 0),
                    (ByteOperation::Assign(ByteOperand::Actor(field)), old),
                    (ByteOperation::Increment, old.wrapping_add(1)),
                    (ByteOperation::Decrement, old.wrapping_sub(1)),
                    (
                        ByteOperation::Add(ByteOperand::Actor(field)),
                        old.wrapping_add(old),
                    ),
                    (ByteOperation::Negate, old.wrapping_neg()),
                ] {
                    let mut actual = actor();
                    actual.base.wait_timer = 83;
                    actual.base.velocity = Vector3 {
                        x: 17,
                        y: -32768,
                        z: 219,
                    };
                    actual.extension.animation_frame = 53;
                    actual.extension.color_frame = 91;
                    actual.extension.path_state.animation.shape =
                        AnimationControl::from_packed(0xA7);
                    actual.extension.path_state.animation.color =
                        AnimationControl::from_packed(0xBC);
                    let mut expected = actual.clone();
                    match channel {
                        AnimationChannel::Shape => {
                            expected.extension.path_state.animation.shape =
                                AnimationControl::from_packed(old)
                        }
                        AnimationChannel::Color => {
                            expected.extension.path_state.animation.color =
                                AnimationControl::from_packed(old)
                        }
                    }
                    field.write(&mut actual, old);
                    assert_eq!(field.read(&actual), old);
                    assert_eq!(actual, expected);
                    match channel {
                        AnimationChannel::Shape => {
                            expected.extension.path_state.animation.shape =
                                AnimationControl::from_packed(packed)
                        }
                        AnimationChannel::Color => {
                            expected.extension.path_state.animation.color =
                                AnimationControl::from_packed(packed)
                        }
                    }
                    Mutation::Byte { field, operation }.apply(&mut actual);
                    assert_eq!(field.read(&actual), packed);
                    assert_eq!(actual, expected);
                    for clock in [0, 49, 127, 128, 255] {
                        let resolved = if packed & 0x80 == 0 {
                            clock & 0x7F
                        } else {
                            packed & 0x7F
                        };
                        match channel {
                            AnimationChannel::Shape => {
                                expected.extension.animation_frame = resolved;
                                expected.extension.color_frame = 60;
                            }
                            AnimationChannel::Color => {
                                expected.extension.animation_frame = 39;
                                expected.extension.color_frame = resolved;
                            }
                        }
                        publish_animation(&mut actual, clock);
                        assert_eq!(actual, expected);
                    }
                }
            }
        }
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

    fn source_chase_step(mut difference: i32) -> i32 {
        if difference == 0 {
            return 0;
        }
        if (1..8).contains(&difference) {
            difference = 8;
        }
        if (-7..0).contains(&difference) {
            difference = -8;
        }
        // Three separate signed halves, adding the lost low bit back to
        // a negative result on each half, as in the source kernel.
        for _ in 0..3 {
            let lost = difference & 1;
            difference >>= 1;
            if difference < 0 {
                difference += lost;
            }
        }
        difference
    }

    #[test]
    fn arithmetic_chase_preserves_wrapped_difference_width_and_samples_live_targets() {
        let mut original = actor();
        original.base.wait_timer = 31;
        original.base.hit_flags = 0xA5;
        for current in 0..=u8::MAX {
            for target in 0..=u8::MAX {
                let mut actual = original.clone();
                actual.extension.path_state.script_parameter = current;
                actual.base.hit_points = target;
                let mut expected = actual.clone();
                expected.extension.path_state.script_parameter = current.wrapping_add(
                    source_chase_step(i32::from(target.wrapping_sub(current) as i8)) as u8,
                );
                Mutation::Byte {
                    field: ByteField::ScriptParameter,
                    operation: ByteOperation::Chase(ByteOperand::Actor(ByteField::Health)),
                }
                .apply(&mut actual);
                assert_eq!(actual, expected);
                Mutation::Byte {
                    field: ByteField::ScriptParameter,
                    operation: ByteOperation::Chase(ByteOperand::Actor(ByteField::ScriptParameter)),
                }
                .apply(&mut actual);
                assert_eq!(actual, expected);
            }
        }
        for current in [0_u16, 1, 0x7FFF, 0x8000, 0xFFFF] {
            for delta in 0..=u16::MAX {
                let mut actual = original.clone();
                actual.extension.path_state.script_value = current;
                actual.extension.path_state.motion_phase = current.wrapping_add(delta);
                let mut expected = actual.clone();
                expected.extension.path_state.script_value =
                    current.wrapping_add(source_chase_step(i32::from(delta as i16)) as u16);
                Mutation::Word {
                    field: WordField::ScriptValue,
                    operation: WordOperation::Chase(WordOperand::Actor(WordField::MotionPhase)),
                }
                .apply(&mut actual);
                assert_eq!(actual, expected);
                Mutation::Word {
                    field: WordField::ScriptValue,
                    operation: WordOperation::Chase(WordOperand::Actor(WordField::ScriptValue)),
                }
                .apply(&mut actual);
                assert_eq!(actual, expected);
            }
        }
    }

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
