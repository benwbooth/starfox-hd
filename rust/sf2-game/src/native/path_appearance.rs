//! Source path animation controls and scaled-sprite parameters. An animation
//! control is not a resolved presentation frame: automatic controls retain a
//! low payload that arithmetic still observes, even though rendering uses the
//! shared animation clock instead.

use super::Object;

const MANUAL_FRAME: u8 = 0x80;
const FRAME_VALUE: u8 = 0x7f;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AnimationControl {
    manual: bool,
    value: u8,
}

impl AnimationControl {
    /// Decode only the animation channel's own packed byte, not an object
    /// address or a generic source-machine field.
    pub const fn from_packed(value: u8) -> Self {
        Self {
            manual: value & MANUAL_FRAME != 0,
            value: value & FRAME_VALUE,
        }
    }

    pub const fn packed(self) -> u8 {
        self.value | if self.manual { MANUAL_FRAME } else { 0 }
    }

    pub const fn resolve(self, clock: u8) -> u8 {
        if self.manual {
            self.value
        } else {
            clock & FRAME_VALUE
        }
    }

    /// INITANIM / INITCOLANIM force manual selection and retain seven bits.
    pub fn initialize(&mut self, value: u8) {
        *self = Self::from_packed(value | MANUAL_FRAME);
    }

    /// `$7F:8D0E` / `$7F:8D4B`: byte addition, conditional correction,
    /// seven-bit truncation, then at most one unsigned subtraction. Neither
    /// arbitrary amounts nor zero/large periods are normalized with modulo.
    pub fn advance(&mut self, amount: u8, period: u8) {
        let mut value = self.packed().wrapping_add(amount);
        if value & MANUAL_FRAME == 0 {
            value = value.wrapping_add(period);
        }
        value &= FRAME_VALUE;
        if value >= period {
            value = value.wrapping_sub(period);
        }
        self.initialize(value);
    }
}

/// `$7F:99D5`: these are the source depth-offset and texture-X channels,
/// repurposed by the renderer for sprite colour and size. World position,
/// texture Y, visibility and all other flags remain untouched.
pub fn set_sprite(actor: &mut Object, color: u8, size: u8) {
    actor.base.flags.scaled_sprite = true;
    actor.extension.depth_offset = color;
    actor.extension.texture_scroll_x = size;
}

#[cfg(test)]
mod tests {
    use super::super::{Behavior, ObjectKind, ShapeId, Vector3};
    use super::*;

    #[test]
    fn packed_controls_preserve_automatic_payload_but_render_from_clock() {
        for value in 0..=u8::MAX {
            let control = AnimationControl::from_packed(value);
            assert_eq!(control.packed(), value);
            for clock in 0..=u8::MAX {
                assert_eq!(
                    control.resolve(clock),
                    if value < MANUAL_FRAME {
                        clock & FRAME_VALUE
                    } else {
                        value & FRAME_VALUE
                    }
                );
            }
        }
    }

    #[test]
    fn initialization_always_switches_to_manual_and_truncates_high_bit() {
        for value in 0..=u8::MAX {
            let mut control = AnimationControl::default();
            control.initialize(value);
            assert_eq!(control.packed(), value | MANUAL_FRAME);
            assert_eq!(control.resolve(value.wrapping_add(1)), value & FRAME_VALUE);
        }
    }

    #[test]
    fn advancement_matches_unsigned_wrapping_rule_for_every_byte_combination() {
        for encoded in 0..=u8::MAX {
            for amount in 0..=u8::MAX {
                for period in 0..=u8::MAX {
                    // Independent wide arithmetic formulation of the source
                    // carry-discard and sign-conditioned correction.
                    let sum = (u16::from(encoded) + u16::from(amount)) % 256;
                    let corrected = if sum < 128 {
                        sum + u16::from(period)
                    } else {
                        sum
                    } % 128;
                    let reduced = if corrected >= u16::from(period) {
                        corrected - u16::from(period)
                    } else {
                        corrected
                    };
                    let mut control = AnimationControl::from_packed(encoded);
                    control.advance(amount, period);
                    assert_eq!(control.packed(), reduced as u8 | MANUAL_FRAME);
                }
            }
        }
        let mut control = AnimationControl::from_packed(MANUAL_FRAME);
        control.advance(30, 6);
        assert_eq!(control.resolve(0), 24); // one subtraction, not modulo
        control.advance(0, 0); // a zero period does not divide by zero
        assert_eq!(control.resolve(0), 24);
    }

    #[test]
    fn sprite_parameters_alias_render_channels_without_moving_actor() {
        let mut actor = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Effect);
        actor.base.position = Vector3 {
            x: 10,
            y: -20,
            z: 30,
        };
        actor.extension.texture_scroll_y = 47;
        let before = actor.clone();
        set_sprite(&mut actor, 5, 200);
        assert!(actor.base.flags.scaled_sprite);
        assert_eq!(actor.extension.depth_offset, 5);
        assert_eq!(actor.extension.texture_scroll_x, 200);
        actor.base.flags.scaled_sprite = before.base.flags.scaled_sprite;
        actor.extension.depth_offset = before.extension.depth_offset;
        actor.extension.texture_scroll_x = before.extension.texture_scroll_x;
        assert_eq!(actor, before);
    }
}
