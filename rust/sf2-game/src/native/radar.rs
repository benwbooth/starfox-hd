//! Authored radar-marker appearance and projection (`$7F:51EE..5265`,
//! `$7F:52A3..5329`). Frame gating, radar focus and HUD publication belong
//! to the caller; these leaves neither draw nor advance the shared clock.

use super::{Object, Vector3};

const HEIGHT_COLORED: u8 = 0x80;
const STYLE_VALUE: u8 = 0x7F;
const LEVEL_GLYPH: u16 = 46;
const ALTITUDE_BAND: i16 = 512;
const TILE_STYLE_BASE: u16 = 0x2040;
const FOCUS_GLYPH: u8 = 30;
const FOCUS_COLORED_GLYPH: u8 = 134;

/// The path opcode inherited the SF1 name TRAIL, but SF2 stores it in the
/// radar marker channel (1CEE), not in a particle emitter or texture field.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RadarMarker(u8);

impl RadarMarker {
    pub const fn from_packed(value: u8) -> Self {
        Self(value)
    }
    pub const fn packed(self) -> u8 {
        self.0
    }

    fn style(self, height_delta: i16, height_cues: bool) -> Option<RadarStyle> {
        if self.0 == 0 {
            return None;
        }
        let glyph = if self.0 & HEIGHT_COLORED == 0 {
            u16::from(self.0)
        } else {
            let level = (u16::from(self.0 & STYLE_VALUE) << 8) | LEVEL_GLYPH;
            if !height_cues {
                level.wrapping_sub(1)
            } else if height_delta.wrapping_sub(-ALTITUDE_BAND) < 0 {
                level.wrapping_add(1)
            } else if height_delta.wrapping_sub(ALTITUDE_BAND) >= 0 {
                level.wrapping_sub(1)
            } else {
                level
            }
        };
        Some(RadarStyle {
            tile_and_attributes: glyph.wrapping_add(TILE_STYLE_BASE),
            counts_for_focus: matches!(self.0, FOCUS_GLYPH | FOCUS_COLORED_GLYPH),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarStyle {
    /// Authored glyph and draw attributes after the source tile-base add.
    pub tile_and_attributes: u16,
    pub counts_for_focus: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarPlot {
    pub x: u8,
    pub y: u8,
    pub style: RadarStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarView {
    pub center: Vector3,
    pub screen_origin: [u8; 2],
    /// Positive halves with arithmetic-floor rounding; negative doubles
    /// with word wrapping. This does not change the clipping operands.
    pub scale_shift: i16,
    pub clip_span: i16,
    pub clip_bias: i16,
}

fn scaled(mut value: i16, shift: i16) -> i16 {
    // After sixteen repetitions the value is stable: zero after doubling,
    // or its sign after arithmetic halving. Preserve the minimum shift.
    for _ in 0..u32::from(shift.unsigned_abs()).min(i16::BITS) {
        value = if shift < 0 {
            value.wrapping_mul(2)
        } else {
            value >> 1
        };
    }
    value
}

impl RadarView {
    fn admits(self, offset: i16) -> bool {
        let coordinate = (offset >> 1).wrapping_add(self.clip_bias);
        coordinate >= 0 && coordinate.wrapping_sub(self.clip_span) < 0
    }

    pub fn project(self, position: Vector3, marker: RadarMarker) -> Option<RadarPlot> {
        let style = marker.style(
            position.y.wrapping_sub(self.center.y),
            self.scale_shift.wrapping_sub(1) < 0,
        )?;
        let x = position.x.wrapping_sub(self.center.x);
        let y = self.center.z.wrapping_sub(position.z);
        if !self.admits(x) || !self.admits(y) {
            return None;
        }
        Some(RadarPlot {
            x: ((scaled(x, self.scale_shift) as u16 >> 8) as u8)
                .wrapping_add(self.screen_origin[0]),
            y: ((scaled(y, self.scale_shift) as u16 >> 8) as u8)
                .wrapping_add(self.screen_origin[1]),
            style,
        })
    }

    pub fn project_actor(self, actor: &Object) -> Option<RadarPlot> {
        self.project(actor.base.position, actor.extension.radar_marker)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Behavior, ObjectKind, ShapeId};
    use super::*;

    fn word(value: i32) -> i32 {
        (value + 32768).rem_euclid(65536) - 32768
    }

    #[test]
    fn marker_styles_preserve_every_byte_and_wrapped_altitude_comparison() {
        for packed in 0..=u8::MAX {
            let marker = RadarMarker::from_packed(packed);
            assert_eq!(marker.packed(), packed);
            for height in 0..=u16::MAX {
                for height_cues in [false, true] {
                    let delta = i32::from(height as i16);
                    let glyph = if packed < 128 {
                        u32::from(packed)
                    } else {
                        let base = u32::from(packed % 128) * 256 + 46;
                        if !height_cues {
                            base - 1
                        } else if word(delta + 512) < 0 {
                            base + 1
                        } else if word(delta - 512) >= 0 {
                            base - 1
                        } else {
                            base
                        }
                    };
                    assert_eq!(
                        marker.style(height as i16, height_cues),
                        if packed == 0 {
                            None
                        } else {
                            Some(RadarStyle {
                                tile_and_attributes: ((glyph + 8256) % 65536) as u16,
                                counts_for_focus: packed == 30 || packed == 134,
                            })
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn scaling_covers_all_words_and_extreme_signed_shift_counts() {
        for shift in [-32768i16, -17, -16, -15, -8, -1, 0, 1, 8, 15, 16, 17, 32767] {
            for value in 0..=u16::MAX {
                let signed = i32::from(value as i16);
                let expected = if shift < 0 {
                    if shift <= -16 {
                        0
                    } else {
                        word(signed * 2i32.pow((-shift) as u32))
                    }
                } else if shift >= 16 {
                    if signed < 0 {
                        -1
                    } else {
                        0
                    }
                } else {
                    signed.div_euclid(2i32.pow(shift as u32))
                };
                assert_eq!(scaled(value as i16, shift), expected as i16);
            }
        }
    }

    #[test]
    fn clipping_uses_unscaled_offsets_with_wrapping_comparisons_and_independent_screen_bytes() {
        for (span, bias) in [
            (8192, 4096),
            (0, 0),
            (32767, 32767),
            (-32768, 0),
            (300, 30000),
        ] {
            for shift in [-32768, -1, 0, 1, 16] {
                let view = RadarView {
                    center: Vector3 {
                        x: 32000,
                        y: -30000,
                        z: -32000,
                    },
                    screen_origin: [250, 1],
                    scale_shift: shift,
                    clip_span: span,
                    clip_bias: bias,
                };
                for value in 0..=u16::MAX {
                    let position = Vector3 {
                        x: value as i16,
                        y: value.rotate_left(3) as i16,
                        z: value.rotate_left(7) as i16,
                    };
                    let x = word(i32::from(position.x) - i32::from(view.center.x));
                    let y = word(i32::from(view.center.z) - i32::from(position.z));
                    let admits = |offset: i32| {
                        let coordinate = word(offset.div_euclid(2) + i32::from(bias));
                        coordinate >= 0 && word(coordinate - i32::from(span)) < 0
                    };
                    let plotted = view.project(position, RadarMarker::from_packed(134));
                    assert_eq!(plotted.is_some(), admits(x) && admits(y));
                    if let Some(plot) = plotted {
                        let scale = |offset: i32| match shift {
                            -32768 => 0,
                            -1 => word(offset * 2),
                            0 => offset,
                            1 => offset.div_euclid(2),
                            16 => {
                                if offset < 0 {
                                    -1
                                } else {
                                    0
                                }
                            }
                            _ => unreachable!(),
                        };
                        assert_eq!(
                            plot.x,
                            (scale(x).div_euclid(256) + 250).rem_euclid(256) as u8
                        );
                        assert_eq!(plot.y, (scale(y).div_euclid(256) + 1).rem_euclid(256) as u8);
                        assert!(plot.style.counts_for_focus);
                        if shift == -32768 || shift > 0 {
                            assert_eq!(plot.style.tile_and_attributes, 0x266D);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn marker_assignment_and_actor_projection_leave_unrelated_actor_state_untouched() {
        let mut actor = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
        actor.extension.render_parameter = 151;
        actor.extension.texture_scroll_x = 73;
        actor.extension.depth_offset = 0x89AB;
        let view = RadarView {
            center: Vector3::default(),
            screen_origin: [100, 80],
            scale_shift: 0,
            clip_span: 8192,
            clip_bias: 4096,
        };
        for packed in 0..=u8::MAX {
            let mut expected = actor.clone();
            expected.extension.radar_marker = RadarMarker::from_packed(packed);
            super::super::path_appearance::AppearanceCommand::RadarMarker(
                expected.extension.radar_marker,
            )
            .apply(&mut actor);
            assert_eq!(actor, expected);
            assert_eq!(view.project_actor(&actor).is_some(), packed != 0);
            if let Some(plot) = view.project_actor(&actor) {
                assert_eq!((plot.x, plot.y), (100, 80));
            }
            assert_eq!(actor, expected);
        }
    }
}
