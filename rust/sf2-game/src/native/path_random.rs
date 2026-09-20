//! Random path operations borrow the world's shared generator. Word draws
//! consume the first byte as HIGH and the next as LOW, even for a zero mask.
//! Centering subtracts a logical half-mask; destination arithmetic wraps.

use super::path_fields::{ByteField, WordField};
use super::{Object, RandomState};

const BRANCH_EXCLUSIVE_CUTOFF: u8 = 127;

/// Source RANDOM GOTO takes 127 of the 256 byte outcomes, not an even half.
/// It always consumes one shared draw, even when both successors coincide.
pub fn take_branch(random: &mut RandomState) -> bool {
    random.next_byte() < BRANCH_EXCLUSIVE_CUTOFF
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandomMutation {
    AssignByte { field: ByteField, mask: u8 },
    AssignWord { field: WordField, mask: u16 },
    AddCenteredByte { field: ByteField, mask: u8 },
    AddCenteredWord { field: WordField, mask: u16 },
}

fn next_word(random: &mut RandomState) -> u16 {
    let high = random.next_byte();
    let low = random.next_byte();
    u16::from_be_bytes([high, low])
}

impl RandomMutation {
    pub fn apply(self, actor: &mut Object, random: &mut RandomState) {
        match self {
            Self::AssignByte { field, mask } => field.write(actor, random.next_byte() & mask),
            Self::AssignWord { field, mask } => field.write(actor, next_word(random) & mask),
            Self::AddCenteredByte { field, mask } => {
                let displacement = (random.next_byte() & mask).wrapping_sub(mask >> 1);
                field.write(actor, field.read(actor).wrapping_add(displacement));
            }
            Self::AddCenteredWord { field, mask } => {
                let displacement = (next_word(random) & mask).wrapping_sub(mask >> 1);
                field.write(actor, field.read(actor).wrapping_add(displacement));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::path_fields::{Axis, BytePart};
    use super::super::{Behavior, ObjectKind, ShapeId};
    use super::*;

    fn actor() -> Object {
        Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath)
    }

    #[test]
    fn random_branch_preserves_the_strict_cutoff_and_consumes_exactly_one_draw() {
        let mut seen = [false; 256];
        for first in [0, 1] {
            for last in 0..=u8::MAX {
                let mut random = RandomState::new([first, 0, 0, last]);
                let mut expected = random;
                let sample = expected.next_byte();
                seen[usize::from(sample)] = true;
                assert_eq!(take_branch(&mut random), sample <= 126);
                assert_eq!(random, expected);
            }
        }
        assert!(seen.into_iter().all(|value| value));
    }

    #[test]
    fn shared_random_draw_matches_widened_subtraction_across_borrow_boundaries() {
        for first in 0..=u8::MAX {
            for second in 0..=u8::MAX {
                for third in [0, 1, 127, 128, 254, 255] {
                    for fourth in [0, 1, 127, 128, 254, 255] {
                        let mut expected = [first, second, third, fourth];
                        let mut value = i16::from(first);
                        let mut borrow = 1;
                        for index in [1, 2, 3, 0] {
                            let difference = value - i16::from(expected[index]) - borrow;
                            borrow = i16::from(difference < 0);
                            value = difference.rem_euclid(256);
                            expected[index] = value as u8;
                        }
                        let mut random = RandomState::new([first, second, third, fourth]);
                        assert_eq!(random.next_byte(), expected[0]);
                        assert_eq!(random.bytes(), expected);
                    }
                }
            }
        }
    }

    #[test]
    fn byte_assignment_consumes_one_draw_even_for_zero_mask() {
        for mask in 0..=u8::MAX {
            let mut actor = actor();
            actor.base.hit_points = 255;
            let mut random = RandomState::default();
            let mut expected = random;
            let sample = expected.next_byte();
            RandomMutation::AssignByte {
                field: ByteField::Health,
                mask,
            }
            .apply(&mut actor, &mut random);
            assert_eq!(actor.base.hit_points, sample & mask);
            assert_eq!(random, expected);
        }
    }

    #[test]
    fn word_order_is_first_high_and_zero_mask_still_draws_twice() {
        for mask in [0, 15, 255, 32768, u16::MAX] {
            let mut random = RandomState::default();
            let mut expected = random;
            let high = expected.next_byte();
            let low = expected.next_byte();
            assert_ne!(high, low);
            let mut actor = actor();
            RandomMutation::AssignWord {
                field: WordField::Position(Axis::X),
                mask,
            }
            .apply(&mut actor, &mut random);
            assert_eq!(
                actor.base.position.x as u16,
                u16::from_be_bytes([high, low]) & mask
            );
            assert_eq!(random, expected);
        }
    }

    #[test]
    fn centered_bytes_preserve_other_half_of_aliased_words() {
        let field = ByteField::WordPart {
            field: WordField::MotionPhase,
            part: BytePart::Low,
        };
        for mask in 0..=u8::MAX {
            for before in [0u8, 1, 127, 128, 255] {
                let mut actor = actor();
                actor.extension.path_state.motion_phase = 0xA500 | u16::from(before);
                let mut random = RandomState::default();
                let mut expected = random;
                let sample = expected.next_byte();
                RandomMutation::AddCenteredByte { field, mask }.apply(&mut actor, &mut random);
                let result =
                    (i32::from(before) + i32::from(sample & mask) - i32::from(mask / 2)) as u8;
                assert_eq!(
                    actor.extension.path_state.motion_phase,
                    0xA500 | u16::from(result)
                );
                assert_eq!(random, expected);
            }
        }
    }

    #[test]
    fn centered_words_keep_unsigned_half_mask_and_wrap() {
        let field = WordField::Position(Axis::Y);
        for mask in 0..=u16::MAX {
            let mut actor = actor();
            field.write(&mut actor, 65530);
            let mut random = RandomState::default();
            let mut expected = random;
            let sample = u16::from_be_bytes([expected.next_byte(), expected.next_byte()]);
            RandomMutation::AddCenteredWord { field, mask }.apply(&mut actor, &mut random);
            assert_eq!(
                field.read(&actor),
                (65530 + i32::from(sample & mask) - i32::from(mask / 2)) as u16
            );
            assert_eq!(random, expected);
        }
    }
}
