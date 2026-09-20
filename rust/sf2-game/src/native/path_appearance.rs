//! Source path animation controls and scaled-sprite parameters. An animation
//! control is not a resolved presentation frame: automatic controls retain a
//! low payload that arithmetic still observes, even though rendering uses the
//! shared animation clock instead.

use super::Object;

const MANUAL_FRAME: u8 = 0x80;
const FRAME_VALUE: u8 = 0x7f;
const FAR_SORT_BIAS: i16 = 15_000;

/// Authored visibility and draw controls. Visibility commands deliberately
/// couple visibility with collision participation; the separate collision
/// command does not change visibility or consume pending contacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceCommand {
    /// Literal shape assignment changes only the catalog selection; it does
    /// not initialize a strategy, animation, visibility or collision state.
    Shape(super::ShapeId),
    Visibility(bool),
    Collision(bool),
    Shadow(bool),
    MaximumDrawDistance(bool),
    FarSortBias(bool),
    SuppressDeathEffects(bool),
}

impl AppearanceCommand {
    pub fn apply(self, actor: &mut Object) {
        match self {
            Self::Shape(shape) => actor.base.shape = shape,
            Self::Visibility(visible) => {
                actor.base.flags.visible = visible;
                actor.base.flags.collision_disabled = !visible;
            }
            Self::Collision(enabled) => actor.base.flags.collision_disabled = !enabled,
            Self::Shadow(enabled) => actor.base.flags.casts_shadow = enabled,
            Self::MaximumDrawDistance(enabled) => actor.base.flags.maximum_draw_distance = enabled,
            Self::FarSortBias(enabled) => actor.base.flags.far_sort_bias = enabled,
            Self::SuppressDeathEffects(enabled) => {
                actor.base.flags.suppress_death_effects = enabled
            }
        }
    }
}

/// `$7F:122C` publishes a separate object bias. The render boundary combines
/// it with its shape bias using word-width addition, never with world depth.
pub fn render_sort_bias(actor: &Object, shape_bias: i16) -> i16 {
    shape_bias.wrapping_add(if actor.base.flags.far_sort_bias {
        FAR_SORT_BIAS
    } else {
        0
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationChannel {
    Shape,
    Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationCommand {
    Initialize {
        channel: AnimationChannel,
        value: u8,
    },
    Advance {
        channel: AnimationChannel,
        amount: u8,
        period: u8,
    },
}

/// Path-owned controls, distinct from the renderer's resolved frame snapshot.
/// Other native strategy families currently author that snapshot directly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AnimationChannels {
    pub shape: AnimationControl,
    pub color: AnimationControl,
}

impl AnimationChannels {
    pub fn apply(&mut self, command: AnimationCommand) {
        let channel = match command {
            AnimationCommand::Initialize { channel, .. }
            | AnimationCommand::Advance { channel, .. } => channel,
        };
        let control = match channel {
            AnimationChannel::Shape => &mut self.shape,
            AnimationChannel::Color => &mut self.color,
        };
        match command {
            AnimationCommand::Initialize { value, .. } => control.initialize(value),
            AnimationCommand::Advance { amount, period, .. } => control.advance(amount, period),
        }
    }
}

/// The world supplies its actual animation clock at the render boundary;
/// advancing an actor path must not independently advance this shared clock.
pub fn publish_animation(actor: &mut Object, clock: u8) {
    let controls = actor.extension.path_state.animation;
    actor.extension.animation_frame = controls.shape.resolve(clock);
    actor.extension.color_frame = controls.color.resolve(clock);
}

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

    /// Collision geometry uses the full shared clock byte when automatic,
    /// unlike the renderer's seven-bit frame selection.
    pub const fn fixed_frame(self) -> Option<u8> {
        if self.manual {
            Some(self.value)
        } else {
            None
        }
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
    actor.extension.depth_offset = (actor.extension.depth_offset & 0xFF00) | u16::from(color);
    actor.extension.texture_scroll_x = size;
}

#[cfg(test)]
mod tests {
    use super::super::{Behavior, ObjectKind, ShapeId, Vector3};

    #[test]
    fn channels_are_independent_and_publication_does_not_advance_them() {
        let mut actor = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
        actor
            .extension
            .path_state
            .animation
            .apply(AnimationCommand::Initialize {
                channel: AnimationChannel::Shape,
                value: 5,
            });
        publish_animation(&mut actor, 17);
        assert_eq!(actor.extension.animation_frame, 5);
        assert_eq!(actor.extension.color_frame, 17);
        actor
            .extension
            .path_state
            .animation
            .apply(AnimationCommand::Advance {
                channel: AnimationChannel::Color,
                amount: 1,
                period: 2,
            });
        let retained = actor.extension.path_state.animation;
        for clock in 0..=u8::MAX {
            publish_animation(&mut actor, clock);
            assert_eq!(actor.extension.animation_frame, 5);
            assert_eq!(actor.extension.color_frame, 1);
            assert_eq!(actor.extension.path_state.animation, retained);
        }
    }
    use super::*;

    #[test]
    fn visibility_couples_collision_but_other_controls_and_draw_observations_are_independent() {
        for flags in 0..256 {
            let mut original =
                Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
            original.base.flags.visible = flags & 1 != 0;
            original.base.flags.collision_disabled = flags & 2 != 0;
            original.base.flags.casts_shadow = flags & 4 != 0;
            original.base.flags.maximum_draw_distance = flags & 8 != 0;
            original.base.flags.draw_list_admitted = flags & 16 != 0;
            original.base.flags.collided = flags & 32 != 0;
            original.base.flags.far_sort_bias = flags & 64 != 0;
            original.base.flags.suppress_death_effects = flags & 128 != 0;
            original.base.hit_flags = 255;
            original.extension.path_state.conditions.hit_event_pending = true;
            for enabled in [false, true] {
                for command in [
                    AppearanceCommand::Visibility(enabled),
                    AppearanceCommand::Collision(enabled),
                    AppearanceCommand::Shadow(enabled),
                    AppearanceCommand::MaximumDrawDistance(enabled),
                    AppearanceCommand::FarSortBias(enabled),
                    AppearanceCommand::SuppressDeathEffects(enabled),
                ] {
                    let mut expected = original.clone();
                    match command {
                        AppearanceCommand::Shape(shape) => expected.base.shape = shape,
                        AppearanceCommand::Visibility(value) => {
                            expected.base.flags.visible = value;
                            expected.base.flags.collision_disabled = !value;
                        }
                        AppearanceCommand::Collision(value) => {
                            expected.base.flags.collision_disabled = !value
                        }
                        AppearanceCommand::Shadow(value) => {
                            expected.base.flags.casts_shadow = value
                        }
                        AppearanceCommand::MaximumDrawDistance(value) => {
                            expected.base.flags.maximum_draw_distance = value
                        }
                        AppearanceCommand::FarSortBias(value) => {
                            expected.base.flags.far_sort_bias = value
                        }
                        AppearanceCommand::SuppressDeathEffects(value) => {
                            expected.base.flags.suppress_death_effects = value
                        }
                    }
                    let mut actual = original.clone();
                    command.apply(&mut actual);
                    assert_eq!(actual, expected);
                    command.apply(&mut actual);
                    assert_eq!(actual, expected);
                }
            }
        }
    }

    #[test]
    fn far_sort_bias_wraps_without_modifying_actor_or_shape_bias() {
        let mut actor = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
        actor.base.position = Vector3 {
            x: 17,
            y: -19,
            z: i16::MAX,
        };
        for enabled in [false, true] {
            AppearanceCommand::FarSortBias(enabled).apply(&mut actor);
            let original = actor.clone();
            for bits in 0..=u16::MAX {
                let expected = (i32::from(bits as i16) + if enabled { 15_000 } else { 0 }) as i16;
                assert_eq!(render_sort_bias(&actor, bits as i16), expected);
            }
            assert_eq!(actor, original);
        }
    }

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

    #[test]
    fn sprite_colour_preserves_the_authored_depth_high_byte() {
        let mut actor = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Effect);
        for high in 0..=u8::MAX {
            for color in 0..=u8::MAX {
                actor.extension.depth_offset = u16::from_le_bytes([!color, high]);
                set_sprite(&mut actor, color, !color);
                assert_eq!(
                    actor.extension.depth_offset,
                    u16::from_le_bytes([color, high])
                );
                assert_eq!(actor.extension.texture_scroll_x, !color);
            }
        }
    }
}
